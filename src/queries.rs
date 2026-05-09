//! Dashboard aggregation queries. Mirror of `properties/queries.py` from the
//! Django version, but talking to the hot-field schema so most aggregations
//! become straight COUNT(*) over typed columns.
//!
//! Time arithmetic uses unix milliseconds (matches `events.created_at`).

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use serde::Serialize;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::models::CustomCard;

const BUILT_IN_EVENTS: &[&str] = &["session_start", "page_view", "page_leave", "click", "scroll"];

const TIME_ON_PAGE_MIN_S: f64 = 1.0;
const TIME_ON_PAGE_MAX_S: f64 = 30.0 * 60.0;

#[derive(Debug, Clone, Serialize)]
pub struct LabelCount {
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphPoint {
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventCard {
    pub name: String,
    pub value: serde_json::Value,
    pub percent_change: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CustomEventDescriptor {
    pub event: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct BotTraffic {
    pub total: i64,
    pub top_bots: Vec<LabelCount>,
    pub top_pages: Vec<LabelCount>,
}

#[derive(Debug, Clone, Default)]
pub struct EventCounts {
    pub session_start: i64,
    pub page_view: i64,
    pub click: i64,
    pub scroll: i64,
    pub total: i64,
}

fn pct_change(current: f64, previous: f64) -> i64 {
    if previous == 0.0 {
        return 0;
    }
    ((current - previous) / previous * 100.0).round() as i64
}

fn filter_clause(filter_url: Option<&str>) -> (&'static str, Option<String>) {
    match filter_url {
        Some(_) => (" AND url = ?", filter_url.map(|s| s.to_string())),
        None => ("", None),
    }
}

/// Total unique user_ids seen in the last 30 minutes.
pub async fn total_live_users(pool: &SqlitePool, property_id: &Uuid) -> i64 {
    let cutoff = (Utc::now() - Duration::minutes(30)).timestamp_millis();
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(DISTINCT user_id) FROM events \
         WHERE property_id = ? AND created_at >= ? AND user_id IS NOT NULL",
    )
    .bind(property_id.as_bytes().to_vec())
    .bind(cutoff)
    .fetch_one(pool)
    .await
    .unwrap_or(0)
}

pub async fn event_counts(
    pool: &SqlitePool,
    property_id: &Uuid,
    start_ms: i64,
    end_ms: i64,
    filter_url: Option<&str>,
) -> EventCounts {
    let (extra_sql, extra_bind) = filter_clause(filter_url);
    let sql = format!(
        "SELECT \
            SUM(CASE WHEN event = 'session_start' THEN 1 ELSE 0 END) AS session_start, \
            SUM(CASE WHEN event = 'page_view'     THEN 1 ELSE 0 END) AS page_view, \
            SUM(CASE WHEN event = 'click'         THEN 1 ELSE 0 END) AS click, \
            SUM(CASE WHEN event = 'scroll'        THEN 1 ELSE 0 END) AS scroll, \
            COUNT(*) AS total \
         FROM events \
         WHERE property_id = ? AND created_at >= ? AND created_at <= ?{}",
        extra_sql
    );
    let mut q = sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<i64>, Option<i64>, i64)>(&sql)
        .bind(property_id.as_bytes().to_vec())
        .bind(start_ms)
        .bind(end_ms);
    if let Some(v) = extra_bind {
        q = q.bind(v);
    }
    let row = q.fetch_one(pool).await.unwrap_or((None, None, None, None, 0));
    EventCounts {
        session_start: row.0.unwrap_or(0),
        page_view: row.1.unwrap_or(0),
        click: row.2.unwrap_or(0),
        scroll: row.3.unwrap_or(0),
        total: row.4,
    }
}

async fn engaged_users(
    pool: &SqlitePool,
    property_id: &Uuid,
    start_ms: i64,
    end_ms: i64,
    filter_url: Option<&str>,
    session_starts: i64,
) -> f64 {
    if session_starts == 0 {
        return 0.0;
    }
    let (extra_sql, extra_bind) = filter_clause(filter_url);
    let sql = format!(
        "SELECT COUNT(*) FROM ( \
           SELECT user_id FROM events \
           WHERE property_id = ? AND created_at >= ? AND created_at <= ? \
                 AND user_id IS NOT NULL{} \
           GROUP BY user_id HAVING COUNT(*) >= 10 \
         )",
        extra_sql
    );
    let mut q = sqlx::query_scalar::<_, i64>(&sql)
        .bind(property_id.as_bytes().to_vec())
        .bind(start_ms)
        .bind(end_ms);
    if let Some(v) = extra_bind {
        q = q.bind(v);
    }
    let engaged = q.fetch_one(pool).await.unwrap_or(0);
    ((engaged as f64) / (session_starts as f64) * 100.0 * 100.0).round() / 100.0
}

async fn avg_time_on_page(
    pool: &SqlitePool,
    property_id: &Uuid,
    start_ms: i64,
    end_ms: i64,
    filter_url: Option<&str>,
) -> f64 {
    let (extra_sql, extra_bind) = filter_clause(filter_url);
    let sql = format!(
        "SELECT AVG(time_on_page_ms / 1000.0) FROM events \
         WHERE property_id = ? AND created_at >= ? AND created_at <= ? \
               AND event = 'page_leave' \
               AND time_on_page_ms IS NOT NULL \
               AND time_on_page_ms / 1000.0 BETWEEN ? AND ?{}",
        extra_sql
    );
    let mut q = sqlx::query_scalar::<_, Option<f64>>(&sql)
        .bind(property_id.as_bytes().to_vec())
        .bind(start_ms)
        .bind(end_ms)
        .bind(TIME_ON_PAGE_MIN_S)
        .bind(TIME_ON_PAGE_MAX_S);
    if let Some(v) = extra_bind {
        q = q.bind(v);
    }
    let avg = q.fetch_one(pool).await.unwrap_or(None).unwrap_or(0.0);
    (avg * 100.0).round() / 100.0
}

pub async fn standard_event_cards(
    pool: &SqlitePool,
    property_id: &Uuid,
    start_ms: i64,
    end_ms: i64,
    prev_start_ms: i64,
    prev_end_ms: i64,
    filter_url: Option<&str>,
) -> Vec<EventCard> {
    let cur = event_counts(pool, property_id, start_ms, end_ms, filter_url).await;
    let prev = event_counts(pool, property_id, prev_start_ms, prev_end_ms, filter_url).await;

    let mut cards = vec![
        EventCard {
            name: "Total session starts".into(),
            value: cur.session_start.into(),
            percent_change: pct_change(cur.session_start as f64, prev.session_start as f64),
            help_text: Some("Unique users visiting your site for your selected date range.".into()),
        },
        EventCard {
            name: "Total page views".into(),
            value: cur.page_view.into(),
            percent_change: pct_change(cur.page_view as f64, prev.page_view as f64),
            help_text: Some("Total pages viewed for your selected date range.".into()),
        },
        EventCard {
            name: "Total clicks".into(),
            value: cur.click.into(),
            percent_change: pct_change(cur.click as f64, prev.click as f64),
            help_text: Some("Total clicks users made on all your pages for your selected date range.".into()),
        },
        EventCard {
            name: "Total scrolls".into(),
            value: cur.scroll.into(),
            percent_change: pct_change(cur.scroll as f64, prev.scroll as f64),
            help_text: Some("Total scrolls users made on all your pages for your selected date range.".into()),
        },
        EventCard {
            name: "Total events".into(),
            value: cur.total.into(),
            percent_change: pct_change(cur.total as f64, prev.total as f64),
            help_text: Some("All events for your selected date range, including custom events.".into()),
        },
    ];

    let eng_cur =
        engaged_users(pool, property_id, start_ms, end_ms, filter_url, cur.session_start).await;
    let eng_prev = engaged_users(
        pool,
        property_id,
        prev_start_ms,
        prev_end_ms,
        filter_url,
        prev.session_start,
    )
    .await;
    cards.push(EventCard {
        name: "Total user engagement".into(),
        value: format!("{eng_cur}%").into(),
        percent_change: pct_change(eng_cur, eng_prev),
        help_text: Some("An engaged user is a user with more than 10 events collected for your selected date range.".into()),
    });

    let t_cur = avg_time_on_page(pool, property_id, start_ms, end_ms, filter_url).await;
    let t_prev = avg_time_on_page(pool, property_id, prev_start_ms, prev_end_ms, filter_url).await;
    cards.push(EventCard {
        name: "Avg. time on page".into(),
        value: format!("{t_cur}s").into(),
        percent_change: pct_change(t_cur, t_prev),
        help_text: Some("Average time a user spends on each page. Sessions over 30 minutes are excluded as idle.".into()),
    });

    cards
}

pub async fn custom_event_cards(
    pool: &SqlitePool,
    property_id: &Uuid,
    custom_cards: &[CustomCard],
    start_ms: i64,
    end_ms: i64,
    prev_start_ms: i64,
    prev_end_ms: i64,
    filter_url: Option<&str>,
) -> (Vec<EventCard>, Vec<CustomEventDescriptor>) {
    // All non-built-in event names that have ever been seen for this property.
    let placeholders = std::iter::repeat("?")
        .take(BUILT_IN_EVENTS.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT DISTINCT event FROM events \
         WHERE property_id = ? AND event NOT IN ({}) \
         ORDER BY event",
        placeholders
    );
    let mut q = sqlx::query_scalar::<_, String>(&sql).bind(property_id.as_bytes().to_vec());
    for built in BUILT_IN_EVENTS {
        q = q.bind(built);
    }
    let names: Vec<String> = q.fetch_all(pool).await.unwrap_or_default();

    let active: std::collections::HashSet<&str> = custom_cards
        .iter()
        .filter(|c| c.value)
        .map(|c| c.event.as_str())
        .collect();

    let descriptors: Vec<CustomEventDescriptor> = names
        .iter()
        .map(|n| CustomEventDescriptor {
            event: n.clone(),
            active: active.contains(n.as_str()),
        })
        .collect();

    if active.is_empty() {
        return (Vec::new(), descriptors);
    }

    // Aggregate counts for active custom events in current and previous periods.
    let active_names: Vec<&str> = active.iter().copied().collect();
    let count_for = |period_start: i64, period_end: i64| {
        let placeholders = std::iter::repeat("?")
            .take(active_names.len())
            .collect::<Vec<_>>()
            .join(",");
        let (extra_sql, extra_bind) = filter_clause(filter_url);
        let sql = format!(
            "SELECT event, COUNT(*) FROM events \
             WHERE property_id = ? AND created_at >= ? AND created_at <= ? \
                   AND event IN ({}){} \
             GROUP BY event",
            placeholders, extra_sql
        );
        let pool = pool.clone();
        let property_id = *property_id;
        let names: Vec<String> = active_names.iter().map(|s| (*s).to_string()).collect();
        let extra_bind = extra_bind.clone();
        async move {
            let mut q = sqlx::query_as::<_, (String, i64)>(&sql)
                .bind(property_id.as_bytes().to_vec())
                .bind(period_start)
                .bind(period_end);
            for n in &names {
                q = q.bind(n);
            }
            if let Some(v) = extra_bind {
                q = q.bind(v);
            }
            q.fetch_all(&pool).await.unwrap_or_default()
        }
    };

    let cur_rows = count_for(start_ms, end_ms).await;
    let prev_rows = count_for(prev_start_ms, prev_end_ms).await;
    let cur_map: std::collections::HashMap<String, i64> = cur_rows.into_iter().collect();
    let prev_map: std::collections::HashMap<String, i64> = prev_rows.into_iter().collect();

    let cards: Vec<EventCard> = active_names
        .iter()
        .map(|name| {
            let v = *cur_map.get(*name).unwrap_or(&0);
            let p = *prev_map.get(*name).unwrap_or(&0);
            EventCard {
                name: (*name).to_string(),
                value: v.into(),
                percent_change: pct_change(v as f64, p as f64),
                help_text: None,
            }
        })
        .collect();

    (cards, descriptors)
}

/// Time-series chart data. Buckets daily/weekly/monthly based on the date range,
/// stepping backwards from `end_date`.
pub async fn events_graph(
    pool: &SqlitePool,
    property_id: &Uuid,
    start_ms: i64,
    end_ms: i64,
    filter_url: Option<&str>,
    end_date: NaiveDate,
    range_days: i64,
) -> Vec<GraphPoint> {
    let (extra_sql, extra_bind) = filter_clause(filter_url);
    let sql = format!(
        "SELECT date(created_at / 1000, 'unixepoch') AS day, COUNT(*) \
         FROM events \
         WHERE property_id = ? AND created_at >= ? AND created_at <= ?{} \
         GROUP BY day",
        extra_sql
    );
    let mut q = sqlx::query_as::<_, (String, i64)>(&sql)
        .bind(property_id.as_bytes().to_vec())
        .bind(start_ms)
        .bind(end_ms);
    if let Some(v) = extra_bind {
        q = q.bind(v);
    }
    let rows = q.fetch_all(pool).await.unwrap_or_default();

    let mut by_day: std::collections::HashMap<NaiveDate, i64> =
        std::collections::HashMap::with_capacity(rows.len());
    for (s, c) in rows {
        if let Ok(d) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
            by_day.insert(d, c);
        }
    }

    let bucket_sum = |start: NaiveDate, days: i64| -> i64 {
        (0..days)
            .map(|j| {
                start
                    .checked_add_signed(Duration::days(j))
                    .and_then(|d| by_day.get(&d).copied())
                    .unwrap_or(0)
            })
            .sum()
    };

    let mut points = Vec::new();
    if range_days <= 28 {
        for i in 0..range_days {
            if let Some(d) = end_date.checked_sub_signed(Duration::days(i)) {
                points.push((d, by_day.get(&d).copied().unwrap_or(0)));
            }
        }
    } else if range_days <= 6 * 28 {
        let weeks = range_days / 7;
        for w in 0..weeks {
            if let Some(d) = end_date.checked_sub_signed(Duration::days(7 * w)) {
                points.push((d, bucket_sum(d, 7)));
            }
        }
    } else {
        let months = range_days / 28;
        for m in 0..months {
            if let Some(d) = end_date.checked_sub_signed(Duration::days(28 * m)) {
                points.push((d, bucket_sum(d, 28)));
            }
        }
    }
    points.sort_by_key(|p| p.0);
    points
        .into_iter()
        .map(|(d, c)| GraphPoint { label: d.format("%b %-d").to_string(), count: c })
        .collect()
}

async fn top_by_column(
    pool: &SqlitePool,
    property_id: &Uuid,
    start_ms: i64,
    end_ms: i64,
    filter_url: Option<&str>,
    column: &str,
    event: Option<&str>,
    limit: i64,
    distinct_users: bool,
) -> Vec<LabelCount> {
    // For user-property breakdowns (device/browser/platform), count one row
    // per anonymous user_id. For everything else, count raw events.
    let count_expr = if distinct_users {
        "COUNT(DISTINCT user_id)"
    } else {
        "COUNT(*)"
    };
    let mut sql = format!(
        "SELECT {col}, {cnt} FROM events \
         WHERE property_id = ? AND created_at >= ? AND created_at <= ? \
               AND {col} IS NOT NULL AND {col} != ''",
        col = column,
        cnt = count_expr,
    );
    if distinct_users {
        sql.push_str(" AND user_id IS NOT NULL");
    }
    if event.is_some() {
        sql.push_str(" AND event = ?");
    }
    let (extra_sql, extra_bind) = filter_clause(filter_url);
    sql.push_str(extra_sql);
    sql.push_str(&format!(" GROUP BY {col} ORDER BY {cnt} DESC LIMIT ?", col = column, cnt = count_expr));

    let mut q = sqlx::query_as::<_, (String, i64)>(&sql)
        .bind(property_id.as_bytes().to_vec())
        .bind(start_ms)
        .bind(end_ms);
    if let Some(e) = event {
        q = q.bind(e);
    }
    if let Some(v) = extra_bind {
        q = q.bind(v);
    }
    q = q.bind(limit);
    let rows = q.fetch_all(pool).await.unwrap_or_default();
    rows.into_iter()
        .map(|(label, count)| LabelCount { label, count })
        .collect()
}

pub async fn events_by_screen_size(
    pool: &SqlitePool,
    property_id: &Uuid,
    start_ms: i64,
    end_ms: i64,
    filter_url: Option<&str>,
    limit: i64,
) -> Vec<LabelCount> {
    // Counts unique anonymous users (cookie-based user_id) per screen size,
    // not raw events. Filtered to page_view so returning visitors are counted
    // — the collectoruserid cookie suppresses session_start after the first
    // visit, but page_view always fires.
    let mut sql = String::from(
        "SELECT screen_width, screen_height, COUNT(DISTINCT user_id) FROM events \
         WHERE property_id = ? AND created_at >= ? AND created_at <= ? \
               AND event = 'page_view' \
               AND screen_width IS NOT NULL \
               AND user_id IS NOT NULL",
    );
    let (extra_sql, extra_bind) = filter_clause(filter_url);
    sql.push_str(extra_sql);
    sql.push_str(" GROUP BY screen_width, screen_height ORDER BY COUNT(DISTINCT user_id) DESC LIMIT ?");
    let mut q = sqlx::query_as::<_, (Option<i64>, Option<i64>, i64)>(&sql)
        .bind(property_id.as_bytes().to_vec())
        .bind(start_ms)
        .bind(end_ms);
    if let Some(v) = extra_bind {
        q = q.bind(v);
    }
    q = q.bind(limit);
    q.fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(w, h, c)| LabelCount {
            label: format!("{}x{}", w.unwrap_or(0), h.unwrap_or(0)),
            count: c,
        })
        .collect()
}

// Device/browser/platform breakdowns filter on page_view (not session_start)
// so they populate for returning visitors too. Server-side UA parsing fills
// these columns on every event, so the data is always present.
pub async fn events_by_device(pool: &SqlitePool, property_id: &Uuid, start_ms: i64, end_ms: i64, filter_url: Option<&str>, limit: i64) -> Vec<LabelCount> {
    top_by_column(pool, property_id, start_ms, end_ms, filter_url, "device", Some("page_view"), limit, true).await
}
pub async fn events_by_browser(pool: &SqlitePool, property_id: &Uuid, start_ms: i64, end_ms: i64, filter_url: Option<&str>, limit: i64) -> Vec<LabelCount> {
    top_by_column(pool, property_id, start_ms, end_ms, filter_url, "browser", Some("page_view"), limit, true).await
}
pub async fn events_by_platform(pool: &SqlitePool, property_id: &Uuid, start_ms: i64, end_ms: i64, filter_url: Option<&str>, limit: i64) -> Vec<LabelCount> {
    top_by_column(pool, property_id, start_ms, end_ms, filter_url, "platform", Some("page_view"), limit, true).await
}
pub async fn events_by_page_url(pool: &SqlitePool, property_id: &Uuid, start_ms: i64, end_ms: i64, filter_url: Option<&str>, limit: i64) -> Vec<LabelCount> {
    top_by_column(pool, property_id, start_ms, end_ms, filter_url, "url", None, limit, false).await
}
pub async fn page_views_by_page_url(pool: &SqlitePool, property_id: &Uuid, start_ms: i64, end_ms: i64, filter_url: Option<&str>, limit: i64) -> Vec<LabelCount> {
    top_by_column(pool, property_id, start_ms, end_ms, filter_url, "url", Some("page_view"), limit, false).await
}
pub async fn session_starts_by_referrer(pool: &SqlitePool, property_id: &Uuid, start_ms: i64, end_ms: i64, filter_url: Option<&str>, limit: i64) -> Vec<LabelCount> {
    top_by_column(pool, property_id, start_ms, end_ms, filter_url, "referrer", Some("session_start"), limit, false).await
}

pub async fn page_views_by_utm(
    pool: &SqlitePool,
    property_id: &Uuid,
    start_ms: i64,
    end_ms: i64,
    filter_url: Option<&str>,
    field: &str,
    limit: i64,
) -> Vec<LabelCount> {
    let column = match field {
        "source" => "utm_source",
        "medium" => "utm_medium",
        "campaign" => "utm_campaign",
        "term" => "utm_term",
        "content" => "utm_content",
        _ => return Vec::new(),
    };
    top_by_column(pool, property_id, start_ms, end_ms, filter_url, column, Some("page_view"), limit, false).await
}

pub async fn events_by_custom_event(
    pool: &SqlitePool,
    property_id: &Uuid,
    start_ms: i64,
    end_ms: i64,
    filter_url: Option<&str>,
    limit: i64,
) -> Vec<LabelCount> {
    let placeholders = std::iter::repeat("?")
        .take(BUILT_IN_EVENTS.len())
        .collect::<Vec<_>>()
        .join(",");
    let (extra_sql, extra_bind) = filter_clause(filter_url);
    let sql = format!(
        "SELECT event, COUNT(*) FROM events \
         WHERE property_id = ? AND created_at >= ? AND created_at <= ? \
               AND event NOT IN ({}){} \
         GROUP BY event ORDER BY COUNT(*) DESC LIMIT ?",
        placeholders, extra_sql
    );
    let mut q = sqlx::query_as::<_, (String, i64)>(&sql)
        .bind(property_id.as_bytes().to_vec())
        .bind(start_ms)
        .bind(end_ms);
    for built in BUILT_IN_EVENTS {
        q = q.bind(built);
    }
    if let Some(v) = extra_bind {
        q = q.bind(v);
    }
    q = q.bind(limit);
    q.fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(label, count)| LabelCount { label, count })
        .collect()
}

pub async fn session_starts_by_country(
    pool: &SqlitePool,
    property_id: &Uuid,
    start_ms: i64,
    end_ms: i64,
    filter_url: Option<&str>,
) -> std::collections::HashMap<String, i64> {
    let (extra_sql, extra_bind) = filter_clause(filter_url);
    let sql = format!(
        "SELECT country, COUNT(*) FROM events \
         WHERE property_id = ? AND created_at >= ? AND created_at <= ? \
               AND event = 'session_start' AND country IS NOT NULL{} \
         GROUP BY country",
        extra_sql
    );
    let mut q = sqlx::query_as::<_, (String, i64)>(&sql)
        .bind(property_id.as_bytes().to_vec())
        .bind(start_ms)
        .bind(end_ms);
    if let Some(v) = extra_bind {
        q = q.bind(v);
    }
    q.fetch_all(pool).await.unwrap_or_default().into_iter().collect()
}

pub async fn session_starts_by_country_region(
    pool: &SqlitePool,
    property_id: &Uuid,
    start_ms: i64,
    end_ms: i64,
    filter_url: Option<&str>,
) -> std::collections::HashMap<String, std::collections::HashMap<String, i64>> {
    let (extra_sql, extra_bind) = filter_clause(filter_url);
    let sql = format!(
        "SELECT country, region, COUNT(*) FROM events \
         WHERE property_id = ? AND created_at >= ? AND created_at <= ? \
               AND event = 'session_start' \
               AND country IS NOT NULL AND region IS NOT NULL{} \
         GROUP BY country, region",
        extra_sql
    );
    let mut q = sqlx::query_as::<_, (String, String, i64)>(&sql)
        .bind(property_id.as_bytes().to_vec())
        .bind(start_ms)
        .bind(end_ms);
    if let Some(v) = extra_bind {
        q = q.bind(v);
    }
    let rows = q.fetch_all(pool).await.unwrap_or_default();
    let mut out: std::collections::HashMap<String, std::collections::HashMap<String, i64>> =
        std::collections::HashMap::new();
    for (country, region, count) in rows {
        out.entry(country).or_default().insert(region, count);
    }
    out
}

pub async fn bot_traffic(
    pool: &SqlitePool,
    property_id: &Uuid,
    start_ms: i64,
    end_ms: i64,
    limit: i64,
) -> BotTraffic {
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bot_events \
         WHERE property_id = ? AND created_at >= ? AND created_at <= ?",
    )
    .bind(property_id.as_bytes().to_vec())
    .bind(start_ms)
    .bind(end_ms)
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    if total == 0 {
        return BotTraffic::default();
    }
    let top_bots = sqlx::query_as::<_, (String, i64)>(
        "SELECT bot_name, COUNT(*) FROM bot_events \
         WHERE property_id = ? AND created_at >= ? AND created_at <= ? \
               AND bot_name IS NOT NULL AND bot_name != '' \
         GROUP BY bot_name ORDER BY COUNT(*) DESC LIMIT ?",
    )
    .bind(property_id.as_bytes().to_vec())
    .bind(start_ms)
    .bind(end_ms)
    .bind(limit)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(label, count)| LabelCount { label, count })
    .collect();
    let top_pages = sqlx::query_as::<_, (String, i64)>(
        "SELECT url, COUNT(*) FROM bot_events \
         WHERE property_id = ? AND created_at >= ? AND created_at <= ? \
               AND url IS NOT NULL AND url != '' \
         GROUP BY url ORDER BY COUNT(*) DESC LIMIT ?",
    )
    .bind(property_id.as_bytes().to_vec())
    .bind(start_ms)
    .bind(end_ms)
    .bind(limit)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(label, count)| LabelCount { label, count })
    .collect();
    BotTraffic { total, top_bots, top_pages }
}

/// Convert "YYYY-MM-DD" + a time-of-day to a unix-ms timestamp in the local tz.
pub fn parse_date_to_ms(date: &str, end_of_day: bool) -> Option<i64> {
    let nd = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let nt = if end_of_day {
        chrono::NaiveTime::from_hms_opt(23, 59, 59)?
    } else {
        chrono::NaiveTime::from_hms_opt(0, 0, 0)?
    };
    let local: DateTime<chrono::Local> = chrono::Local
        .from_local_datetime(&chrono::NaiveDateTime::new(nd, nt))
        .single()?;
    Some(local.with_timezone(&Utc).timestamp_millis())
}
