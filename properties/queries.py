from django.db.models import Avg, Count, FloatField, Q
from django.db.models.functions import Cast, TruncDate
from django.utils import timezone

from .constants import BUILT_IN_EVENTS


def human_events(events):
    """Exclude bot-tagged events from a queryset.

    SQLite's JSON NOT-EQUAL doesn't match rows where the key is missing, so
    `exclude(data__is_bot=True)` drops everything. `has_key` matches only rows
    that contain the key — and we only ever set is_bot when the UA is a bot.
    """
    return events.exclude(data__has_key="is_bot")


def total_live_users(property_obj):
    """
    Total unique user_ids seen in the last 30 minutes.
    """
    return (
        human_events(property_obj.events)
        .filter(created_at__gte=timezone.now() - timezone.timedelta(minutes=30))
        .exclude(data__user_id__isnull=True)
        .values("data__user_id")
        .distinct()
        .count()
    )


# Cap time-on-page to filter out idle-tab outliers (left open for hours).
# < 1s is likely bot/instant-exit; > 30min is almost certainly idle.
TIME_ON_PAGE_MIN_S = 1
TIME_ON_PAGE_MAX_S = 30 * 60


def _pct_change(current, previous):
    if not previous:
        return 0
    return round((current - previous) / previous * 100)


def _event_counts(qs):
    """Single-aggregate pass for the five built-in event counts + total."""
    return qs.aggregate(
        session_start=Count("id", filter=Q(event="session_start")),
        page_view=Count("id", filter=Q(event="page_view")),
        click=Count("id", filter=Q(event="click")),
        scroll=Count("id", filter=Q(event="scroll")),
        total=Count("id"),
    )


def _engaged_users(qs, session_starts):
    if not session_starts:
        return 0
    engaged = (
        qs.exclude(data__user_id__isnull=True)
        .values("data__user_id")
        .annotate(c=Count("id"))
        .filter(c__gte=10)
        .count()
    )
    return round(engaged / session_starts * 100, 2)


def _avg_time_on_page(qs):
    try:
        avg = (
            qs.filter(event="page_leave")
            .annotate(time_on_page_s=Cast("data__time_on_page", FloatField()) / 1000)
            .filter(
                time_on_page_s__gte=TIME_ON_PAGE_MIN_S,
                time_on_page_s__lte=TIME_ON_PAGE_MAX_S,
            )
            .aggregate(avg=Avg("time_on_page_s"))["avg"]
        )
        return round(avg, 2) if avg is not None else 0
    except TypeError:
        return 0


def standard_event_cards(events_filtered, events_filtered_prev):
    """Standard metric cards. Two aggregate queries plus engagement/time-on-page helpers."""
    cur = _event_counts(events_filtered)
    prev = _event_counts(events_filtered_prev)

    cards = [
        {
            "name": "Total session starts",
            "value": cur["session_start"],
            "percent_change": _pct_change(cur["session_start"], prev["session_start"]),
            "help_text": "Unique users visiting your site for your selected date range.",
        },
        {
            "name": "Total page views",
            "value": cur["page_view"],
            "percent_change": _pct_change(cur["page_view"], prev["page_view"]),
            "help_text": "Total pages viewed for your selected date range.",
        },
        {
            "name": "Total clicks",
            "value": cur["click"],
            "percent_change": _pct_change(cur["click"], prev["click"]),
            "help_text": "Total clicks users made on all your pages for your selected date range.",
        },
        {
            "name": "Total scrolls",
            "value": cur["scroll"],
            "percent_change": _pct_change(cur["scroll"], prev["scroll"]),
            "help_text": "Total scrolls users made on all your pages for your selected date range.",
        },
        {
            "name": "Total events",
            "value": cur["total"],
            "percent_change": _pct_change(cur["total"], prev["total"]),
            "help_text": "All events for your selected date range, including custom events.",
        },
    ]

    engagement_cur = _engaged_users(events_filtered, cur["session_start"])
    engagement_prev = _engaged_users(events_filtered_prev, prev["session_start"])
    cards.append({
        "name": "Total user engagement",
        "value": f"{engagement_cur}%",
        "percent_change": _pct_change(engagement_cur, engagement_prev),
        "help_text": "An engaged user is a user more than 10 events collected for your selected date range.",
    })

    time_cur = _avg_time_on_page(events_filtered)
    time_prev = _avg_time_on_page(events_filtered_prev)
    cards.append({
        "name": "Avg. time on page",
        "value": f"{time_cur}s",
        "percent_change": _pct_change(time_cur, time_prev),
        "help_text": "Average time a user spends on each page. Sessions over 30 minutes are excluded as idle.",
    })

    return cards


def custom_event_cards(property_obj, events_filtered, events_filtered_prev):
    """Returns (cards, custom_events). Custom events are non-built-in event names."""
    custom_events = list(
        human_events(property_obj.events)
        .exclude(event__in=BUILT_IN_EVENTS)
        .values("event")
        .distinct()
        .order_by("event")
    )

    active_names = {c["event"] for c in property_obj.custom_cards if c.get("value") is True}
    for ce in custom_events:
        ce["active"] = ce["event"] in active_names

    if not active_names:
        return [], custom_events

    cur = events_filtered.filter(event__in=active_names).values("event").annotate(c=Count("id"))
    prev = events_filtered_prev.filter(event__in=active_names).values("event").annotate(c=Count("id"))
    cur_map = {r["event"]: r["c"] for r in cur}
    prev_map = {r["event"]: r["c"] for r in prev}

    cards = []
    for ce in custom_events:
        if ce["event"] not in active_names:
            continue
        v = cur_map.get(ce["event"], 0)
        p = prev_map.get(ce["event"], 0)
        cards.append({
            "name": ce["event"],
            "value": v,
            "percent_change": _pct_change(v, p),
        })
    return cards, custom_events


def events_graph(events_filtered, date_end_obj, date_range):
    """
    One GROUP BY query for daily counts, then bucket into days/weeks/months
    in Python. Buckets step backwards from date_end.
    """
    rows = (
        events_filtered.annotate(day=TruncDate("created_at"))
        .values("day")
        .annotate(count=Count("id"))
    )
    by_day = {r["day"]: r["count"] for r in rows if r["day"]}

    end_date = date_end_obj.date()

    def bucket_sum(start_date, days):
        return sum(
            by_day.get(start_date + timezone.timedelta(days=j), 0)
            for j in range(days)
        )

    if date_range <= 28:
        points = [
            {
                "label": end_date - timezone.timedelta(days=i),
                "count": by_day.get(end_date - timezone.timedelta(days=i), 0),
            }
            for i in range(date_range)
        ]
    elif date_range <= 6 * 28:
        points = [
            {
                "label": end_date - timezone.timedelta(days=7 * w),
                "count": bucket_sum(end_date - timezone.timedelta(days=7 * w), 7),
            }
            for w in range(date_range // 7)
        ]
    else:
        points = [
            {
                "label": end_date - timezone.timedelta(days=28 * m),
                "count": bucket_sum(end_date - timezone.timedelta(days=28 * m), 28),
            }
            for m in range(date_range // 28)
        ]

    points.sort(key=lambda k: k["label"])
    for p in points:
        p["label"] = p["label"].strftime("%b %-d")
    return points


def _top_by_key(qs, key, limit=10, event=None):
    """Generic top-N list for a JSON key with count."""
    if event is not None:
        qs = qs.filter(event=event)
    rows = (
        qs.exclude(**{f"{key}__isnull": True})
        .exclude(**{key: ""})
        .values(key)
        .annotate(count=Count("id"))
        .order_by("-count")[:limit]
    )
    return [{"label": r[key], "count": r["count"]} for r in rows]


def events_by_screen_size(events_filtered, limit=7):
    rows = (
        events_filtered.filter(event="session_start")
        .exclude(data__screen_width__isnull=True)
        .values("data__screen_width", "data__screen_height")
        .annotate(count=Count("id"))
        .order_by("-count")[:limit]
    )
    return [
        {
            "label": f"{r['data__screen_width']}x{r['data__screen_height']}",
            "count": r["count"],
        }
        for r in rows
    ]


def events_by_device(events_filtered, limit=7):
    return _top_by_key(events_filtered, "data__device", limit, event="session_start")


def events_by_browser(events_filtered, limit=7):
    return _top_by_key(events_filtered, "data__browser", limit, event="session_start")


def events_by_platform(events_filtered, limit=7):
    return _top_by_key(events_filtered, "data__platform", limit, event="session_start")


def events_by_page_url(events_filtered, limit=10):
    return _top_by_key(events_filtered, "data__url", limit)


def page_views_by_page_url(events_filtered, limit=10):
    return _top_by_key(events_filtered, "data__url", limit, event="page_view")


def events_by_custom_event(events_filtered, limit=10):
    rows = (
        events_filtered.exclude(event__in=BUILT_IN_EVENTS)
        .values("event")
        .annotate(count=Count("id"))
        .order_by("-count")[:limit]
    )
    return [{"label": r["event"], "count": r["count"]} for r in rows]


def session_starts_by_referrer(events_filtered, limit=10):
    return _top_by_key(events_filtered, "data__referrer", limit, event="session_start")


def page_views_by_utm(events_filtered, field, limit=10):
    return _top_by_key(events_filtered, f"data__utm_{field}", limit, event="page_view")


def session_starts_by_country(events_filtered):
    """Sessions grouped by ISO 3166-1 alpha-2 country code."""
    rows = (
        events_filtered.filter(event="session_start")
        .exclude(data__country__isnull=True)
        .values("data__country")
        .annotate(count=Count("id"))
    )
    return {r["data__country"]: r["count"] for r in rows}


def session_starts_by_country_region(events_filtered):
    """
    Sessions grouped first by country, then by region within that country.

    Returned shape: {"US": {"CA": 42, "NY": 17}, "DE": {"BY": 9}, ...}.
    Used by the world map for click-to-drill-down — the whole tree ships
    with the dashboard so no extra request is needed when a country is
    selected.
    """
    rows = (
        events_filtered.filter(event="session_start")
        .exclude(data__country__isnull=True)
        .exclude(data__region__isnull=True)
        .values("data__country", "data__region")
        .annotate(count=Count("id"))
    )
    out = {}
    for r in rows:
        out.setdefault(r["data__country"], {})[r["data__region"]] = r["count"]
    return out


def bot_traffic(events_all, limit=10):
    """
    Bot-only stats for the dashboard's bot card. Takes the unfiltered (bots
    included) events queryset.
    """
    bots = events_all.filter(data__is_bot=True)
    total = bots.count()
    if not total:
        return {"total": 0, "top_bots": [], "top_pages": []}
    top_bots = list(
        bots.exclude(data__bot_name__isnull=True)
        .exclude(data__bot_name="")
        .values("data__bot_name")
        .annotate(count=Count("id"))
        .order_by("-count")[:limit]
    )
    top_pages = list(
        bots.exclude(data__url__isnull=True)
        .exclude(data__url="")
        .values("data__url")
        .annotate(count=Count("id"))
        .order_by("-count")[:limit]
    )
    return {
        "total": total,
        "top_bots": [{"label": r["data__bot_name"], "count": r["count"]} for r in top_bots],
        "top_pages": [{"label": r["data__url"], "count": r["count"]} for r in top_pages],
    }
