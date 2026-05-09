# Analytics

Self-hosted website analytics, single-operator. One axum binary with sqlx + SQLite, minijinja templates, and a Vite + Bootstrap frontend. Tracks page views, clicks, scrolls, sessions, and custom events; renders metric cards, time-series charts, a world map with admin-1 drill-down, and PDF/markdown reports for any date range.

## Features

- Embed a small collector script on any number of properties (tracked sites)
- Dashboard with 16 metric/chart/list aggregations and user-defined custom cards
- GeoIP enrichment (auto-downloads DB-IP City Lite) and UA parsing (auto-downloads ua-parser regexes)
- Bot traffic routed to a separate table so human dashboards never have to filter it
- Public/private toggle per property; signed-cookie auth for the operator
- PDF and markdown export of any dashboard view (PDF rendered in-process via embedded Typst, no chromium)
- Single-binary deploy via `git push server master`

## System dependencies

Local dev needs all of these on your `PATH`:

| Tool | Why | Version |
|---|---|---|
| `rustc` / `cargo` | Build the axum binary | 2021 edition, current stable is fine (1.70+) |
| `bun` | Frontend deps + Vite + map builder | 1.x |
| `make` | Run the dev/build targets | any |
| `pkg-config` + OpenSSL headers | Linked at build time on Linux | distro packages: `pkg-config`, `libssl-dev` (Debian/Ubuntu), `openssl-dev` (Alpine) |

Install hints:

```sh
# Rust toolchain (recommended via rustup)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Bun
curl -fsSL https://bun.sh/install | bash

# System libs (Debian/Ubuntu)
sudo apt install -y build-essential pkg-config libssl-dev

# System libs (Alpine)
sudo apk add musl-dev pkgconfig openssl-dev
```

The Docker build (see `Dockerfile`) reproduces this on `rust:alpine` + `alpine:3.23`. If you only care about Docker, you do not need any of the above on the host.

Two more files are downloaded into `data/` at runtime, no setup required:

- `data/db.mmdb`: DB-IP City Lite (CC-BY-4.0). Refreshed if older than 30 days.
- `data/regexes.yaml`: canonical ua-parser regexes (Apache-2.0).

If either download fails the server still boots: UA parsing falls back to a substring heuristic and GeoIP enrichment is skipped.

## Quickstart

```sh
cp samplefiles/env.sample .env
# edit .env to set ANALYTICS_PASSWORD and BASE_URL
make
```

`make` (alias `make run`) installs frontend deps if needed, then runs Vite watch and `cargo run` concurrently on port 8000. Visit http://localhost:8000/login.

First boot creates a "Proprium" property and starts tracking the dashboard's own usage.

## Configuration

All config comes from `.env` (loaded via `dotenvy`). The full set:

| Variable | Required | Purpose |
|---|---|---|
| `ANALYTICS_PASSWORD` | yes | Single operator password |
| `BASE_URL` | yes for prod | Used in absolute URLs (sitemap, og tags, embed snippet) |
| `PORT` | no (default `8000`) | HTTP listen port |
| `ANALYTICS_COOKIE_SECRET` | no | 32+ bytes for signing the session cookie. Falls back to a SHA-512 of the password, so rotating the password invalidates sessions |
| `ANALYTICS_DATA_DIR` | no (default `./data`) | Where the SQLite db, mmdb, and regexes live. Production sets this to `/data` |
| `ANALYTICS_ROOT` | no | Override the project root (where `templates/`, `dist/`, `migrations/`, `static_maps/` are read from) |

## Make targets

| Target | What it does |
|---|---|
| `make run` (default) | Vite watch + `cargo run` on port 8000 |
| `make build` | Vite assets + topojson maps + release binary (`target/release/analytics`) |
| `make start` | Run the release binary (after `make build`) |
| `make maps` | Rebuild per-country topojson under `static_maps/` from Natural Earth |
| `make seed` | Create or refresh a "Seed Test" property with realistic fake events. Override with `SESSIONS=2000 DAYS=60` |
| `make migrate FROM=<path-to-django.sqlite3>` | One-shot import of an existing Django analytics database, preserving property UUIDs so embedded snippets keep working. Add `FORCE=1` to wipe first |
| `make pull` | rsync the production db + geoip from `git remote server` into `data/` |
| `make push` | `git push` to every configured remote |
| `make clean` | `cargo clean` plus removing `dist/`, `node_modules/`, the SQLite db, and the mmdb |

There are no tests or linters configured.

## Deploy

Production runs on Docker. The standard flow is `git push server master` to a remote whose post-receive hook runs `docker compose up --build --detach`. Sample files in `samplefiles/`:

- `Caddyfile.sample`: reverse proxy with TLS
- `env.sample`: the same `.env` shown above
- `post-receive.sample`: the git hook

Data persists to `/srv/data/analytics/` on the host (mounted into the container at `/data`).

## Stack

- **Backend:** axum 0.8, sqlx 0.8 against SQLite (WAL, `synchronous=NORMAL`, `busy_timeout=5s`), tower-cookies for signed sessions
- **Templates:** minijinja 2 with a Jinja2-faithful HTML formatter
- **Frontend:** Vite 6, Bootstrap 5 SCSS, Chart.js, d3-geo + topojson, monaspace argon font (self-hosted via `@fontsource`)
- **Enrichment:** maxminddb (GeoIP), uaparser (UA)
- **PDF:** embedded Typst (`typst` + `typst-pdf` + `typst-kit`), no chromium subprocess

See [CLAUDE.md](CLAUDE.md) for the full architecture rundown, route table, and data model.
