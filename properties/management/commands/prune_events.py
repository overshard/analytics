from django.core.management.base import BaseCommand
from django.utils import timezone

from properties.models import Event


class Command(BaseCommand):
    help = "Delete Event rows older than --days (default 730)."

    def add_arguments(self, parser):
        parser.add_argument("--days", type=int, default=730)
        parser.add_argument("--dry-run", action="store_true")

    def handle(self, *args, **options):
        cutoff = timezone.now() - timezone.timedelta(days=options["days"])
        qs = Event.objects.filter(created_at__lt=cutoff)
        count = qs.count()

        if options["dry_run"]:
            self.stdout.write(f"Would delete {count} events older than {cutoff.isoformat()}")
            return

        deleted, _ = qs.delete()
        self.stdout.write(f"Deleted {deleted} events older than {cutoff.isoformat()}")
