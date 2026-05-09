use axum::{
    extract::{Form, Path as AxumPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Redirect, Response},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde_json::json;
use tower_cookies::Cookies;
use uuid::Uuid;

use crate::render::render;
use crate::routes::auth::is_authenticated;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/properties", get(properties).post(properties_create))
        .route("/properties/{id}/delete", post(property_delete))
        .route("/properties/{id}/cards", post(property_cards))
        .route("/properties/{id}/public", post(property_public_toggle))
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

    // 7 days in ms. Used as the "active in the last week" threshold for
    // marking a property as live in the list view.
    const ACTIVE_WINDOW_MS: i64 = 7 * 24 * 60 * 60 * 1000;

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
        .bind(chrono::Utc::now().timestamp_millis() - ACTIVE_WINDOW_MS)
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

    render(&state, "properties/properties.html", "/properties", true, extra)
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
