use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

pub async fn init(data_dir: &Path) -> anyhow::Result<SqlitePool> {
    let db_path = data_dir.join("db.sqlite3");
    let url = format!("sqlite://{}", db_path.display());

    // Ensure file exists so sqlx can attach.
    if !db_path.exists() {
        std::fs::File::create(&db_path)?;
    }

    let opts = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn ensure_proprium(pool: &SqlitePool) -> anyhow::Result<Uuid> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT value FROM meta WHERE key = 'proprium_id'")
            .fetch_optional(pool)
            .await?;

    if let Some((s,)) = existing {
        if let Ok(uuid) = Uuid::parse_str(&s) {
            // Make sure the property still exists (db could have been wiped).
            let row: Option<(Vec<u8>,)> =
                sqlx::query_as("SELECT id FROM properties WHERE id = ?")
                    .bind(uuid.as_bytes().to_vec())
                    .fetch_optional(pool)
                    .await?;
            if row.is_some() {
                return Ok(uuid);
            }
        }
    }

    let id = Uuid::new_v4();
    let now = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        r#"INSERT INTO properties (id, name, custom_cards, is_protected, is_public, created_at, updated_at)
           VALUES (?, 'Proprium', '[]', 1, 0, ?, ?)"#,
    )
    .bind(id.as_bytes().to_vec())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    sqlx::query("INSERT OR REPLACE INTO meta (key, value) VALUES ('proprium_id', ?)")
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(id)
}
