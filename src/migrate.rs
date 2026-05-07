//! One-shot migration from the original Django analytics SQLite into the
//! Rust hot-field schema. Preserves property UUIDs so embedded snippets on
//! tracked sites keep working without a snippet rotation.
//!
//! Invoked as a subcommand of the main binary so it ships in the existing
//! Docker image with no extra wiring:
//!
//! ```text
//! ./analytics migrate <path-to-django.sqlite3> [--force]
//! ```
//!
//! Without `--force`, refuses to run if the destination has any properties,
//! events, or bot_events. With `--force`, wipes those tables (and `meta`)
//! before importing — so an auto-created Proprium row from a prior boot is
//! replaced with the original Proprium from Django.

use anyhow::{bail, Context, Result};
use sqlx::{Acquire, Row};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub async fn run(source: PathBuf, force: bool) -> Result<()> {
    if !source.exists() {
        bail!("source database not found: {}", source.display());
    }

    let data_dir = std::env::var("ANALYTICS_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./data"));
    std::fs::create_dir_all(&data_dir)?;

    let pool = crate::db::init(&data_dir).await?;

    let dest_props: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM properties").fetch_one(&pool).await?;
    let dest_events: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&pool).await?;
    let dest_bots: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM bot_events").fetch_one(&pool).await?;

    if dest_props + dest_events + dest_bots > 0 {
        if !force {
            bail!(
                "destination not empty (properties={dest_props}, events={dest_events}, bot_events={dest_bots}); pass --force to wipe before migrating"
            );
        }
        eprintln!(
            "wiping destination: {dest_props} properties, {dest_events} events, {dest_bots} bot_events"
        );
        sqlx::query("DELETE FROM events").execute(&pool).await?;
        sqlx::query("DELETE FROM bot_events").execute(&pool).await?;
        sqlx::query("DELETE FROM properties").execute(&pool).await?;
        sqlx::query("DELETE FROM meta").execute(&pool).await?;
    }

    // ATTACH source DB. SQLite won't attach across pool connections cleanly,
    // so grab a single connection and use it for the whole migration.
    let mut conn = pool.acquire().await?;
    let attach_sql = format!("ATTACH DATABASE '{}' AS src", escape_path(&source));
    sqlx::query(&attach_sql).execute(&mut *conn).await?;

    // 1. Read django properties (host-side parse so we can convert hex UUIDs
    //    to 16-byte BLOBs in Rust without depending on a specific SQLite
    //    version's unhex() availability).
    let prop_rows = sqlx::query(
        "SELECT id, name, custom_cards, is_protected, is_public, created_at, updated_at \
         FROM src.properties_property",
    )
    .fetch_all(&mut *conn)
    .await
    .context("reading source properties")?;

    if prop_rows.is_empty() {
        bail!("source database has no properties; nothing to migrate");
    }

    eprintln!("found {} properties in source", prop_rows.len());

    let mut tx = conn.begin().await?;

    let mut proprium_blob: Option<Vec<u8>> = None;
    for row in &prop_rows {
        let id_text: String = row.try_get("id")?;
        let name: String = row.try_get("name")?;
        let custom_cards: Option<String> = row.try_get("custom_cards")?;
        let is_protected: i64 = row.try_get("is_protected")?;
        let is_public: i64 = row.try_get("is_public")?;
        let created_at: String = row.try_get("created_at")?;
        let updated_at: String = row.try_get("updated_at")?;

        let uuid = parse_django_uuid(&id_text)
            .with_context(|| format!("parsing property id {id_text:?}"))?;
        let id_blob = uuid.as_bytes().to_vec();

        sqlx::query(
            "INSERT INTO properties (id, name, custom_cards, is_protected, is_public, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, \
                     CAST((julianday(?) - 2440587.5) * 86400000 AS INTEGER), \
                     CAST((julianday(?) - 2440587.5) * 86400000 AS INTEGER))",
        )
        .bind(&id_blob)
        .bind(&name)
        .bind(custom_cards.unwrap_or_else(|| "[]".to_string()))
        .bind(is_protected)
        .bind(is_public)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("inserting property {name:?}"))?;

        if name == "Proprium" && is_protected != 0 {
            proprium_blob = Some(id_blob);
        }
    }

    // 2. Build a temp mapping (text-hex id → BLOB) so the events INSERT…SELECT
    //    can join across the ATTACHed database without per-row Rust roundtrips.
    sqlx::query("CREATE TEMP TABLE prop_id_map (text_id TEXT PRIMARY KEY, blob_id BLOB NOT NULL)")
        .execute(&mut *tx)
        .await?;
    for row in &prop_rows {
        let id_text: String = row.try_get("id")?;
        let uuid = parse_django_uuid(&id_text)?;
        sqlx::query("INSERT INTO prop_id_map (text_id, blob_id) VALUES (?, ?)")
            .bind(&id_text)
            .bind(uuid.as_bytes().to_vec())
            .execute(&mut *tx)
            .await?;
    }

    // 3. Bot events first — rows where data.is_bot is set route to bot_events
    //    with a smaller projection.
    let bot_count = sqlx::query(
        "INSERT INTO bot_events (property_id, event, created_at, bot_name, url, user_agent, country, extra) \
         SELECT \
             m.blob_id, \
             e.event, \
             CAST((julianday(e.created_at) - 2440587.5) * 86400000 AS INTEGER), \
             json_extract(e.data, '$.bot_name'), \
             json_extract(e.data, '$.url'), \
             json_extract(e.data, '$.user_agent'), \
             json_extract(e.data, '$.country'), \
             '{}' \
         FROM src.properties_event e \
         JOIN prop_id_map m ON e.property_id = m.text_id \
         WHERE json_extract(e.data, '$.is_bot') IS NOT NULL",
    )
    .execute(&mut *tx)
    .await
    .context("inserting bot_events")?
    .rows_affected();

    // 4. Human events. Project every hot field via json_extract; lat/lon
    //    come out of Django's `loc: [lat, lon]` array; `time_on_page` (ms)
    //    flows into `time_on_page_ms`. The `extra` blob stays empty since
    //    Django stored everything in `data` and the hot fields cover what
    //    the dashboard uses.
    let human_count = sqlx::query(
        "INSERT INTO events ( \
             property_id, event, created_at, user_id, url, title, referrer, user_agent, \
             platform, browser, device, screen_width, screen_height, country, region, city, \
             lat, lon, utm_source, utm_medium, utm_campaign, utm_term, utm_content, \
             time_on_page_ms, extra \
         ) \
         SELECT \
             m.blob_id, \
             e.event, \
             CAST((julianday(e.created_at) - 2440587.5) * 86400000 AS INTEGER), \
             CAST(json_extract(e.data, '$.user_id') AS TEXT), \
             json_extract(e.data, '$.url'), \
             json_extract(e.data, '$.title'), \
             json_extract(e.data, '$.referrer'), \
             json_extract(e.data, '$.user_agent'), \
             json_extract(e.data, '$.platform'), \
             json_extract(e.data, '$.browser'), \
             json_extract(e.data, '$.device'), \
             json_extract(e.data, '$.screen_width'), \
             json_extract(e.data, '$.screen_height'), \
             json_extract(e.data, '$.country'), \
             json_extract(e.data, '$.region'), \
             json_extract(e.data, '$.city'), \
             json_extract(e.data, '$.loc[0]'), \
             json_extract(e.data, '$.loc[1]'), \
             json_extract(e.data, '$.utm_source'), \
             json_extract(e.data, '$.utm_medium'), \
             json_extract(e.data, '$.utm_campaign'), \
             json_extract(e.data, '$.utm_term'), \
             json_extract(e.data, '$.utm_content'), \
             json_extract(e.data, '$.time_on_page'), \
             '{}' \
         FROM src.properties_event e \
         JOIN prop_id_map m ON e.property_id = m.text_id \
         WHERE json_extract(e.data, '$.is_bot') IS NULL",
    )
    .execute(&mut *tx)
    .await
    .context("inserting events")?
    .rows_affected();

    // 5. Persist the Proprium id so self-tracking continues without a fresh row.
    if let Some(blob) = proprium_blob {
        let uuid = Uuid::from_slice(&blob)?;
        sqlx::query("INSERT OR REPLACE INTO meta (key, value) VALUES ('proprium_id', ?)")
            .bind(uuid.to_string())
            .execute(&mut *tx)
            .await?;
        eprintln!("set proprium_id = {uuid}");
    } else {
        eprintln!("no Proprium property found in source — server will create a new one on next boot");
    }

    sqlx::query("DROP TABLE prop_id_map").execute(&mut *tx).await?;
    tx.commit().await?;

    sqlx::query("DETACH DATABASE src").execute(&mut *conn).await?;

    eprintln!(
        "migrated {} properties, {human_count} events, {bot_count} bot_events",
        prop_rows.len()
    );
    Ok(())
}

/// Parse a Django-stored UUID (32 lowercase hex chars, no dashes) into a `Uuid`.
fn parse_django_uuid(s: &str) -> Result<Uuid> {
    Uuid::parse_str(s).context("expected 32-hex Django UUID")
}

/// Quote a path for use in an `ATTACH DATABASE 'path' AS src` statement.
/// SQLite uses single-quote-doubled escaping inside string literals.
fn escape_path(p: &Path) -> String {
    p.display().to_string().replace('\'', "''")
}
