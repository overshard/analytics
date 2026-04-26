# TODO

## Upgrade SQLite to ≥3.51.3 to close the WAL-reset corruption bug

The `status` repo hit a `database disk image is malformed` corruption on
2026-04-19. Root cause was a two-part interaction:

1. **SQLite WAL-reset bug** (introduced 3.7.0, fixed in **3.51.3** released
   2026-03-13). Triggered when two or more connections on the same file
   write/checkpoint simultaneously.
2. **`PRAGMA mmap_size=128MB`** amplified a transient WAL inconsistency into
   structural corruption. SQLite docs explicitly warn against mmap with
   multi-process writers.

This repo had the same `mmap_size` PRAGMA and the same Alpine 3.21 base, so
mmap has been removed defensively (commit `9cca9dc`). Risk is lower than
status because there's no always-on thread-pool scheduler — just gunicorn's
2 workers — but the latent bug still exists because we're stuck on SQLite
3.48.0 (Alpine 3.21).

### Path forward, by preference

- **Wait for Alpine 3.23** to ship SQLite ≥3.51.3 (likely May–June 2026), then
  bump `FROM alpine:3.21` in the Dockerfile. Zero code change.
- If it recurs before Alpine 3.23 lands: build SQLite from source in the
  Dockerfile and `LD_PRELOAD` it, or switch the DB to Postgres.

### Versions checked 2026-04-26

- Alpine 3.21: sqlite-libs 3.48.0 (vulnerable, currently in use)
- Alpine 3.22: sqlite-libs 3.49.2 (vulnerable)
- Alpine edge: sqlite-libs 3.53.0 (fixed, but edge isn't appropriate for prod)
- `pysqlite3-binary` 0.5.4.post2: bundles 3.51.1 (vulnerable; package's last
  release predates the SQLite fix)
