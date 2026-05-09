use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use tower_cookies::Cookies;
use uuid::Uuid;

use crate::render::{render, render_to_string};
use crate::routes::auth::is_authenticated;
use crate::AppState;

// Milliseconds per day. Used to convert between the date-range query
// parameter (in days) and the millisecond timestamps stored in events.
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
// Default look-back when the request omits ?date_range= and gives a custom
// start/end. Matches what the dashboard's date selector picks by default.
const DEFAULT_DATE_RANGE_DAYS: i64 = 28;

pub fn router() -> Router<AppState> {
    // The dashboard's UUID path segment is a catch-all; merging this module
    // last in app::router keeps named routes (e.g. /login, /properties)
    // winning the match (axum prefers literal segments over path parameters
    // at the same depth).
    Router::new().route("/{property_id}", get(property))
}

#[derive(Debug, Deserialize)]
pub struct DashboardQuery {
    pub date_start: Option<String>,
    pub date_end: Option<String>,
    pub date_range: Option<String>,
    pub filter_url: Option<String>,
    pub report: Option<String>,
}

pub async fn property(
    State(state): State<AppState>,
    AxumPath(property_id): AxumPath<Uuid>,
    cookies: Cookies,
    Query(q): Query<DashboardQuery>,
) -> Response {
    let row: Option<crate::models::PropertyRow> = sqlx::query_as(
        "SELECT id, name, custom_cards, is_protected, is_public, created_at, updated_at \
         FROM properties WHERE id = ?",
    )
    .bind(property_id.as_bytes().to_vec())
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);
    let Some(row) = row else {
        return Redirect::to("/properties").into_response();
    };
    let p = row.into_property();
    let authed = is_authenticated(&cookies, &state);
    if !p.is_public && !authed {
        return Redirect::to("/login").into_response();
    }

    use chrono::{Duration, Local};

    let today = Local::now().date_naive();
    let default_start = today - Duration::days(DEFAULT_DATE_RANGE_DAYS);
    let date_start = q
        .date_start
        .clone()
        .unwrap_or_else(|| default_start.format("%Y-%m-%d").to_string());
    let date_end = q
        .date_end
        .clone()
        .unwrap_or_else(|| today.format("%Y-%m-%d").to_string());

    let start_ms = match crate::queries::parse_date_to_ms(&date_start, false) {
        Some(v) => v,
        None => return (StatusCode::BAD_REQUEST, "bad date_start").into_response(),
    };
    let end_ms = match crate::queries::parse_date_to_ms(&date_end, true) {
        Some(v) => v,
        None => return (StatusCode::BAD_REQUEST, "bad date_end").into_response(),
    };

    let date_range: i64 = match q.date_range.as_deref() {
        Some("custom") | None => {
            // Days between start and end, inclusive of the end-of-day window.
            let span = (end_ms - start_ms) / DAY_MS;
            span.max(1)
        }
        Some(other) => other.parse::<i64>().unwrap_or(DEFAULT_DATE_RANGE_DAYS),
    };

    let prev_start_ms = start_ms - date_range * DAY_MS;
    let prev_end_ms = end_ms - date_range * DAY_MS;
    let filter_url = q.filter_url.as_deref().filter(|s| !s.is_empty());

    let dash_value: serde_json::Value = {
        let pool = &state.pool;
        let pid = &p.id;

        let event_cards =
            crate::queries::standard_event_cards(pool, pid, start_ms, end_ms, prev_start_ms, prev_end_ms, filter_url).await;
        let (custom_cards, custom_events) = crate::queries::custom_event_cards(
            pool, pid, &p.custom_cards, start_ms, end_ms, prev_start_ms, prev_end_ms, filter_url,
        )
        .await;
        let mut all_cards = event_cards;
        all_cards.extend(custom_cards);

        let total_events_graph = crate::queries::events_graph(
            pool, pid, start_ms, end_ms, filter_url, today, date_range,
        )
        .await;

        let total_events_by_screen_size =
            crate::queries::events_by_screen_size(pool, pid, start_ms, end_ms, filter_url, 7).await;
        let total_events_by_device =
            crate::queries::events_by_device(pool, pid, start_ms, end_ms, filter_url, 7).await;
        let total_events_by_browser =
            crate::queries::events_by_browser(pool, pid, start_ms, end_ms, filter_url, 7).await;
        let total_events_by_platform =
            crate::queries::events_by_platform(pool, pid, start_ms, end_ms, filter_url, 7).await;
        let total_events_by_page_url =
            crate::queries::events_by_page_url(pool, pid, start_ms, end_ms, filter_url, 10).await;
        let total_page_views_by_page_url =
            crate::queries::page_views_by_page_url(pool, pid, start_ms, end_ms, filter_url, 10).await;
        let total_events_by_custom_event =
            crate::queries::events_by_custom_event(pool, pid, start_ms, end_ms, filter_url, 10).await;
        let total_session_starts_by_referrer =
            crate::queries::session_starts_by_referrer(pool, pid, start_ms, end_ms, filter_url, 10).await;
        let total_page_views_by_utm_medium =
            crate::queries::page_views_by_utm(pool, pid, start_ms, end_ms, filter_url, "medium", 10).await;
        let total_page_views_by_utm_source =
            crate::queries::page_views_by_utm(pool, pid, start_ms, end_ms, filter_url, "source", 10).await;
        let total_page_views_by_utm_campaign =
            crate::queries::page_views_by_utm(pool, pid, start_ms, end_ms, filter_url, "campaign", 10).await;
        let session_starts_by_country =
            crate::queries::session_starts_by_country(pool, pid, start_ms, end_ms, filter_url).await;
        let session_starts_by_country_region =
            crate::queries::session_starts_by_country_region(pool, pid, start_ms, end_ms, filter_url).await;
        let bot_traffic =
            crate::queries::bot_traffic(pool, pid, start_ms, end_ms, 10).await;

        serde_json::json!({
            "event_cards": all_cards,
            "custom_events": custom_events,
            "total_events_graph": total_events_graph,
            "total_events_by_screen_size": total_events_by_screen_size,
            "total_events_by_device": total_events_by_device,
            "total_events_by_browser": total_events_by_browser,
            "total_events_by_platform": total_events_by_platform,
            "total_events_by_page_url": total_events_by_page_url,
            "total_page_views_by_page_url": total_page_views_by_page_url,
            "total_events_by_custom_event": total_events_by_custom_event,
            "total_session_starts_by_referrer": total_session_starts_by_referrer,
            "total_page_views_by_utm_medium": total_page_views_by_utm_medium,
            "total_page_views_by_utm_source": total_page_views_by_utm_source,
            "total_page_views_by_utm_campaign": total_page_views_by_utm_campaign,
            "session_starts_by_country": session_starts_by_country,
            "session_starts_by_country_region": session_starts_by_country_region,
            "bot_traffic": bot_traffic,
        })
    };

    let total_live_users = crate::queries::total_live_users(&state.pool, &p.id).await;

    // Build the small chart helpers + breakdown totals the print template needs.
    let chart_polyline = build_chart_polyline(
        dash_value
            .get("total_events_graph")
            .and_then(|v| v.as_array())
            .map(|v| v.as_slice())
            .unwrap_or(&[]),
    );
    let graph_arr = dash_value
        .get("total_events_graph")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let chart_label_start = graph_arr
        .first()
        .and_then(|p| p.get("label"))
        .and_then(|l| l.as_str())
        .unwrap_or("")
        .to_string();
    let chart_label_end = graph_arr
        .last()
        .and_then(|p| p.get("label"))
        .and_then(|l| l.as_str())
        .unwrap_or("")
        .to_string();
    let (chart_peak_count, chart_peak_label) = graph_arr
        .iter()
        .max_by_key(|p| p.get("count").and_then(|c| c.as_i64()).unwrap_or(0))
        .map(|p| {
            (
                p.get("count").and_then(|c| c.as_i64()).unwrap_or(0),
                p.get("label").and_then(|l| l.as_str()).unwrap_or("").to_string(),
            )
        })
        .unwrap_or((0, String::new()));

    let breakdown_total = |key: &str| -> i64 {
        dash_value
            .get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.get("count").and_then(|c| c.as_i64()))
                    .sum::<i64>()
                    .max(1)
            })
            .unwrap_or(1)
    };
    let breakdown_totals = serde_json::json!({
        "device": breakdown_total("total_events_by_device"),
        "browser": breakdown_total("total_events_by_browser"),
        "platform": breakdown_total("total_events_by_platform"),
        "screen_size": breakdown_total("total_events_by_screen_size"),
    });

    let mut top_countries: Vec<serde_json::Value> = dash_value
        .get("session_starts_by_country")
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .map(|(k, v)| {
                    serde_json::json!({
                        "label": k,
                        "count": v.as_i64().unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    top_countries.sort_by_key(|v| -v.get("count").and_then(|c| c.as_i64()).unwrap_or(0));
    top_countries.truncate(10);

    let generated_at = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();

    let extra = minijinja::context! {
        page => minijinja::context! {
            title => &p.name,
            description => format!("Analytics for {}", p.name),
        },
        property => minijinja::context! {
            id => p.id.to_string(),
            name => &p.name,
            is_protected => p.is_protected,
            is_public => p.is_public,
        },
        date_start => &date_start,
        date_end => &date_end,
        date_range => date_range,
        filter_url => filter_url,
        total_live_users => total_live_users,
        event_cards => dash_value.get("event_cards").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        custom_events => dash_value.get("custom_events").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        total_events_graph => dash_value.get("total_events_graph").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        total_events_by_screen_size => dash_value.get("total_events_by_screen_size").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        total_events_by_device => dash_value.get("total_events_by_device").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        total_events_by_browser => dash_value.get("total_events_by_browser").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        total_events_by_platform => dash_value.get("total_events_by_platform").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        total_events_by_page_url => dash_value.get("total_events_by_page_url").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        total_page_views_by_page_url => dash_value.get("total_page_views_by_page_url").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        total_events_by_custom_event => dash_value.get("total_events_by_custom_event").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        total_session_starts_by_referrer => dash_value.get("total_session_starts_by_referrer").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        total_page_views_by_utm_medium => dash_value.get("total_page_views_by_utm_medium").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        total_page_views_by_utm_source => dash_value.get("total_page_views_by_utm_source").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        total_page_views_by_utm_campaign => dash_value.get("total_page_views_by_utm_campaign").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        session_starts_by_country => dash_value.get("session_starts_by_country").cloned().unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
        session_starts_by_country_region => dash_value.get("session_starts_by_country_region").cloned().unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
        bot_traffic => dash_value.get("bot_traffic").cloned().unwrap_or(serde_json::json!({"total": 0, "top_bots": [], "top_pages": []})),
        chart_polyline => &chart_polyline,
        chart_label_start => &chart_label_start,
        chart_label_end => &chart_label_end,
        chart_peak_count => chart_peak_count,
        chart_peak_label => &chart_peak_label,
        breakdown_totals => &breakdown_totals,
        top_countries => &top_countries,
        generated_at => &generated_at,
    };

    // Report exports.
    if let Some(fmt) = q.report.as_deref() {
        let fmt = if fmt.is_empty() { "pdf" } else { fmt };
        let path = format!("/{property_id}");
        if fmt == "md" {
            let body = match render_to_string(
                &state,
                "properties/property_report.md",
                &path,
                authed,
                extra,
            ) {
                Ok(b) => b,
                Err(resp) => return resp,
            };
            let mut h = axum::http::HeaderMap::new();
            h.insert(
                axum::http::header::CONTENT_TYPE,
                "text/markdown; charset=utf-8".parse().unwrap(),
            );
            h.insert(
                axum::http::header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{}.md\"", p.name).parse().unwrap(),
            );
            return (StatusCode::OK, h, body).into_response();
        }
        if fmt == "pdf" {
            let html = match render_to_string(
                &state,
                "properties/property_print.html",
                &path,
                authed,
                extra,
            ) {
                Ok(b) => b,
                Err(resp) => return resp,
            };
            let server_base = state.config.base_url.clone();
            let pdf_res =
                tokio::task::spawn_blocking(move || crate::pdf::html_to_pdf(&html, &server_base))
                    .await;
            match pdf_res {
                Ok(Ok(bytes)) => {
                    let mut h = axum::http::HeaderMap::new();
                    h.insert(axum::http::header::CONTENT_TYPE, "application/pdf".parse().unwrap());
                    h.insert(
                        axum::http::header::CONTENT_DISPOSITION,
                        format!("inline; filename=\"{}.pdf\"", p.name).parse().unwrap(),
                    );
                    return (StatusCode::OK, h, bytes).into_response();
                }
                Ok(Err(e)) => {
                    tracing::error!("pdf render: {e}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, "pdf error").into_response();
                }
                Err(e) => {
                    tracing::error!("pdf join: {e}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, "pdf error").into_response();
                }
            }
        }
    }

    render(
        &state,
        "properties/property.html",
        &format!("/{property_id}"),
        authed,
        extra,
    )
}

/// Toner-friendly SVG polyline points for the print template. `width`,
/// `height`, and `padding` match the SVG viewBox in
/// templates/properties/property_print.html — change one and change the other.
fn build_chart_polyline(points: &[serde_json::Value]) -> String {
    if points.is_empty() {
        return String::new();
    }
    let counts: Vec<i64> = points
        .iter()
        .map(|p| p.get("count").and_then(|c| c.as_i64()).unwrap_or(0))
        .collect();
    let max = *counts.iter().max().unwrap_or(&1);
    let max = if max == 0 { 1 } else { max };
    let n = counts.len();
    let width = 600.0_f64;
    let height = 100.0_f64;
    let padding = 4.0_f64;
    let usable_h = height - 2.0 * padding;
    if n == 1 {
        let x = width / 2.0;
        let y = height - padding - (counts[0] as f64 / max as f64) * usable_h;
        return format!("{x:.1},{y:.1}");
    }
    counts
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let x = (i as f64 / (n - 1) as f64) * width;
            let y = height - padding - (*c as f64 / max as f64) * usable_h;
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}
