use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/favicon.ico", get(favicon))
        .route("/robots.txt", get(robots))
        .route("/sitemap.xml", get(sitemap))
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
    (StatusCode::OK, h, body).into_response()
}
