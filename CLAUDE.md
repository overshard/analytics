# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Self-hosted website analytics, single-operator. Tracks page views, clicks, scrolls, sessions, and custom events. Properties (tracked sites) embed a collector script that POSTs JSON to `/collect`. The dashboard renders metric cards, time-series charts, world map, and PDF/markdown reports for any date range.

Single-binary axum app, no Django, no multi-user. Password from `.env`. Originally a Django service; data from that era was migrated in via `./analytics migrate <django.sqlite3>` and the Django code has been retired.

## Commands

- **Dev server:** `make run` (runs Vite watch + cargo run concurrently on port 8000)
- **Production build:** `make build` (Vite assets + topojson maps + release binary)
- **Run release binary:** `make start`
- **Pull production data:** `make pull` (rsync db + geoip from `git remote server`)
- **Rebuild map data:** `make maps` (regenerates per-country topojson)
- **Seed fake data:** `make seed` (creates/refreshes a "Seed Test" property with realistic events; override `SESSIONS=2000 DAYS=60`)
- **Import a Django DB:** `make migrate FROM=<path-to-django.sqlite3> [FORCE=1]`: one-shot import that preserves property UUIDs (so embedded snippets keep working). Same logic is exposed on the binary as `./analytics migrate <path> [--force]` so the production Docker image can run it.
- **Docker build:** `sudo docker build .`

There are no tests or linters configured.

## Architecture

**Backend:** Single-binary axum app (`src/main.rs`). Async sqlx + sqlite (WAL, `synchronous=NORMAL`, `busy_timeout=5s`, foreign keys on) for both reads and writes. Schema lives in `migrations/0001_initial.sql` and is applied automatically on boot. State is shared via `AppState` (template env, db pool, cookie key, geoip + ua parsers, dashboard cache, config).

**Auth:** Single password from `ANALYTICS_PASSWORD` in `.env`. Login form posts to `/login`; on success a signed cookie (`SameSite=Strict`, 30-day max-age) is set. The cookie payload is just `1:<exp-timestamp>`, signed with `tower-cookies`. The cookie key is `ANALYTICS_COOKIE_SECRET` if set, else derived from the password via SHA-512 (so changing the password invalidates sessions). No CSRF tokens; `SameSite=Strict` is the protection.

**Data model:** Three tables. `properties` holds tracked sites (UUID PK, name, custom_cards JSON, is_protected, is_public). `events` stores human events with hot fields extracted into typed columns (url, referrer, user_agent, country, region, city, lat/lon, screen size, platform/browser/device, utm_*, time_on_page_ms, user_id) plus an `extra` JSON blob for custom keys. `bot_events` is a separate table with the small subset of fields useful for bot reporting; bot traffic is routed there at write time so human dashboard queries never have to filter it. `meta` is a key-value table for things like the Proprium property id.

**Collector flow:** `POST /collect` (also `/collect/` for compat) accepts `{collectorId, event, data}` JSON. It looks up the property, normalizes the referrer to a bare hostname, runs GeoIP enrichment (via `maxminddb` against `data/db.mmdb`) on `session_start`, parses the user agent (via `uaparser` against `data/regexes.yaml`), and either inserts into `events` or `bot_events` based on the parsed bot flag. CORS is fully open on this endpoint. Client IP is read from `X-Forwarded-For` first, then `X-Real-IP`.

**Self-tracking (Proprium):** On first boot the binary auto-creates a "Proprium" property with `is_protected=1` and stores its UUID in the `meta` table. The base template renders a collector snippet pointing at that property when `BASE_URL` is set, so the app tracks its own usage like any other site.

**Auto-downloads:** On boot, two best-effort background tasks download `data/db.mmdb` (DB-IP City Lite, CC-BY-4.0, no signup) if missing or older than 30 days, and `data/regexes.yaml` (canonical ua-parser regexes from the `ua-parser/uap-core` repo, Apache-2.0) if missing. Failures are logged and the server still boots. UA parsing falls back to a substring heuristic and GeoIP enrichment is skipped.

**Templates:** Jinja2 templates in `templates/` rendered by minijinja with a Jinja2-faithful HTML formatter so `/` is not escaped to `&#x2f;` (matches status/blog/darkfurrow). The `vite_asset` global resolves hashed asset names by reading `dist/.vite/manifest.json`: re-read per call in debug builds (Vite watcher rebuilds show up immediately), cached at startup in release builds.

**Frontend pipeline:** Vite (run from `frontend/`) builds `frontend/static_src/` into `dist/`. Four entry points: `base` (Bootstrap 5 SCSS + monaspace font + the bootstrap JS shell), `pages` (marketing pages), `properties` (dashboard charts + map), `collector` (the public embed script). Output filenames are content-hashed and served at `/static/`.

**Map data:** `frontend/scripts/build_maps.js` (run at Docker build time via `bun run build:maps`) downloads Natural Earth admin-0 110m + admin-1 10m GeoJSON and writes per-country TopoJSON files into `static_maps/`. The Rust binary serves that directory at `/static_maps/`. The world view ships with the dashboard; per-country admin-1 is lazy-fetched on click.

**PDF generation:** `src/pdf.rs` embeds the Typst compiler (`typst` + `typst-pdf`) as a library, no chromium subprocess. `PdfRenderer::new` runs typst-kit's font searcher once at startup and shares the resulting library/book/font slots across renders. The dashboard route renders a Typst template through minijinja first (using the `typst_md` and `typst_str` filters in `templates.rs` to escape user data into Typst-safe markup), then passes the resulting source to `PdfRenderer::render` inside `tokio::task::spawn_blocking` (Typst compilation is CPU-bound and synchronous). `?report=md` returns the markdown variant directly.

**Request logging:** `src/main.rs::log_requests` middleware prints `time METHOD STATUS latency path` per request with ANSI-colored status codes (green 2xx, cyan 3xx, yellow 4xx, red 5xx). Sub-microsecond cost.

## Layout

```
analytics/
├── Cargo.toml, Cargo.lock        # rust deps
├── Makefile, README.md           # top-level
├── migrations/                   # sqlx migrations (0001_initial.sql)
├── src/                          # rust source
│   ├── main.rs        # tiny entry: env init, subcommand dispatch, server boot
│   ├── app.rs         # AppState::from_env + router() (assembles per-feature routes)
│   ├── render.rs      # render() helper (injects standard ctx)
│   ├── middleware.rs  # request log + 404 handler
│   ├── routes/        # per-feature route modules, each exposing fn router()
│   │   ├── mod.rs
│   │   ├── auth.rs        # /login, /logout, is_authenticated
│   │   ├── home.rs        # /, /changelog, /documentation
│   │   ├── seo.rs         # /favicon.ico, /robots.txt, /sitemap.xml
│   │   ├── properties.rs  # /properties (list/create/delete/custom-cards/visibility)
│   │   ├── dashboard.rs   # /<id> dashboard, ?report=pdf|md
│   │   └── collector.rs   # POST /collect with UA + GeoIP enrichment + bot routing
│   ├── db.rs          # sqlx pool init, migrate, ensure_proprium
│   ├── models.rs      # Property + PropertyRow structs
│   ├── queries.rs     # dashboard aggregations
│   ├── geoip.rs       # maxminddb wrapper + DB-IP auto-download + reload
│   ├── ua.rs          # uap-rs wrapper + regexes auto-download
│   ├── pdf.rs         # embedded Typst renderer
│   ├── migrate.rs     # `./analytics migrate <django.sqlite3>`
│   ├── bin/seed.rs    # `cargo run --bin seed` to seed a "Seed Test" property
│   └── templates.rs   # minijinja env, vite_asset, url_for, jinja2-compat formatter
├── templates/                    # minijinja-compatible jinja2
│   ├── base.html      # layout
│   ├── includes/      # collector, messages, social
│   ├── registration/  # login form
│   ├── properties/    # properties list + dashboard
│   └── pages/         # home, changelog, documentation
├── frontend/                     # JS pipeline (package.json, vite.config.js, static_src/)
│   ├── static_src/
│   │   ├── base/       # bootstrap scss + base scripts (entry: index.js)
│   │   ├── pages/      # marketing styles
│   │   ├── properties/ # dashboard chart + map JS
│   │   └── collector/  # public embed
│   └── scripts/build_maps.js   # natural earth → topojson
├── dist/                         # vite build output (gitignored, served at /static/)
├── static_maps/                  # topojson per country (gitignored, served at /static_maps/)
├── data/                         # sqlite db + geoip mmdb + regexes.yaml at runtime (gitignored)
├── target/                       # cargo build output (gitignored)
├── Dockerfile, docker-compose.yml
└── samplefiles/                  # Caddyfile.sample, env.sample, post-receive.sample
```

The binary reads `templates/`, `dist/`, `migrations/`, and `static_maps/` from cwd by default. Override the project root with `ANALYTICS_ROOT=<path>` and the data directory with `ANALYTICS_DATA_DIR=<path>` (production sets the latter to `/data`).

## Key Routes

- `/`: marketing home (redirects to `/properties` when authenticated)
- `/login`, `/logout`: single-password auth
- `/properties`: list + create + delete + custom cards + visibility toggle (auth required)
- `/<property-id>`: dashboard (auth required unless property is public). Accepts `?date_start`, `?date_end`, `?date_range`, `?filter_url`, `?report=pdf|md`.
- `/collect`, `/collect/`: public collector endpoint (CORS-open, accepts JSON POST)
- `/changelog`, `/documentation`, `/favicon.ico`, `/robots.txt`, `/sitemap.xml`: static pages
- `/static/*`: Vite assets (1y cache header)
- `/static_maps/*`: topojson per country (1y cache header)

## Tooling

- **Rust deps:** managed with `cargo` (`Cargo.toml`, `Cargo.lock`)
- **JS deps:** managed with `bun`, run from `frontend/` (`frontend/package.json`, `frontend/bun.lock`)
- **Production:** Docker (`rust:alpine` builder + `alpine:3.23` runtime, no chromium since PDF is embedded Typst). Runtime image installs `font-jetbrains-mono`, `ttf-dejavu`, `ttf-liberation`, and `fontconfig` so Typst can find a body sans, mono, and fallback fonts. Deployed via `git push server master` triggering a post-receive hook that runs `docker compose up --build --detach`. Data persisted to `/srv/data/analytics/`.

## Status of the port

The port is feature-complete against the original Django version:

- Login, properties CRUD, dashboard with all 16 metric/chart/list aggregations, custom cards, public toggle, collector with UA + GeoIP enrichment + bot routing, PDF + markdown report export, world map with admin-1 drill-down, self-tracking via Proprium, auto-download of geoip + uaparser regexes, Dockerfile + samplefiles for the standard `git push server master` deploy.
- The dashboard's frontend JS (Chart.js charts, d3-geo + topojson world map, custom-card form, public toggle, filter chips, date selector) is the original code; the server-side context shape matches what those scripts expect.

Possible future work (none of it required for parity):

- Custom-cards POST currently accepts a JSON array but the in-page form uses checkbox names. The form-handler JS posts JSON, so this works; if the JS ever stops sending JSON, the endpoint would need to accept form-encoded too.
- Auto-download of the DB-IP mmdb fails on the first day or two of every month while DB-IP rolls the new file. The code retries on next boot. For production set up `restic-status`-style monthly cron via `make pull` if you need stronger guarantees.
