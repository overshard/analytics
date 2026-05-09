mod app;
mod db;
mod geoip;
mod middleware;
mod migrate;
mod models;
mod pdf;
mod queries;
mod render;
mod routes;
mod templates;
mod ua;

pub use app::{AppState, Config};

use std::net::SocketAddr;
use std::path::PathBuf;

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

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8000);

    let state = AppState::from_env().await?;
    app::spawn_background_downloads(&state);
    let router = app::router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("analytics listening on http://{addr}");
    axum::serve(listener, router).await?;
    Ok(())
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
