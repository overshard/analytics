use axum::{
    extract::{Form, Path as AxumPath, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Redirect, Response},
};
use chrono::Datelike;
use serde::Deserialize;
use serde_json::json;
use tower_cookies::Cookies;
use uuid::Uuid;

use crate::auth::is_authenticated;
use crate::templates::{RequestCtx, UserCtx};
use crate::AppState;

fn now_year() -> i32 {
    use chrono::Datelike;
    chrono::Local::now().year()
}

fn render(
    state: &AppState,
    template: &str,
    extra: minijinja::Value,
    authed: bool,
    path: &str,
) -> Result<Html<String>, StatusCode> {
    let tmpl = state
        .env
        .get_template(template)
        .map_err(|e| {
            tracing::error!("template '{}': {}", template, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let request = RequestCtx {
        url: String::new(),
        url_root: "/".to_string(),
        base_url: String::new(),
        path: path.to_string(),
    };
    let body = tmpl
        .render(minijinja::context! {
            user => UserCtx { is_authenticated: authed },
            request => &request,
            now => minijinja::context! { year => now_year() },
            base_url => &state.config.base_url,
            collector_id => state.config.proprium_id.map(|u| u.to_string()),
            collector_server => &state.config.base_url,
            messages => Vec::<()>::new(),
            ..extra
        })
        .map_err(|e| {
            tracing::error!("render '{}': {}", template, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Html(body))
}

#[derive(Debug, Deserialize)]
pub struct PropertiesQuery {
    #[serde(default)]
    pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PropertyCreateForm {
    pub name: String,
}

pub async fn properties(
    State(state): State<AppState>,
    cookies: Cookies,
    Query(q): Query<PropertiesQuery>,
) -> Response {
    if !is_authenticated(&cookies, &state) {
        return Redirect::to("/login").into_response();
    }
    let search = q.q.as_deref().unwrap_or("").trim().to_string();

    let rows = if search.is_empty() {
        sqlx::query_as::<_, crate::models::PropertyRow>(
            "SELECT id, name, custom_cards, is_protected, is_public, created_at, updated_at \
             FROM properties ORDER BY is_protected DESC, created_at ASC",
        )
        .fetch_all(&state.pool)
        .await
    } else {
        let like = format!("%{}%", search);
        sqlx::query_as::<_, crate::models::PropertyRow>(
            "SELECT id, name, custom_cards, is_protected, is_public, created_at, updated_at \
             FROM properties WHERE name LIKE ? ORDER BY is_protected DESC, created_at ASC",
        )
        .bind(like)
        .fetch_all(&state.pool)
        .await
    };

    let rows = match rows {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("properties query: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
        }
    };

    let mut props = Vec::with_capacity(rows.len());
    let mut total_events = 0i64;
    let mut total_page_views = 0i64;
    let mut total_sessions = 0i64;

    for row in rows {
        let id_bytes = row.id.clone();
        let p = row.into_property();

        let counts: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT \
                (SELECT COUNT(*) FROM events WHERE property_id = ?1) AS total, \
                (SELECT COUNT(*) FROM events WHERE property_id = ?1 AND event = 'page_view') AS pv, \
                (SELECT COUNT(*) FROM events WHERE property_id = ?1 AND event = 'session_start') AS ss, \
                (SELECT COUNT(*) FROM events WHERE property_id = ?1 AND created_at >= ?2) AS active",
        )
        .bind(&id_bytes)
        .bind(chrono::Utc::now().timestamp_millis() - 7 * 24 * 60 * 60 * 1000)
        .fetch_one(&state.pool)
        .await
        .unwrap_or((0, 0, 0, 0));

        total_events += counts.0;
        total_page_views += counts.1;
        total_sessions += counts.2;

        props.push(json!({
            "id": p.id.to_string(),
            "name": p.name,
            "is_protected": p.is_protected,
            "is_public": p.is_public,
            "is_active": counts.3 > 0,
            "total_events": counts.0,
            "total_page_views": counts.1,
            "total_session_starts": counts.2,
        }));
    }

    let totals = json!({
        "properties": props.len(),
        "events": total_events,
        "page_views": total_page_views,
        "sessions": total_sessions,
    });

    let extra = minijinja::context! {
        page => minijinja::context! {
            title => "Properties",
            description => "Manage your properties.",
        },
        properties => &props,
        totals => &totals,
        q => &search,
    };

    render(&state, "properties/properties.html", extra, true, "/properties")
        .map(IntoResponse::into_response)
        .unwrap_or_else(|e| e.into_response())
}

pub async fn properties_create(
    State(state): State<AppState>,
    cookies: Cookies,
    Form(form): Form<PropertyCreateForm>,
) -> Response {
    if !is_authenticated(&cookies, &state) {
        return Redirect::to("/login").into_response();
    }
    let name = form.name.trim();
    if name.is_empty() {
        return Redirect::to("/properties").into_response();
    }
    let id = Uuid::new_v4();
    let now = chrono::Utc::now().timestamp_millis();
    let res = sqlx::query(
        "INSERT INTO properties (id, name, custom_cards, is_protected, is_public, created_at, updated_at) \
         VALUES (?, ?, '[]', 0, 0, ?, ?)",
    )
    .bind(id.as_bytes().to_vec())
    .bind(name)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await;
    if let Err(e) = res {
        tracing::error!("create property: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
    }
    Redirect::to("/properties").into_response()
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
    let default_start = today - Duration::days(28);
    let date_start = q.date_start.clone().unwrap_or_else(|| default_start.format("%Y-%m-%d").to_string());
    let date_end = q.date_end.clone().unwrap_or_else(|| today.format("%Y-%m-%d").to_string());

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
            let span = (end_ms - start_ms) / (24 * 60 * 60 * 1000);
            span.max(1)
        }
        Some(other) => other.parse::<i64>().unwrap_or(28),
    };

    let prev_start_ms = start_ms - date_range * 24 * 60 * 60 * 1000;
    let prev_end_ms = end_ms - date_range * 24 * 60 * 60 * 1000;
    let filter_url = q.filter_url.as_deref().filter(|s| !s.is_empty());

    // Cache key includes property updated_at so card/visibility edits bust it.
    let cache_key = format!(
        "dash:{}:{}:{}:{}:{}:{}",
        p.id,
        p.updated_at,
        date_start,
        date_end,
        date_range,
        filter_url.unwrap_or("")
    );
    let bypass_cache = q.report.is_some();
    let cached = if bypass_cache { None } else { state.cache.get(&cache_key).await };

    let dash_value: serde_json::Value = if let Some(arc) = cached {
        (*arc).clone()
    } else {
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

        let v = serde_json::json!({
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
        });
        if !bypass_cache {
            state
                .cache
                .insert(cache_key.clone(), std::sync::Arc::new(v.clone()))
                .await;
        }
        v
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
        if fmt == "md" {
            let tmpl = match state.env.get_template("properties/property_report.md") {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("template property_report.md: {e}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response();
                }
            };
            let body = match tmpl.render(minijinja::context! {
                user => crate::templates::UserCtx { is_authenticated: authed },
                request => crate::templates::RequestCtx {
                    url: String::new(),
                    url_root: "/".to_string(),
                    base_url: String::new(),
                    path: format!("/{property_id}"),
                },
                now => minijinja::context! { year => chrono::Local::now().year() },
                base_url => &state.config.base_url,
                collector_id => state.config.proprium_id.map(|u| u.to_string()),
                collector_server => &state.config.base_url,
                messages => Vec::<()>::new(),
                ..extra
            }) {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!("render md: {e}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, "render error").into_response();
                }
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
            let tmpl = match state.env.get_template("properties/property_print.html") {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("template property_print.html: {e}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response();
                }
            };
            let html = match tmpl.render(minijinja::context! {
                user => crate::templates::UserCtx { is_authenticated: authed },
                request => crate::templates::RequestCtx {
                    url: String::new(),
                    url_root: "/".to_string(),
                    base_url: String::new(),
                    path: format!("/{property_id}"),
                },
                now => minijinja::context! { year => chrono::Local::now().year() },
                base_url => &state.config.base_url,
                collector_id => state.config.proprium_id.map(|u| u.to_string()),
                collector_server => &state.config.base_url,
                messages => Vec::<()>::new(),
                ..extra
            }) {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!("render print: {e}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, "render error").into_response();
                }
            };
            let server_base = if state.config.base_url.is_empty() {
                String::new()
            } else {
                state.config.base_url.clone()
            };
            let pdf_res = tokio::task::spawn_blocking(move || crate::pdf::html_to_pdf(&html, &server_base)).await;
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
        extra,
        authed,
        &format!("/{property_id}"),
    )
    .map(IntoResponse::into_response)
    .unwrap_or_else(|e| e.into_response())
}

/// Toner-friendly SVG polyline points for the print template.
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

pub async fn property_delete(
    State(state): State<AppState>,
    AxumPath(property_id): AxumPath<Uuid>,
    cookies: Cookies,
) -> Response {
    if !is_authenticated(&cookies, &state) {
        return Redirect::to("/login").into_response();
    }
    let _ = sqlx::query("DELETE FROM properties WHERE id = ? AND is_protected = 0")
        .bind(property_id.as_bytes().to_vec())
        .execute(&state.pool)
        .await;
    Redirect::to("/properties").into_response()
}

pub async fn property_cards(
    State(state): State<AppState>,
    AxumPath(property_id): AxumPath<Uuid>,
    cookies: Cookies,
    body: String,
) -> Response {
    if !is_authenticated(&cookies, &state) {
        return Redirect::to("/login").into_response();
    }
    // Body is the raw JSON array of {event,value} objects.
    let parsed: serde_json::Value =
        serde_json::from_str(&body).unwrap_or(serde_json::json!([]));
    let payload = parsed.to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let _ = sqlx::query("UPDATE properties SET custom_cards = ?, updated_at = ? WHERE id = ?")
        .bind(payload)
        .bind(now)
        .bind(property_id.as_bytes().to_vec())
        .execute(&state.pool)
        .await;
    Json(serde_json::json!({"success": true})).into_response()
}

pub async fn property_public_toggle(
    State(state): State<AppState>,
    AxumPath(property_id): AxumPath<Uuid>,
    cookies: Cookies,
) -> Response {
    if !is_authenticated(&cookies, &state) {
        return Redirect::to("/login").into_response();
    }
    let now = chrono::Utc::now().timestamp_millis();
    let _ = sqlx::query("UPDATE properties SET is_public = 1 - is_public, updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(property_id.as_bytes().to_vec())
        .execute(&state.pool)
        .await;
    Json(serde_json::json!({"success": true})).into_response()
}

