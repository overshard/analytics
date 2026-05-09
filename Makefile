CARGO ?= $(HOME)/.cargo/bin/cargo
PORT  ?= 8000

.DEFAULT_GOAL := run
.PHONY: run build start clean push pull maps seed migrate

# Dev: Vite watch + cargo run concurrently. Both die on Ctrl+C.
run: frontend/node_modules dist/.vite/manifest.json static_maps/world.json
	@trap 'kill 0' EXIT INT TERM; \
	(cd frontend && bun run dev) & \
	PORT=$(PORT) $(CARGO) run

# Production build (Vite assets + release binary)
build: frontend/node_modules static_maps/world.json
	cd frontend && bun run build
	$(CARGO) build --release

# Run the release binary (after `make build`)
start:
	PORT=$(PORT) ./target/release/analytics

# Force-rebuild the per-country topojson from natural earth.
maps: frontend/node_modules
	cd frontend && bun run build:maps

# Lazy build for `make run`: only fetches maps if world.json is missing.
# Use `make maps` to force a refresh.
static_maps/world.json: frontend/node_modules
	cd frontend && bun run build:maps

clean:
	$(CARGO) clean
	rm -rf dist frontend/node_modules data/db.sqlite3 data/db.mmdb

push:
	git remote | xargs -I R git push R master

# Seed a "Seed Test" property with realistic fake events. Re-runnable.
# Override defaults: `make seed SESSIONS=2000 DAYS=60`
seed:
	$(CARGO) run --bin seed -- $(SESSIONS) $(DAYS)

# Import an existing Django analytics SQLite into the rust hot-field schema.
# `make migrate FROM=../analytics/db.sqlite3` (add FORCE=1 to wipe first).
migrate:
	@if [ -z "$(FROM)" ]; then echo "usage: make migrate FROM=<path-to-django.sqlite3> [FORCE=1]"; exit 2; fi
	$(CARGO) run -- migrate "$(FROM)" $(if $(FORCE),--force,)

# Pull production data (db, geoip, media) for local dev
pull:
	@SERVER=$$(git config --get remote.server.url | sed 's|ssh://||' | cut -d ':' -f 1 | cut -d '/' -f 1); \
	NAME=$$(basename $$(pwd)); \
	mkdir -p data; \
	rsync -avz $$SERVER:/srv/data/$$NAME/db/db.sqlite3 data/db.sqlite3; \
	rsync -avz $$SERVER:/srv/data/$$NAME/db.mmdb data/db.mmdb

frontend/node_modules:
	cd frontend && bun install

dist/.vite/manifest.json: frontend/node_modules
	cd frontend && bun run build
