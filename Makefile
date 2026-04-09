# Django + Webpack Makefile
# v. 2022.07.13


.PHONY: run runserver webpack clean push pull update
.DEFAULT: run


SERVER_URL = $(shell git config --get remote.server.url | sed 's|ssh://||' | cut -d ':' -f 1 | cut -d '/' -f 1)
PROJECT_NAME = $(shell basename $(PWD))


run: install
	@echo "run ----------------------------------------------------------------"
	${MAKE} -j2 runserver webpack

runserver:
	uv run python manage.py runserver 0.0.0.0:8000

webpack:
	npx webpack --config webpack.config.js --mode development --watch --devtool source-map


install: node_modules/touchfile .venv/touchfile db.sqlite3

node_modules/touchfile: package.json
	@echo "install node deps --------------------------------------------------"
	yarn install
	touch $@
	@echo "> all node deps installed"

.venv/touchfile: pyproject.toml
	@echo "install python deps ------------------------------------------------"
	uv sync
	touch $@
	@echo "> all python deps installed"

db.sqlite3:
	@echo "create database ----------------------------------------------------"
	uv run python manage.py migrate
	@echo "> database created"


push:
	@echo "push ---------------------------------------------------------------"
	git remote | xargs -I R git push R master

pull:
	@echo "pull ---------------------------------------------------------------"
	rsync -avz $(SERVER_URL):/srv/data/$(PROJECT_NAME)/db/db.sqlite3 db.sqlite3
	rsync -avz $(SERVER_URL):/srv/data/$(PROJECT_NAME)/db.mmdb db.mmdb
	rsync -avz $(SERVER_URL):/srv/data/$(PROJECT_NAME)/media/ media
	@echo "> all files copied"


update: install
	@echo "update -------------------------------------------------------------"
	uv lock --upgrade
	yarn upgrade --latest
	@echo "> all deps updated"


clean:
	@echo "clean --------------------------------------------------------------"
	rm -rf node_modules
	rm -rf .venv
	rm -rf db.sqlite3
	rm -rf db.mmdb
	rm -rf media
	@echo "> all files removed"
