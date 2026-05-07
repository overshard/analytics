use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
};
use chrono::Datelike;
use tower_cookies::Cookies;

use crate::auth::is_authenticated;
use crate::AppState;

fn render_page(
    state: &AppState,
    template: &str,
    title: &str,
    description: &str,
    authed: bool,
    path: &str,
    extra: minijinja::Value,
) -> Response {
    let tmpl = match state.env.get_template(template) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("template '{}': {}", template, e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response();
        }
    };
    let request = crate::templates::RequestCtx {
        url: String::new(),
        url_root: "/".to_string(),
        base_url: String::new(),
        path: path.to_string(),
    };
    let body = tmpl.render(minijinja::context! {
        page => minijinja::context! { title => title, description => description },
        user => crate::templates::UserCtx { is_authenticated: authed },
        request => &request,
        now => minijinja::context! { year => chrono::Local::now().year() },
        base_url => &state.config.base_url,
        collector_id => state.config.proprium_id.map(|u| u.to_string()),
        collector_server => &state.config.base_url,
        messages => Vec::<()>::new(),
        ..extra
    });
    match body {
        Ok(b) => Html(b).into_response(),
        Err(e) => {
            tracing::error!("render '{}': {}", template, e);
            (StatusCode::INTERNAL_SERVER_ERROR, "render error").into_response()
        }
    }
}

pub async fn home(State(state): State<AppState>, cookies: Cookies) -> Response {
    let authed = is_authenticated(&cookies, &state);
    if authed {
        return axum::response::Redirect::to("/properties").into_response();
    }
    let totals: (i64, i64, Option<i64>) = sqlx::query_as(
        "SELECT \
           (SELECT COUNT(*) FROM properties), \
           (SELECT COUNT(*) FROM events), \
           (SELECT MIN(created_at) FROM events)",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or((0, 0, None));

    let first = totals.2.and_then(|ms| {
        chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)
            .map(|d| d.format("%b %-d, %Y").to_string())
    });

    let extra = minijinja::context! {
        total_properties => totals.0,
        total_events => totals.1,
        total_users => 1,
        first_event_created_at => first,
    };
    render_page(
        &state,
        "pages/home.html",
        "Self-hosted analytics",
        "Self-hosted website analytics. Page views, clicks, scrolls, sessions, and custom events.",
        authed,
        "/",
        extra,
    )
}

pub async fn changelog(State(state): State<AppState>, cookies: Cookies) -> Response {
    let authed = is_authenticated(&cookies, &state);
    render_page(
        &state,
        "pages/changelog.html",
        "Changelog",
        "What's new in Analytics.",
        authed,
        "/changelog",
        minijinja::context! {},
    )
}

pub async fn documentation(State(state): State<AppState>, cookies: Cookies) -> Response {
    let authed = is_authenticated(&cookies, &state);
    render_page(
        &state,
        "pages/documentation.html",
        "Documentation",
        "How to embed and operate Analytics.",
        authed,
        "/documentation",
        minijinja::context! {},
    )
}

// Stable URL for the collector embed script. Vite content-hashes the
// `collector` entry, but every embed snippet in the wild (and the one shown
// to users in the dashboard) hardcodes `/static/collector.js`. This handler
// reads the Vite manifest, resolves the hashed asset, and serves it with a
// short cache TTL so updates propagate without forcing consumers to re-embed.
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
    h.insert(
        header::CACHE_CONTROL,
        "public, max-age=300, must-revalidate".parse().unwrap(),
    );
    (StatusCode::OK, h, bytes).into_response()
}

pub async fn favicon() -> Response {
    let mut h = HeaderMap::new();
    h.insert(header::CONTENT_TYPE, "image/svg+xml".parse().unwrap());
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <rect x="6"  y="38" width="10" height="22" rx="1.5" fill="#6b9e78"/>
  <rect x="20" y="28" width="10" height="32" rx="1.5" fill="#6b9e78"/>
  <rect x="34" y="18" width="10" height="42" rx="1.5" fill="#6b9e78"/>
  <rect x="48" y="8"  width="10" height="52" rx="1.5" fill="#6b9e78"/>
  <rect x="48" y="8"  width="10" height="6"  rx="1.5" fill="#c9a84c"/>
</svg>"##;
    (StatusCode::OK, h, svg).into_response()
}

pub async fn robots() -> Response {
    let mut h = HeaderMap::new();
    h.insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());
    (StatusCode::OK, h, "User-agent: *\nAllow: /\n").into_response()
}

pub async fn sitemap(State(state): State<AppState>) -> Response {
    let mut h = HeaderMap::new();
    h.insert(header::CONTENT_TYPE, "application/xml".parse().unwrap());
    let base = if state.config.base_url.is_empty() {
        "/".to_string()
    } else {
        format!("{}/", state.config.base_url.trim_end_matches('/'))
    };
    let now = chrono::Utc::now().format("%Y-%m-%d");
    let body = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>{base}</loc><lastmod>{now}</lastmod></url>
  <url><loc>{base}documentation</loc><lastmod>{now}</lastmod></url>
  <url><loc>{base}changelog</loc><lastmod>{now}</lastmod></url>
</urlset>
"##
    );
    let _ = chrono::Local::now().year(); // keep chrono::Datelike used
    (StatusCode::OK, h, body).into_response()
}
