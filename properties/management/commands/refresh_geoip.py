"""
Download or refresh the DB-IP City Lite GeoIP database.

DB-IP publishes a fresh build on the 1st of each month at:

    https://download.db-ip.com/free/dbip-city-lite-YYYY-MM.mmdb.gz

License is CC-BY-4.0 — attribution lives in the dashboard footer. No account,
no API key. The mmdb format is MaxMind-compatible so the existing geoip2
reader picks it up unchanged.

Run on container start (idempotent, skips if the on-disk file is fresh) and
again monthly via host cron.
"""

import gzip
import os
import shutil
import sys
import tempfile
import urllib.error
import urllib.request
from datetime import date, timedelta
from pathlib import Path

from django.conf import settings
from django.core.management.base import BaseCommand


URL_TEMPLATE = "https://download.db-ip.com/free/dbip-city-lite-{year}-{month:02d}.mmdb.gz"
USER_AGENT = "analytics-refresh-geoip/1.0 (+https://github.com/overshard/analytics)"
MAX_AGE_DAYS = 30


def _candidate_months(today=None):
    """
    Yield (year, month) tuples to try, newest first.

    DB-IP publishes on the 1st but may lag a few hours; if the current month
    isn't up yet, fall back to the previous month, and one more before that
    in case we're catching up after a long outage.
    """
    today = today or date.today()
    for offset in (0, 1, 2):
        d = (today.replace(day=1) - timedelta(days=offset * 28)).replace(day=1)
        yield d.year, d.month


class Command(BaseCommand):
    help = "Download (or refresh) the DB-IP City Lite GeoIP database to GEOIP_PATH."

    def add_arguments(self, parser):
        parser.add_argument(
            "--force",
            action="store_true",
            help="Re-download even if the existing file is younger than 30 days.",
        )

    def handle(self, *args, **options):
        target = Path(getattr(settings, "GEOIP_PATH", "")).resolve()
        if not target.parent.exists():
            self.stderr.write(f"GEOIP_PATH parent dir does not exist: {target.parent}")
            sys.exit(0)

        if not options["force"] and target.exists():
            age_days = (date.today() - date.fromtimestamp(target.stat().st_mtime)).days
            if age_days < MAX_AGE_DAYS:
                self.stdout.write(f"GeoIP database is {age_days}d old; skipping refresh.")
                return

        last_error = None
        for year, month in _candidate_months():
            url = URL_TEMPLATE.format(year=year, month=month)
            try:
                self.stdout.write(f"Fetching {url}")
                self._download(url, target)
                self.stdout.write(self.style.SUCCESS(f"GeoIP database updated at {target}"))
                return
            except urllib.error.HTTPError as e:
                if e.code == 404:
                    self.stdout.write(f"  not yet published ({year}-{month:02d})")
                    last_error = e
                    continue
                last_error = e
                break
            except (urllib.error.URLError, OSError) as e:
                last_error = e
                break

        # Non-fatal: dashboard works without GeoIP, collector silently skips
        # enrichment when the file is missing or stale.
        self.stderr.write(f"GeoIP refresh failed: {last_error}")
        sys.exit(0)

    def _download(self, url, target):
        request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
        with urllib.request.urlopen(request, timeout=120) as response:
            tmp_dir = target.parent
            with tempfile.NamedTemporaryFile(
                dir=tmp_dir, prefix=".geoip-", suffix=".mmdb", delete=False
            ) as tmp:
                tmp_path = Path(tmp.name)
                try:
                    with gzip.GzipFile(fileobj=response) as gz:
                        shutil.copyfileobj(gz, tmp)
                    tmp.flush()
                    os.fsync(tmp.fileno())
                except Exception:
                    tmp_path.unlink(missing_ok=True)
                    raise
        os.replace(tmp_path, target)
