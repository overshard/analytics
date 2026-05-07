mod auth;
mod cache;
mod collector;
mod db;
mod geoip;
mod migrate;
mod models;
mod pages;
mod pdf;
mod queries;
mod templates;
mod ua;
mod views;

use axum::{
    extract::Request,
    http::{header, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use chrono::Local;
use minijinja::Environment;
use sqlx::SqlitePool;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tower_cookies::{CookieManagerLayer, Key};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

use crate::cache::DashboardCache;
use crate::geoip::GeoIp;
use crate::ua::UaParser;

#[derive(Clone)]
pub struct AppState {
    pub env: Arc<Environment<'static>>,
    pub pool: SqlitePool,
    pub cookie_key: Key,
    pub geoip: Arc<GeoIp>,
    pub ua: Arc<UaParser>,
    pub cache: DashboardCache,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,sqlx=warn")),
        )
        .init();

    // Subcommand dispatch. Anything besides `migrate` falls through to the server.
    let mut argv = std::env::args().skip(1);
    if let Some(first) = argv.next() {
        match first.as_str() {
            "migrate" => return run_migrate(argv.collect()).await,
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other if !other.is_empty() => {
                eprintln!("unknown subcommand: {other}");
                print_usage();
                std::process::exit(2);
            }
            _ => {}
        }
    }

    let root: PathBuf = std::env::var("ANALYTICS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let data_dir = std::env::var("ANALYTICS_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("data"));
    std::fs::create_dir_all(&data_dir)?;

    let templates_dir = root.join("templates");
    let dist_dir = root.join("dist");
    let static_maps_dir = root.join("static_maps");
    let manifest_path = dist_dir.join(".vite/manifest.json");

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8000);

    let password = std::env::var("ANALYTICS_PASSWORD").unwrap_or_else(|_| "admin".to_string());
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

    let geoip_path = data_dir.join("db.mmdb");
    let regexes_path = data_dir.join("regexes.yaml");
    let geoip = Arc::new(GeoIp::load(&geoip_path));
    let ua = Arc::new(UaParser::load(&regexes_path));

    // Best-effort background downloads. Server boots immediately; once these
    // finish the next collector hit picks up the loaded data.
    {
        let geoip = geoip.clone();
        let geoip_path = geoip_path.clone();
        tokio::spawn(async move {
            match geoip::ensure_db(&geoip_path).await {
                Ok(true) => {
                    geoip.reload();
                }
                Ok(false) => {}
                Err(e) => tracing::warn!("geoip download skipped: {e}"),
            }
        });
    }
    {
        let regexes_path = regexes_path.clone();
        tokio::spawn(async move {
            if let Err(e) = ua::ensure_regexes(&regexes_path).await {
                tracing::warn!("uaparser regexes download skipped: {e}");
            }
            // Note: hot-reload of UA parser would need RwLock too. For now
            // the download primes the file for the next process restart.
        });
    }

    let env = templates::build_env(&templates_dir, &manifest_path);

    let config = Arc::new(Config {
        root: root.clone(),
        data_dir: data_dir.clone(),
        password,
        base_url,
        proprium_id: Some(proprium_id),
    });

    let cache = DashboardCache::new();

    let state = AppState {
        env: Arc::new(env),
        pool,
        cookie_key,
        geoip,
        ua,
        cache,
        config: config.clone(),
    };

    let collector_cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let static_cache = SetResponseHeaderLayer::if_not_present(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000"),
    );

    let app = Router::new()
        .route("/", get(pages::home))
        .route("/login", get(auth::login_form).post(auth::login_submit))
        .route("/logout", post(auth::logout))
        .route("/properties", get(views::properties).post(views::properties_create))
        .route("/properties/{id}/delete", post(views::property_delete))
        .route("/properties/{id}/cards", post(views::property_cards))
        .route("/properties/{id}/public", post(views::property_public_toggle))
        .route("/changelog", get(pages::changelog))
        .route("/documentation", get(pages::documentation))
        .route("/favicon.ico", get(pages::favicon))
        .route("/robots.txt", get(pages::robots))
        .route("/sitemap.xml", get(pages::sitemap))
        .route("/collect", post(collector::collect).options(collector::options))
        .route("/collect/", post(collector::collect).options(collector::options))
        .layer(collector_cors)
        // Stable URL for the embed snippet. Resolves the hashed Vite output
        // at request time so consumers can hardcode this path forever.
        .route("/static/collector.js", get(pages::collector_alias))
        // Property dashboard uses a UUID path segment. Keep it last so the named
        // routes above take precedence.
        .route("/{property_id}", get(views::property))
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
        .fallback(not_found)
        .layer(CookieManagerLayer::new())
        .layer(middleware::from_fn(log_requests))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("analytics listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn log_requests(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());
    let start = Instant::now();
    let response = next.run(req).await;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let status = response.status().as_u16();
    let now = Local::now().format("%H:%M:%S");
    let color = match status {
        200..=299 => "\x1b[32m",
        300..=399 => "\x1b[36m",
        400..=499 => "\x1b[33m",
        _ => "\x1b[31m",
    };
    eprintln!("{now} {method:<5} {color}{status}\x1b[0m {elapsed_ms:>7.2}ms  {path}");
    response
}

async fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "404 Not Found").into_response()
}

fn print_usage() {
    eprintln!(
        "analytics — single-binary axum analytics server\n\
         \n\
         Usage:\n  \
           analytics                              run the HTTP server\n  \
           analytics migrate <path> [--force]     import a Django analytics SQLite\n"
    );
}

async fn run_migrate(args: Vec<String>) -> anyhow::Result<()> {
    let mut source: Option<PathBuf> = None;
    let mut force = false;
    for arg in args {
        match arg.as_str() {
            "--force" | "-f" => force = true,
            "--help" | "-h" => {
                eprintln!("Usage: analytics migrate <path-to-django-sqlite3> [--force]");
                return Ok(());
            }
            v if v.starts_with("--") => anyhow::bail!("unknown migrate flag: {v}"),
            v => {
                if source.is_some() {
                    anyhow::bail!("migrate takes a single source path");
                }
                source = Some(PathBuf::from(v));
            }
        }
    }
    let source = source.ok_or_else(|| {
        anyhow::anyhow!("usage: analytics migrate <path-to-django-sqlite3> [--force]")
    })?;
    migrate::run(source, force).await
}
