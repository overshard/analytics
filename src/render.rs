use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use chrono::Datelike;

use crate::templates::{RequestCtx, UserCtx};
use crate::AppState;

/// Render a template to a String, with the standard page context injected.
///
/// `extra` is merged on top of the standard context. Templates expect
/// `user`, `request`, `now`, `base_url`, `collector_id`, `collector_server`,
/// `messages` to be present; callers supply page-specific fields like `page`
/// via `extra`.
///
/// Returns the rendered body on success, or a 500 Response with the error
/// already logged.
pub fn render_to_string(
    state: &AppState,
    template: &str,
    path: &str,
    authed: bool,
    extra: minijinja::Value,
) -> Result<String, Response> {
    let tmpl = state.env.get_template(template).map_err(|e| {
        tracing::error!("template '{}': {}", template, e);
        (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response()
    })?;
    tmpl.render(minijinja::context! {
        user => UserCtx { is_authenticated: authed },
        request => RequestCtx {
            url: String::new(),
            url_root: "/".to_string(),
            base_url: String::new(),
            path: path.to_string(),
        },
        now => minijinja::context! { year => chrono::Local::now().year() },
        base_url => &state.config.base_url,
        collector_id => state.config.proprium_id.map(|u| u.to_string()),
        collector_server => &state.config.base_url,
        messages => Vec::<()>::new(),
        ..extra
    })
    .map_err(|e| {
        tracing::error!("render '{}': {}", template, e);
        (StatusCode::INTERNAL_SERVER_ERROR, "render error").into_response()
    })
}

/// Convenience wrapper around `render_to_string` for HTML responses. Most
/// page handlers want this; `render_to_string` is for callers that need the
/// raw body (e.g. markdown downloads, PDF print templates).
pub fn render(
    state: &AppState,
    template: &str,
    path: &str,
    authed: bool,
    extra: minijinja::Value,
) -> Response {
    match render_to_string(state, template, path, authed, extra) {
        Ok(body) => Html(body).into_response(),
        Err(resp) => resp,
    }
}
