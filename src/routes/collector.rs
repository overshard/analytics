use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde_json::Value;
use std::net::IpAddr;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

use crate::AppState;

pub fn router() -> Router<AppState> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let collect_routes = Router::new()
        .route("/collect", post(collect).options(options))
        // /collect/ is a compatibility alias for embeds that hardcoded the
        // trailing slash. Keep it pointing at the same handlers.
        .route("/collect/", post(collect).options(options))
        .layer(cors);

    let alias_routes = Router::new()
        // Stable URL for the collector embed script. Vite content-hashes the
        // entry, but every embed snippet in the wild hardcodes
        // /static/collector.js. This aliased handler reads the manifest and
        // serves the hashed asset by that stable path. CORS does not apply
        // here; same-origin browsers fetch the script directly.
        .route("/static/collector.js", get(collector_alias));

    Router::new().merge(collect_routes).merge(alias_routes)
}

#[derive(Debug, Deserialize)]
struct CollectBody {
    #[serde(rename = "collectorId", alias = "collector_id")]
    collector_id: Option<String>,
    event: Option<String>,
    #[serde(default)]
    data: Value,
}

pub async fn options(headers: HeaderMap) -> Response {
    let mut h = HeaderMap::new();
    let allow_origin = headers
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("*")
        .to_string();
    let allow_headers = headers
        .get("access-control-request-headers")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Content-Type")
        .to_string();
    h.insert("allow", "OPTIONS, POST".parse().unwrap());
    h.insert("access-control-allow-methods", "OPTIONS, POST".parse().unwrap());
    h.insert("access-control-allow-headers", allow_headers.parse().unwrap());
    h.insert("access-control-allow-origin", allow_origin.parse().unwrap());
    (StatusCode::NO_CONTENT, h).into_response()
}

pub async fn collect(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Response {
    if body.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let parsed: CollectBody = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let Some(collector_id) = parsed.collector_id.as_deref() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(event_name) = parsed.event.as_deref() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(property_id) = Uuid::parse_str(collector_id) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    // Confirm property exists.
    let exists: Option<(Vec<u8>,)> =
        sqlx::query_as("SELECT id FROM properties WHERE id = ?")
            .bind(property_id.as_bytes().to_vec())
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);
    if exists.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let mut data = if parsed.data.is_object() {
        parsed.data.as_object().unwrap().clone()
    } else {
        serde_json::Map::new()
    };

    // Normalize referrer to bare hostname.
    if let Some(r) = data.get("referrer").and_then(|v| v.as_str()) {
        let host = r.split("://").last().unwrap_or(r);
        let host = host.split('/').next().unwrap_or("");
        let host = host.to_ascii_lowercase().trim_start_matches("www.").to_string();
        data.insert("referrer".to_string(), Value::String(host));
    }

    // GeoIP enrichment for session_start.
    if event_name == "session_start" {
        if let Some(ip) = client_ip(&headers) {
            if !ip.is_loopback() {
                if let Some(g) = state.geoip.lookup(ip) {
                    if let Some(c) = g.country {
                        data.insert("country".to_string(), Value::String(c));
                    }
                    if let Some(r) = g.region {
                        data.insert("region".to_string(), Value::String(r));
                    }
                    if let Some(c) = g.city {
                        data.insert("city".to_string(), Value::String(c));
                    }
                    if let (Some(lat), Some(lon)) = (g.lat, g.lon) {
                        data.insert(
                            "loc".to_string(),
                            Value::Array(vec![
                                serde_json::json!(lat),
                                serde_json::json!(lon),
                            ]),
                        );
                    }
                }
            }
        }
    }

    // UA parsing.
    let ua_string = data
        .get("user_agent")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            headers
                .get(header::USER_AGENT)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        });
    if let Some(ua) = &ua_string {
        let parsed_ua = state.ua.parse(ua);
        if let Some(p) = parsed_ua.platform.clone() {
            data.insert("platform".to_string(), Value::String(p));
        }
        if let Some(b) = parsed_ua.browser.clone() {
            data.insert("browser".to_string(), Value::String(b));
        }
        if let Some(d) = parsed_ua.device.clone() {
            data.insert("device".to_string(), Value::String(d));
        }
        if parsed_ua.is_bot {
            data.insert("is_bot".to_string(), Value::Bool(true));
            if let Some(name) = parsed_ua.bot_name.clone() {
                data.insert("bot_name".to_string(), Value::String(name));
            }

            // Bot routing: write to bot_events instead of events.
            let now = chrono::Utc::now().timestamp_millis();
            let extra = serde_json::Value::Object(data.clone()).to_string();
            let _ = sqlx::query(
                "INSERT INTO bot_events (property_id, event, created_at, bot_name, url, user_agent, country, extra) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(property_id.as_bytes().to_vec())
            .bind(event_name)
            .bind(now)
            .bind(parsed_ua.bot_name.as_deref())
            .bind(data.get("url").and_then(|v| v.as_str()))
            .bind(ua.as_str())
            .bind(data.get("country").and_then(|v| v.as_str()))
            .bind(extra)
            .execute(&state.pool)
            .await;
            return cors_204(&headers);
        }
    }

    // Human path: extract hot fields, leave the rest in extra.
    let now = chrono::Utc::now().timestamp_millis();
    let take_str = |key: &str, m: &mut serde_json::Map<String, Value>| -> Option<String> {
        m.remove(key)
            .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| Some(v.to_string())))
            .filter(|s| !s.is_empty())
    };
    let take_i64 = |key: &str, m: &mut serde_json::Map<String, Value>| -> Option<i64> {
        m.remove(key).and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
    };
    let take_f64 = |key: &str, m: &mut serde_json::Map<String, Value>| -> Option<f64> {
        m.remove(key).and_then(|v| v.as_f64())
    };

    let user_id = take_str("user_id", &mut data);
    let url = take_str("url", &mut data);
    let title = take_str("title", &mut data);
    let referrer = take_str("referrer", &mut data);
    let user_agent = ua_string.clone();
    data.remove("user_agent");
    let platform = take_str("platform", &mut data);
    let browser = take_str("browser", &mut data);
    let device = take_str("device", &mut data);
    let screen_width = take_i64("screen_width", &mut data);
    let screen_height = take_i64("screen_height", &mut data);
    let country = take_str("country", &mut data);
    let region = take_str("region", &mut data);
    let city = take_str("city", &mut data);
    let (lat, lon) = match data.remove("loc") {
        Some(Value::Array(arr)) if arr.len() >= 2 => (arr[0].as_f64(), arr[1].as_f64()),
        _ => (None, None),
    };
    let utm_source = take_str("utm_source", &mut data);
    let utm_medium = take_str("utm_medium", &mut data);
    let utm_campaign = take_str("utm_campaign", &mut data);
    let utm_term = take_str("utm_term", &mut data);
    let utm_content = take_str("utm_content", &mut data);
    let time_on_page_ms = take_i64("time_on_page", &mut data);
    let _ = take_f64; // suppress unused

    let extra = if data.is_empty() {
        "{}".to_string()
    } else {
        Value::Object(data).to_string()
    };

    let _ = sqlx::query(
        "INSERT INTO events (\
            property_id, event, created_at, user_id, url, title, referrer, user_agent, \
            platform, browser, device, screen_width, screen_height, country, region, city, \
            lat, lon, utm_source, utm_medium, utm_campaign, utm_term, utm_content, \
            time_on_page_ms, extra\
        ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(property_id.as_bytes().to_vec())
    .bind(event_name)
    .bind(now)
    .bind(user_id.as_deref())
    .bind(url.as_deref())
    .bind(title.as_deref())
    .bind(referrer.as_deref())
    .bind(user_agent.as_deref())
    .bind(platform.as_deref())
    .bind(browser.as_deref())
    .bind(device.as_deref())
    .bind(screen_width)
    .bind(screen_height)
    .bind(country.as_deref())
    .bind(region.as_deref())
    .bind(city.as_deref())
    .bind(lat)
    .bind(lon)
    .bind(utm_source.as_deref())
    .bind(utm_medium.as_deref())
    .bind(utm_campaign.as_deref())
    .bind(utm_term.as_deref())
    .bind(utm_content.as_deref())
    .bind(time_on_page_ms)
    .bind(extra)
    .execute(&state.pool)
    .await;

    cors_204(&headers)
}

pub async fn collector_alias(State(state): State<AppState>) -> Response {
    let dist_dir = state.config.root.join("dist");
    let manifest_path = dist_dir.join(".vite/manifest.json");
    let manifest_text = match std::fs::read_to_string(&manifest_path) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("collector manifest read: {e}");
            return (StatusCode::SERVICE_UNAVAILABLE, "collector unavailable").into_response();
        }
    };
    let manifest: serde_json::Value = match serde_json::from_str(&manifest_text) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("collector manifest parse: {e}");
            return (StatusCode::SERVICE_UNAVAILABLE, "collector unavailable").into_response();
        }
    };
    let rel = manifest
        .get("static_src/collector/index.js")
        .and_then(|c| c.get("file"))
        .and_then(|v| v.as_str());
    let Some(rel) = rel else {
        tracing::error!("collector entry missing from manifest");
        return (StatusCode::SERVICE_UNAVAILABLE, "collector unavailable").into_response();
    };
    let asset_path = dist_dir.join(rel);
    let bytes = match std::fs::read(&asset_path) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("collector read {asset_path:?}: {e}");
            return (StatusCode::SERVICE_UNAVAILABLE, "collector unavailable").into_response();
        }
    };
    let mut h = HeaderMap::new();
    h.insert(
        header::CONTENT_TYPE,
        "application/javascript; charset=utf-8".parse().unwrap(),
    );
    // 5 minutes. Short enough that an asset re-hash propagates within a deploy
    // window, long enough to absorb burst traffic from the embed snippet.
    h.insert(
        header::CACHE_CONTROL,
        "public, max-age=300, must-revalidate".parse().unwrap(),
    );
    (StatusCode::OK, h, bytes).into_response()
}

fn cors_204(req_headers: &HeaderMap) -> Response {
    let mut h = HeaderMap::new();
    let origin = req_headers
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("*")
        .to_string();
    h.insert("access-control-allow-origin", origin.parse().unwrap());
    (StatusCode::NO_CONTENT, h).into_response()
}

fn client_ip(headers: &HeaderMap) -> Option<IpAddr> {
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = xff.split(',').next() {
            if let Ok(ip) = first.trim().parse::<IpAddr>() {
                return Some(ip);
            }
        }
    }
    if let Some(real) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        if let Ok(ip) = real.parse::<IpAddr>() {
            return Some(ip);
        }
    }
    None
}
