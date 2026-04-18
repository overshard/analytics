# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Self-hosted website analytics service. Tracks page views, clicks, scrolls, sessions, and custom events. Users create "Properties" (tracked sites), embed a collector script, and view dashboards with date range filtering, comparisons, charts, maps, and PDF report export.

## Development Commands

- **`make`** — installs deps (uv + bun) if needed, creates DB, then runs Django dev server and Vite watch concurrently
- **`uv run python manage.py runserver`** — Django only
- **`bun run dev`** — Vite watch only
- **`uv run python manage.py migrate`** — apply migrations
- **`uv run python manage.py makemigrations`** — generate migrations
- **`make pull`** — rsync production DB, GeoIP database, and media from server
- **`make push`** — push to all git remotes
- **Deps:** Python via `uv`, JS via `bun`. No tests, no linters.
- **Default login:** admin / admin

## Django Apps

| App | Purpose |
|---|---|
| `analytics` | Project config (settings, URLs, templates, ASGI/WSGI, context processors, headless chromium PDF utils via `analytics/chromium.py`). Vite outputs land in `analytics/static/`. |
| `accounts` | Custom `User` model (UUID PK, extends `AbstractUser`). Login/logout/signup views. |
| `properties` | Core domain. `Property` model (a tracked site, UUID PK, belongs to User) and `Event` model (stores all analytics events as JSON in `data` field). Dashboard views, query helpers (`queries.py`), and custom card management. |
| `collector` | Single `POST /collect/` endpoint (CSRF-exempt, CORS-open). Receives events from client JS, enriches with GeoIP + user-agent parsing, filters bots, saves to DB. |
| `pages` | Static pages: home, changelog, documentation, favicon, robots, sitemap. |

## Architecture

**Data model:** Everything centers on `Property` → `Event`. Events have a `event` type string (session_start, page_view, click, scroll, page_leave, or custom) and a `data` JSONField holding all event-specific key-value pairs. There is no separate table per event type — all querying is done via Django's JSON field lookups (`data__url`, `data__referrer`, `data__utm_source`, etc.).

**Collection flow:** Client sites include `collector.js` (bundled via Vite's `collector` entry point). The script sets a `collectoruserid` cookie, fires session_start (on first visit), page_view, click, scroll, and page_leave events to `POST /collect/`. The server-side view enriches session_start events with GeoIP data (optional, requires `db.mmdb` MaxMind database) and parses user-agent strings into platform/browser/device fields. Bot traffic is silently dropped.

**Dashboard:** `properties/views.py:property()` is the main dashboard view. It filters events by date range, computes current vs. previous period comparisons, and builds all chart/list data server-side. Standard metric cards are computed in `properties/queries.py`. Properties can have custom event cards (stored as JSON on the Property model). The dashboard supports a `?report` query param to generate PDF reports via a headless Chromium subprocess (`analytics/chromium.py`).

**Frontend:** Vite bundles 4 entry points (`base`, `pages`, `properties`, `collector`) from each app's `static_src/` directory. Uses Bootstrap 5 (SCSS), Chart.js for graphs, D3 + datamaps for the US state map. Output goes to `analytics/static/`. WhiteNoise serves static files.

**Settings:** Split into `analytics/settings/__init__.py` (shared), `development.py`, and `production.py`. Dev uses SQLite at project root; production uses SQLite at `/data/db/db.sqlite3`. `DJANGO_SETTINGS_MODULE` defaults to development; production sets it via `.env`.

**Production:** Single Docker container (Alpine 3.21 base) running Gunicorn with Uvicorn workers (ASGI). Caddy as reverse proxy. Data persisted to `/srv/data/analytics/`. Deployed via `git push server master` triggering a post-receive hook.

## Key Conventions

- All model PKs are UUIDs.
- Event data is schemaless — the `data` JSONField is the extensibility point. New event attributes are added by sending them from the client; no migration needed.
- The `collector` context processor injects `collector_server` and `collector_id` into all templates so the app can track its own usage (property named "Proprium").
- GeoIP is optional — if `db.mmdb` is missing, location enrichment is silently skipped.
- Chromium is bundled in the Docker image (Alpine `chromium` package) for server-side PDF generation. `analytics/chromium.py` wraps a headless Chromium subprocess — no Playwright dependency.
