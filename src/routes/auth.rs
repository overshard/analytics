use axum::{
    extract::{Form, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde::Deserialize;
use tower_cookies::{
    cookie::{time::Duration, SameSite},
    Cookie, Cookies,
};

use crate::render::render;
use crate::AppState;

const COOKIE_NAME: &str = "session";
// 30 days. Matches the cookie max-age the browser stores.
const SESSION_TTL_SECS: i64 = 30 * 24 * 60 * 60;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", get(login_form).post(login_submit))
        .route("/logout", post(logout))
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub password: String,
    #[serde(default)]
    pub next: Option<String>,
}

/// Returns true if the request carries a valid, unexpired signed session
/// cookie. Used by every auth-gated route module.
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

fn render_login(state: &AppState, error: Option<&str>) -> Response {
    render(
        state,
        "registration/login.html",
        "/login",
        false,
        minijinja::context! {
            page => minijinja::context! {
                title => "Log in",
                description => "Log in to your dashboard.",
            },
            error => error,
            next => "/properties",
        },
    )
}

pub async fn login_form(State(state): State<AppState>, cookies: Cookies) -> Response {
    if is_authenticated(&cookies, &state) {
        return Redirect::to("/properties").into_response();
    }
    render_login(&state, None)
}

pub async fn login_submit(
    State(state): State<AppState>,
    cookies: Cookies,
    Form(form): Form<LoginForm>,
) -> Response {
    if form.password != state.config.password {
        let html = render_login(&state, Some("Invalid password."));
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
