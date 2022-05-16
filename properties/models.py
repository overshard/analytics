import uuid

from django.db import models
from django.utils import timezone
from django.contrib.auth import get_user_model


User = get_user_model()


class Property(models.Model):
    """
    A site that we attach all our analytics hits to and connect up to a user.
    """
    id = models.UUIDField(primary_key=True, default=uuid.uuid4, editable=False)
    user = models.ForeignKey(User, on_delete=models.CASCADE, related_name='properties')
    name = models.CharField(max_length=255)
    custom_cards = models.JSONField(default=list)
    created_at = models.DateTimeField(auto_now_add=True, editable=False)
    updated_at = models.DateTimeField(auto_now=True, editable=False)
    is_protected = models.BooleanField(default=False, editable=False)
    is_public = models.BooleanField(default=False, editable=False)

    class Meta:
        verbose_name = 'Property'
        verbose_name_plural = 'Properties'

    def __str__(self):
        return self.name

    @property
    def is_active(self):
        """
        Returns True if we've recieved any events for this property in the last
        7 days.
        """
        return self.events.filter(created_at__gte=timezone.now() - timezone.timedelta(days=7)).exists()

    @property
    def total_events(self):
        return self.events.count()

    @property
    def total_session_starts(self):
        return self.events.filter(event="session_start").count()

    @property
    def total_page_views(self):
        return self.events.filter(event="page_view").count()

    @property
    def total_clicks(self):
        return self.events.filter(event="click").count()

    @property
    def total_scrolls(self):
        return self.events.filter(event="scroll").count()


class Event(models.Model):
    """
    An event that is sent by a site that we want to track. The most basic of
    events is a "page_view" event. All events can have a variety of key-value
    pairs sent along with them that we store in a JSONField.

    As an example a "page_view" may contain the following key-value pairs:

    - url: The URL of the page that was viewed
    - title: The title of the page that was viewed
    - referrer: The URL of the page that referred the user to the page that was viewed
    - user_agent: The user agent of the user that viewed the page
    - screen_width: The width of the screen of the user that viewed the page
    - screen_height: The height of the screen of the user that viewed the page

    Users are free to send any event with any key-value pairs they want.
    """
    created_at = models.DateTimeField(auto_now_add=True, editable=False)
    property = models.ForeignKey(Property, on_delete=models.CASCADE, related_name="events", editable=False)
    event = models.CharField(max_length=255, editable=False)
    data = models.JSONField(editable=False)

    def __str__(self):
        return self.event

    class Meta:
        indexes = [
            models.Index(fields=['created_at']),
        ]
