use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use tower_cookies::Cookies;

use crate::render::render;
use crate::routes::auth::is_authenticated;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(home))
        .route("/changelog", get(changelog))
        .route("/documentation", get(documentation))
}

fn render_page(
    state: &AppState,
    template: &str,
    title: &str,
    description: &str,
    authed: bool,
    path: &str,
    extra: minijinja::Value,
) -> Response {
    render(
        state,
        template,
        path,
        authed,
        minijinja::context! {
            page => minijinja::context! { title => title, description => description },
            ..extra
        },
    )
}

pub async fn home(State(state): State<AppState>, cookies: Cookies) -> Response {
    let authed = is_authenticated(&cookies, &state);
    if authed {
        return Redirect::to("/properties").into_response();
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

    render_page(
        &state,
        "pages/home.html",
        "Self-hosted analytics",
        "Self-hosted website analytics. Page views, clicks, scrolls, sessions, and custom events.",
        authed,
        "/",
        minijinja::context! {
            total_properties => totals.0,
            total_events => totals.1,
            total_users => 1,
            first_event_created_at => first,
        },
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
