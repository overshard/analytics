from django.db import models
from django.db.models.functions import Cast
from django.utils import timezone


def total_live_users(property_obj):
    """
    Returns the total number of live users for a given property within the last
    30 minutes.

    :param property_obj: Property object
    """
    return (
        property_obj.events.filter(
            created_at__gte=timezone.now() - timezone.timedelta(minutes=30)
        )
        .exclude(data__user_id__isnull=True)
        .values("data__user_id")
        .distinct()
        .count()
    )


def standard_event_cards(events_filtered, events_filtered_prev):
    """
    Returns the standard event cards for a given property.

    :param events_filtered: Filtered events
    :param events_filtered_prev: Filtered events from previous period
    :return: Event cards array
    """
    event_cards = []

    total_session_starts = events_filtered.filter(event="session_start").count()
    total_session_starts_prev = events_filtered_prev.filter(event="session_start").count()
    event_cards.append({
        "name": "Total session starts",
        "value": total_session_starts,
        "percent_change": round((total_session_starts - total_session_starts_prev) / total_session_starts_prev * 100) if total_session_starts_prev > 0 else 0,
        "help_text": "Unique users visiting your site for your selected date range.",
    })

    total_page_views = events_filtered.filter(event="page_view").count()
    total_page_views_prev = events_filtered_prev.filter(event="page_view").count()
    event_cards.append({
        "name": "Total page views",
        "value": total_page_views,
        "percent_change": round((total_page_views - total_page_views_prev) / total_page_views_prev * 100) if total_page_views_prev else 0,
        "help_text": "Total pages viewed for your selected date range.",
    })

    total_clicks = events_filtered.filter(event="click").count()
    total_clicks_prev = events_filtered_prev.filter(event="click").count()
    event_cards.append({
        "name": "Total clicks",
        "value": total_clicks,
        "percent_change": round((total_clicks - total_clicks_prev) / total_clicks_prev * 100) if total_clicks_prev > 0 else 0,
        "help_text": "Total clicks users made on all your pages for your selected date range.",
    })

    total_scrolls = events_filtered.filter(event="scroll").count()
    total_scrolls_prev = events_filtered_prev.filter(event="scroll").count()
    event_cards.append({
        "name": "Total scrolls",
        "value": total_scrolls,
        "percent_change": round((total_scrolls - total_scrolls_prev) / total_scrolls_prev * 100) if total_scrolls_prev > 0 else 0,
        "help_text": "Total scrolls users made on all your pages for your selected date range.",
    })

    total_events = events_filtered.count()
    total_events_prev = events_filtered_prev.count()
    event_cards.append({
        "name": "Total events",
        "value": total_events,
        "percent_change": round((total_events - total_events_prev) / total_events_prev * 100) if total_events_prev else 0,
        "help_text": "All events for your selected date range, including custom events.",
    })

    try:
        total_unique_users_with_events = (
            events_filtered.exclude(data__user_id__isnull=True)
            .values("data__user_id")
            .distinct()
            .annotate(count=models.Count("data__user_id"))
            .filter(count__gte=10)
            .count()
        )
        total_user_engagement = round(
            total_unique_users_with_events / total_session_starts * 100, 2
        )
    except ZeroDivisionError:
        total_user_engagement = 0
    try:
        total_unique_users_with_events_prev = (
            events_filtered_prev.exclude(data__user_id__isnull=True)
            .values("data__user_id")
            .distinct()
            .annotate(count=models.Count("data__user_id"))
            .filter(count__gte=10)
            .count()
        )
        total_user_engagement_prev = round(
            total_unique_users_with_events_prev / total_session_starts_prev * 100, 2
        )
    except ZeroDivisionError:
        total_user_engagement_prev = 0
    event_cards.append({
        "name": "Total user engagement",
        "value": f"{total_user_engagement}%",
        "percent_change": round((total_user_engagement - total_user_engagement_prev) / total_user_engagement_prev * 100) if total_user_engagement_prev else 0,
        "help_text": "An engaged user is a user more than 10 events collected for your selected date range.",
    })

    # Cap time-on-page to filter out idle-tab outliers (left open for hours).
    # Anything < 1s is likely bot/instant-exit; anything > 30min is almost
    # certainly an idle/abandoned tab rather than real engagement.
    TIME_ON_PAGE_MIN_S = 1
    TIME_ON_PAGE_MAX_S = 30 * 60

    def _avg_time_on_page(qs):
        try:
            avg = qs.filter(event="page_leave").annotate(
                time_on_page_s=Cast("data__time_on_page", models.FloatField()) / 1000
            ).filter(
                time_on_page_s__gte=TIME_ON_PAGE_MIN_S,
                time_on_page_s__lte=TIME_ON_PAGE_MAX_S,
            ).aggregate(avg=models.Avg("time_on_page_s"))["avg"]
            return round(avg, 2) if avg is not None else 0
        except TypeError:
            return 0

    avg_time_on_page = _avg_time_on_page(events_filtered)
    avg_time_on_page_prev = _avg_time_on_page(events_filtered_prev)
    event_cards.append({
        "name": "Avg. time on page",
        "value": f"{avg_time_on_page}s",
        "percent_change": round((avg_time_on_page - avg_time_on_page_prev) / avg_time_on_page_prev * 100) if avg_time_on_page_prev else 0,
        "help_text": "Average time a user spends on each page. Sessions over 30 minutes are excluded as idle.",
    })

    return event_cards


def custom_event_cards(property_obj, events_filtered, events_filtered_prev):
    """
    Returns the custom event cards for a given property.

    :param events_filtered: Filtered events
    :param events_filtered_prev: Filtered events from previous period
    :return: A list of custom events and a list of custom event cards
    """
    event_cards = []

    custom_events = property_obj.events.exclude(
        event__in=["session_start", "page_view", "page_leave", "click", "scroll"]
    ).values("event").distinct().order_by("event")

    active_cards = []
    for card in property_obj.custom_cards:
        if card['value'] is True:
            active_cards.append(card['event'])

    # add active = True or active = False to custom_events if in active_cards
    for card in custom_events:
        if card['event'] in active_cards:
            card['active'] = True
        else:
            card['active'] = False

    for custom_event in custom_events:
        if custom_event["event"] not in active_cards:
            continue
        total_events = events_filtered.filter(event=custom_event["event"]).count()
        total_events_prev = events_filtered_prev.filter(event=custom_event["event"]).count()
        event_cards.append({
            "name": custom_event["event"],
            "value": total_events,
            "percent_change": round((total_events - total_events_prev) / total_events_prev * 100) if total_events_prev else 0,
        })

    return event_cards, custom_events
