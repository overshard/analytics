# Analytics

Self-hosted website analytics, single-operator. Single-binary axum app with sqlx + SQLite, minijinja templates, and a Vite + Bootstrap frontend.

## Quickstart

```sh
cp samplefiles/env.sample .env
# edit .env to set ANALYTICS_PASSWORD, BASE_URL, etc.
make
```

Then visit http://localhost:8000/login.

## Stack

- axum + sqlx (sqlite, WAL) on the backend, single binary
- minijinja templates, Vite + Bootstrap 5 frontend, Bun for JS deps
- maxminddb for GeoIP, ua-parser for user agents, moka for the dashboard cache
- chrome-headless-shell for PDF report export

See [CLAUDE.md](CLAUDE.md) for the full architecture rundown.
