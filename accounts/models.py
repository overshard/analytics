import uuid

from django.db import models
from django.db.models import Count, Q
from django.contrib.auth.models import AbstractUser


class User(AbstractUser):
    id = models.UUIDField(primary_key=True, default=uuid.uuid4, editable=False)

    def __str__(self):
        return self.username

    def _event_totals(self):
        # One query rolls up everything the profile page displays.
        if not hasattr(self, "_cached_event_totals"):
            self._cached_event_totals = self.properties.aggregate(
                total_properties=Count("id", distinct=True),
                total_events=Count("events"),
                total_page_views=Count("events", filter=Q(events__event="page_view")),
                total_session_starts=Count("events", filter=Q(events__event="session_start")),
            )
        return self._cached_event_totals

    @property
    def total_properties(self):
        return self._event_totals()["total_properties"]

    @property
    def total_events(self):
        return self._event_totals()["total_events"]

    @property
    def total_page_views(self):
        return self._event_totals()["total_page_views"]

    @property
    def total_session_starts(self):
        return self._event_totals()["total_session_starts"]
