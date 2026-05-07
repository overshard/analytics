use axum::{
    extract::{Form, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use chrono::{Datelike, Utc};
use serde::Deserialize;
use tower_cookies::{
    cookie::{time::Duration, SameSite},
    Cookie, Cookies,
};

use crate::AppState;

const COOKIE_NAME: &str = "session";
const SESSION_TTL_SECS: i64 = 30 * 24 * 60 * 60;

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub password: String,
    #[serde(default)]
    pub next: Option<String>,
}

pub fn is_authenticated(cookies: &Cookies, state: &AppState) -> bool {
    let signed = cookies.signed(&state.cookie_key);
    let Some(c) = signed.get(COOKIE_NAME) else { return false };
    let value = c.value();
    let Some((flag, exp_str)) = value.split_once(':') else { return false };
    if flag != "1" {
        return false;
    }
    let Ok(exp) = exp_str.parse::<i64>() else { return false };
    Utc::now().timestamp() < exp
}

fn render_login(state: &AppState, error: Option<&str>) -> Result<Html<String>, StatusCode> {
    let tmpl = state
        .env
        .get_template("registration/login.html")
        .map_err(|e| {
            tracing::error!("template registration/login.html: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let body = tmpl
        .render(minijinja::context! {
            user => crate::templates::UserCtx::default(),
            request => crate::templates::RequestCtx {
                url: String::new(),
                url_root: "/".to_string(),
                base_url: String::new(),
                path: "/login".to_string(),
            },
            now => minijinja::context! { year => chrono::Local::now().year() },
            base_url => &state.config.base_url,
            collector_id => state.config.proprium_id.map(|u| u.to_string()),
            collector_server => &state.config.base_url,
            messages => Vec::<()>::new(),
            page => minijinja::context! {
                title => "Log in",
                description => "Log in to your dashboard.",
            },
            error => error,
            next => "/properties",
        })
        .map_err(|e| {
            tracing::error!("render login: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Html(body))
}

pub async fn login_form(State(state): State<AppState>, cookies: Cookies) -> Response {
    if is_authenticated(&cookies, &state) {
        return Redirect::to("/properties").into_response();
    }
    match render_login(&state, None) {
        Ok(html) => html.into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn login_submit(
    State(state): State<AppState>,
    cookies: Cookies,
    Form(form): Form<LoginForm>,
) -> Response {
    if form.password != state.config.password {
        let html = match render_login(&state, Some("Invalid password.")) {
            Ok(h) => h,
            Err(e) => return e.into_response(),
        };
        return (StatusCode::UNAUTHORIZED, html).into_response();
    }
    let exp = Utc::now().timestamp() + SESSION_TTL_SECS;
    let value = format!("1:{exp}");
    let cookie = Cookie::build((COOKIE_NAME, value))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .max_age(Duration::seconds(SESSION_TTL_SECS))
        .build();
    cookies.signed(&state.cookie_key).add(cookie);
    let next = form
        .next
        .filter(|n| n.starts_with('/') && !n.starts_with("//"))
        .unwrap_or_else(|| "/properties".to_string());
    Redirect::to(&next).into_response()
}

pub async fn logout(State(state): State<AppState>, cookies: Cookies) -> Redirect {
    let signed = cookies.signed(&state.cookie_key);
    if let Some(c) = signed.get(COOKIE_NAME) {
        cookies.remove(c);
    }
    Redirect::to("/")
}
