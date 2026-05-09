use axum::{http::HeaderValue, middleware as axum_middleware, Router};
use minijinja::Environment;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use tower_cookies::{CookieManagerLayer, Key};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

use crate::geoip::{self, GeoIp};
use crate::routes;
use crate::ua::{self, UaParser};
use crate::{db, middleware, templates};

#[derive(Clone)]
pub struct AppState {
    pub env: Arc<Environment<'static>>,
    pub pool: SqlitePool,
    pub cookie_key: Key,
    pub geoip: Arc<GeoIp>,
    pub ua: Arc<UaParser>,
    pub config: Arc<Config>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub root: PathBuf,
    pub data_dir: PathBuf,
    pub password: String,
    pub base_url: String,
    pub proprium_id: Option<uuid::Uuid>,
}

impl AppState {
    pub async fn from_env() -> anyhow::Result<Self> {
        let root: PathBuf = std::env::var("ANALYTICS_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let data_dir = std::env::var("ANALYTICS_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| root.join("data"));
        std::fs::create_dir_all(&data_dir)?;

        let password =
            std::env::var("ANALYTICS_PASSWORD").unwrap_or_else(|_| "admin".to_string());
        let base_url = std::env::var("BASE_URL").unwrap_or_default();

        let cookie_secret = std::env::var("ANALYTICS_COOKIE_SECRET").unwrap_or_else(|_| {
            // 64+ bytes derived from password if no secret provided. For a single-user
            // self-hosted app this is fine; setting ANALYTICS_COOKIE_SECRET is preferred.
            use sha2::{Digest, Sha512};
            let mut h = Sha512::new();
            h.update(b"analytics-cookie:");
            h.update(password.as_bytes());
            let digest = h.finalize();
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, digest)
        });
        let cookie_key = Key::from(cookie_secret.as_bytes());

        let pool = db::init(&data_dir).await?;
        let proprium_id = db::ensure_proprium(&pool).await?;
        tracing::info!("Proprium property: {}", proprium_id);

        let geoip = Arc::new(GeoIp::load(&data_dir.join("db.mmdb")));
        let ua = Arc::new(UaParser::load(&data_dir.join("regexes.yaml")));

        let templates_dir = root.join("templates");
        let manifest_path = root.join("dist/.vite/manifest.json");
        let env = Arc::new(templates::build_env(&templates_dir, &manifest_path));

        let config = Arc::new(Config {
            root,
            data_dir,
            password,
            base_url,
            proprium_id: Some(proprium_id),
        });

        Ok(Self {
            env,
            pool,
            cookie_key,
            geoip,
            ua,
            config,
        })
    }
}

/// Best-effort background downloads. The server boots immediately; once these
/// finish the next collector hit picks up the loaded data. Failures are logged
/// and ignored so a flaky network doesn't block the server from running.
pub fn spawn_background_downloads(state: &AppState) {
    let geoip = state.geoip.clone();
    let geoip_path = state.config.data_dir.join("db.mmdb");
    tokio::spawn(async move {
        match geoip::ensure_db(&geoip_path).await {
            Ok(true) => {
                geoip.reload();
            }
            Ok(false) => {}
            Err(e) => tracing::warn!("geoip download skipped: {e}"),
        }
    });

    let regexes_path = state.config.data_dir.join("regexes.yaml");
    tokio::spawn(async move {
        if let Err(e) = ua::ensure_regexes(&regexes_path).await {
            tracing::warn!("uaparser regexes download skipped: {e}");
        }
        // Note: hot-reload of UA parser would need RwLock too. For now
        // the download primes the file for the next process restart.
    });
}

pub fn router(state: AppState) -> Router {
    let dist_dir = state.config.root.join("dist");
    let static_maps_dir = state.config.root.join("static_maps");

    let static_cache = SetResponseHeaderLayer::if_not_present(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000"),
    );

    Router::new()
        // Per-feature routers. CORS lives inside routes::collector so it's
        // scoped to /collect (the only endpoint that is cross-origin by
        // design). Same-origin routes don't need it.
        .merge(routes::home::router())
        .merge(routes::auth::router())
        .merge(routes::seo::router())
        .merge(routes::collector::router())
        .merge(routes::properties::router())
        // routes::dashboard holds the UUID `/{property_id}` catch-all; merge
        // last so named routes win the match.
        .merge(routes::dashboard::router())
        .nest_service(
            "/static",
            tower::ServiceBuilder::new()
                .layer(static_cache.clone())
                .service(ServeDir::new(&dist_dir)),
        )
        .nest_service(
            "/static_maps",
            tower::ServiceBuilder::new()
                .layer(static_cache)
                .service(ServeDir::new(&static_maps_dir)),
        )
        .fallback(middleware::not_found)
        .layer(CookieManagerLayer::new())
        .layer(axum_middleware::from_fn(middleware::log_requests))
        .with_state(state)
}
