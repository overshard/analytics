import uuid

from django.db import models
from django.contrib.auth.models import AbstractUser


class User(AbstractUser):
    id = models.UUIDField(primary_key=True, default=uuid.uuid4, editable=False)

    def __str__(self):
        return self.username

    @property
    def total_properties(self):
        return self.properties.count()

    @property
    def total_events(self):
        total_events = 0
        for property in self.properties.all():
            total_events += property.total_events
        return total_events

    @property
    def total_page_views(self):
        total_page_views = 0
        for property in self.properties.all():
            total_page_views += property.total_page_views
        return total_page_views

    @property
    def total_session_starts(self):
        total_session_starts = 0
        for property in self.properties.all():
            total_session_starts += property.total_session_starts
        return total_session_starts
