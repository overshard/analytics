import json
import uuid

from django.conf import settings
from django.contrib import messages
from django.core.files.storage import default_storage
from django.db import models
from django.http import HttpResponse, JsonResponse
from django.shortcuts import redirect, render
from django.template.loader import render_to_string
from django.utils import timezone

from analytics.playwright import generate_pdf_from_html

from . import queries as q
from .forms import PropertyForm
from .models import Property


def properties(request):
    if not request.user.is_authenticated:
        return redirect("/")

    if request.method == "POST":
        form = PropertyForm(request.POST)
        if form.is_valid():
            new_property = form.save(commit=False)
            new_property.user = request.user
            new_property.save()
            messages.success(request, "Property added successfully.")
            return redirect("properties")
    else:
        form = PropertyForm()

    properties = request.user.properties.all()
    q = request.GET.get("q", None)
    if q:
        properties = properties.filter(name__icontains=q)

    return render(
        request,
        "properties/properties.html",
        {
            "form": form,
            "title": "Properties",
            "description": "Manage your properties.",
            "properties": properties,
            "q": q,
        },
    )


def property_delete(request, property_id):
    if not request.user.is_authenticated:
        return redirect("/")

    try:
        property_obj = request.user.properties.get(pk=property_id)
    except Property.DoesNotExist:
        return redirect("properties")

    property_obj.delete()
    messages.success(request, "Property deleted successfully.")
    return redirect("properties")


def adjust_custom_event_cards(request, property_id):
    """
    Adds and removes custom event cards on a property
    """
    if not request.user.is_authenticated:
        return redirect("/")

    try:
        property_obj = request.user.properties.get(pk=property_id)
    except Property.DoesNotExist:
        return redirect("properties")

    if request.method == "POST":
        custom_cards = json.loads(request.body.decode("utf-8"))
        property_obj.custom_cards = custom_cards
        property_obj.save()
        print(property_obj.custom_cards)
        return JsonResponse({"success": True})

    return JsonResponse({"success": False})


def adjust_is_public_property(request, property_id):
    """
    Sets the property to public or private
    """
    if not request.user.is_authenticated:
        return redirect("/")

    try:
        property_obj = request.user.properties.get(pk=property_id)
    except Property.DoesNotExist:
        return redirect("properties")

    if request.method == "POST":
        property_obj.is_public = property_obj.is_public is False
        property_obj.save()
        return JsonResponse({"success": True})

    return JsonResponse({"success": False})


def property(request, property_id):
    context = {}

    # Get the property and check permissions
    try:
        property_obj = Property.objects.get(pk=property_id)
        context["property"] = property_obj
    except Property.DoesNotExist:
        return redirect("properties")

    if not property_obj.is_public and property_obj.user != request.user:
        return redirect("properties")

    # Set some basic page context variables
    context["title"] = property_obj.name
    context["description"] = "Analytics for " + property_obj.name
    context["BASE_URL"] = settings.BASE_URL

    # Date range filter which defaults to 28 days if nothing is selected
    date_start = request.GET.get(
        "date_start",
        (timezone.now() - timezone.timedelta(days=28)).strftime("%Y-%m-%d"),
    )
    date_end = request.GET.get("date_end", timezone.now().strftime("%Y-%m-%d"))
    date_range = request.GET.get("date_range", 28)

    context["date_start"] = date_start
    context["date_end"] = date_end
    context["date_range"] = date_range

    date_start_obj = timezone.datetime.strptime(date_start, "%Y-%m-%d")
    date_end_obj = timezone.datetime.strptime(
        date_end, "%Y-%m-%d"
    ) + timezone.timedelta(hours=23, minutes=59, seconds=59)

    # Set the timezone
    date_start_obj = timezone.make_aware(
        date_start_obj, timezone.get_current_timezone()
    )
    date_end_obj = timezone.make_aware(date_end_obj, timezone.get_current_timezone())

    if date_range == "custom":
        date_range = (date_end_obj - date_start_obj).days
    else:
        date_range = int(date_range)

    # Get the current period based on the date range
    events_filtered = property_obj.events.filter(
        created_at__gte=date_start_obj, created_at__lte=date_end_obj
    )

    # Get the filter_url and filter by data__url if filter_url exists
    filter_url = request.GET.get("filter_url", None)
    if filter_url:
        events_filtered = events_filtered.filter(data__url=filter_url)
        context['filter_url'] = filter_url

    # Get the previous period as well for comparisons
    date_start_obj_prev = date_start_obj - timezone.timedelta(days=date_range)
    date_end_obj_prev = date_end_obj - timezone.timedelta(days=date_range)
    events_filtered_prev = property_obj.events.filter(
        created_at__gte=date_start_obj_prev, created_at__lte=date_end_obj_prev
    )

    # Start querying our data
    context["total_live_users"] = q.total_live_users(property_obj)

    event_cards = []
    event_cards.extend(q.standard_event_cards(events_filtered, events_filtered_prev))
    custom_event_cards, custom_events = q.custom_event_cards(
        property_obj, events_filtered, events_filtered_prev
    )
    event_cards.extend(custom_event_cards)
    context["custom_events"] = custom_events
    context["event_cards"] = event_cards

    #
    # Total events per day graph
    #
    # If less than 28 days show each day, if less than 6 months then show
    # each week. If less than 24 months then show each month. If more than 24
    # months then show each year.

    if date_range <= 28:
        total_events_by_day = []
        for day in range(date_range):
            date = date_end_obj - timezone.timedelta(days=day)
            count = events_filtered.filter(created_at__date=date).count()
            total_events_by_day.append({"label": date, "count": count})
        context["total_events_graph"] = sorted(
            total_events_by_day, key=lambda k: k["label"]
        )
    elif date_range <= 6 * 28:
        total_events_by_week = []
        # group weeks sunday through saturday
        for week in range(date_range // 7):
            date = date_end_obj - timezone.timedelta(days=7 * week)
            count = events_filtered.filter(
                created_at__gte=date, created_at__lte=date + timezone.timedelta(days=6)
            ).count()
            total_events_by_week.append({"label": date, "count": count})
        context["total_events_graph"] = sorted(
            total_events_by_week, key=lambda k: k["label"]
        )
    else:
        total_events_by_month = []
        # group months 1 through 31
        for month in range(date_range // 28):
            date = date_end_obj - timezone.timedelta(days=28 * month)
            count = events_filtered.filter(
                created_at__gte=date, created_at__lte=date + timezone.timedelta(days=27)
            ).count()
            total_events_by_month.append({"label": date, "count": count})
        context["total_events_graph"] = sorted(
            total_events_by_month, key=lambda k: k["label"]
        )
    for day in context["total_events_graph"]:
        day["label"] = day["label"].strftime("%b %-d")

    #
    # Total events by screen size graph
    #

    total_events_by_screen_size = []
    for event in (
        events_filtered.filter(event="session_start")
        .exclude(data__screen_width__isnull=True)
        .values("data__screen_width", "data__screen_height")
        .annotate(count=models.Count("data__screen_width"))
        .order_by("-count")[:10]
    ):
        total_events_by_screen_size.append(
            {
                "label": str(event["data__screen_width"])
                + "x"
                + str(event["data__screen_height"]),
                "count": event["count"],
            }
        )
    context["total_events_by_screen_size"] = total_events_by_screen_size[:7]

    #
    # Total events by device graph
    #

    total_events_by_device = []
    for event in (
        events_filtered.filter(event="session_start")
        .exclude(data__device__isnull=True)
        .values("data__device")
        .annotate(count=models.Count("data__device"))
        .order_by("-count")[:10]
    ):
        if event["data__device"] != "" and event["data__device"] is not None:
            total_events_by_device.append(
                {"label": event["data__device"], "count": event["count"]}
            )
    context["total_events_by_device"] = total_events_by_device[:7]

    #
    # Total events by browser graph
    #

    total_events_by_browser = []
    for event in (
        events_filtered.filter(event="session_start")
        .exclude(data__browser__isnull=True)
        .values("data__browser")
        .annotate(count=models.Count("data__browser"))
        .order_by("-count")[:10]
    ):
        if event["data__browser"] != "" and event["data__browser"] is not None:
            total_events_by_browser.append(
                {"label": event["data__browser"], "count": event["count"]}
            )
    context["total_events_by_browser"] = total_events_by_browser[:7]

    #
    # Total events by platform graph
    #

    total_events_by_platform = []
    for event in (
        events_filtered.filter(event="session_start")
        .exclude(data__platform__isnull=True)
        .values("data__platform")
        .annotate(count=models.Count("data__platform"))
        .order_by("-count")[:10]
    ):
        if event["data__platform"] != "" and event["data__platform"] is not None:
            total_events_by_platform.append(
                {"label": event["data__platform"], "count": event["count"]}
            )
    context["total_events_by_platform"] = total_events_by_platform[:7]

    #
    # Total events by page url list
    #

    total_events_by_page_url = []
    for page_view in (
        events_filtered.filter(data__url__isnull=False)
        .values("data__url")
        .annotate(count=models.Count("data__url"))
        .order_by("-count")[:10]
    ):
        total_events_by_page_url.append(
            {"label": page_view["data__url"], "count": page_view["count"]}
        )
    context["total_events_by_page_url"] = total_events_by_page_url

    #
    # Total events by page view
    #

    total_page_views_by_page_url = []
    for page_view in (
        events_filtered.filter(event="page_view")
        .exclude(data__url__isnull=True)
        .values("data__url")
        .annotate(count=models.Count("data__url"))
        .order_by("-count")[:10]
    ):
        total_page_views_by_page_url.append(
            {"label": page_view["data__url"], "count": page_view["count"]}
        )
    context["total_page_views_by_page_url"] = total_page_views_by_page_url

    #
    # Total events by custom event list
    #

    total_events_by_custom_event = []
    for custom_event in (
        events_filtered.exclude(
            event__in=["session_start", "page_view", "page_leave", "click", "scroll"]
        )
        .values("event")
        .annotate(count=models.Count("event"))
        .order_by("-count")[:10]
    ):
        total_events_by_custom_event.append(
            {"label": custom_event["event"], "count": custom_event["count"]}
        )
    context["total_events_by_custom_event"] = total_events_by_custom_event

    #
    # Total session starts by referrer list
    #

    total_session_starts_by_referrer = []
    for referrer in (
        events_filtered.filter(event="session_start")
        .exclude(data__referrer__isnull=True, data__referrer="")
        .values("data__referrer")
        .annotate(count=models.Count("data__referrer"))
        .order_by("-count")[:10]
    ):
        if referrer["data__referrer"] != "" and referrer["data__referrer"] is not None:
            total_session_starts_by_referrer.append(
                {"label": referrer["data__referrer"], "count": referrer["count"]}
            )
    context["total_session_starts_by_referrer"] = total_session_starts_by_referrer

    #
    # Total page views by utm_medium list
    #

    total_page_views_by_utm_medium = []
    for utm_medium in (
        events_filtered.filter(event="page_view")
        .exclude(data__utm_medium__isnull=True, data__utm_medium="")
        .values("data__utm_medium")
        .annotate(count=models.Count("data__utm_medium"))
        .order_by("-count")[:10]
    ):
        if (
            utm_medium["data__utm_medium"] != ""
            and utm_medium["data__utm_medium"] is not None
        ):
            total_page_views_by_utm_medium.append(
                {"label": utm_medium["data__utm_medium"], "count": utm_medium["count"]}
            )
    context["total_page_views_by_utm_medium"] = total_page_views_by_utm_medium

    #
    # Total page views by utm_source list
    #

    total_page_views_by_utm_source = []
    for utm_source in (
        events_filtered.filter(event="page_view")
        .exclude(data__utm_source__isnull=True, data__utm_source="")
        .values("data__utm_source")
        .annotate(count=models.Count("data__utm_source"))
        .order_by("-count")[:10]
    ):
        if (
            utm_source["data__utm_source"] != ""
            and utm_source["data__utm_source"] is not None
        ):
            total_page_views_by_utm_source.append(
                {"label": utm_source["data__utm_source"], "count": utm_source["count"]}
            )
    context["total_page_views_by_utm_source"] = total_page_views_by_utm_source

    #
    # Total page views by utm_campaign list
    #

    total_page_views_by_utm_campaign = []
    for utm_campaign in (
        events_filtered.filter(event="page_view")
        .exclude(data__utm_campaign__isnull=True, data__utm_campaign="")
        .values("data__utm_campaign")
        .annotate(count=models.Count("data__utm_campaign"))
        .order_by("-count")[:10]
    ):
        if (
            utm_campaign["data__utm_campaign"] != ""
            and utm_campaign["data__utm_campaign"] is not None
        ):
            total_page_views_by_utm_campaign.append(
                {
                    "label": utm_campaign["data__utm_campaign"],
                    "count": utm_campaign["count"],
                }
            )
    context["total_page_views_by_utm_campaign"] = total_page_views_by_utm_campaign

    #
    # Total session starts by region list
    #

    total_session_starts_by_region = []
    for region in (
        events_filtered.filter(event="session_start")
        .exclude(data__region__isnull=True)
        .values("data__region")
        .annotate(count=models.Count("data__region"))
        .order_by("-count")
    ):
        total_session_starts_by_region.append(
            {"label": region["data__region"], "count": region["count"]}
        )
    context["total_session_starts_by_region"] = total_session_starts_by_region[:10]

    #
    # Total session starts by region map
    #

    # For datamaps we want to convert each state label to a two letter
    # abbreviation and format the data in a dict such as:
    # {'AL': {'count': 23}, 'AK': {'count': 14}, '...'}
    us_states = {
        "Alabama": "AL",
        "Alaska": "AK",
        "Arizona": "AZ",
        "Arkansas": "AR",
        "California": "CA",
        "Colorado": "CO",
        "Connecticut": "CT",
        "Delaware": "DE",
        "Florida": "FL",
        "Georgia": "GA",
        "Hawaii": "HI",
        "Idaho": "ID",
        "Illinois": "IL",
        "Indiana": "IN",
        "Iowa": "IA",
        "Kansas": "KS",
        "Kentucky": "KY",
        "Louisiana": "LA",
        "Maine": "ME",
        "Maryland": "MD",
        "Massachusetts": "MA",
        "Michigan": "MI",
        "Minnesota": "MN",
        "Mississippi": "MS",
        "Missouri": "MO",
        "Montana": "MT",
        "Nebraska": "NE",
        "Nevada": "NV",
        "New Hampshire": "NH",
        "New Jersey": "NJ",
        "New Mexico": "NM",
        "New York": "NY",
        "North Carolina": "NC",
        "North Dakota": "ND",
        "Ohio": "OH",
        "Oklahoma": "OK",
        "Oregon": "OR",
        "Pennsylvania": "PA",
        "Rhode Island": "RI",
        "South Carolina": "SC",
        "South Dakota": "SD",
        "Tennessee": "TN",
        "Texas": "TX",
        "Utah": "UT",
        "Vermont": "VT",
        "Virginia": "VA",
        "Washington": "WA",
        "West Virginia": "WV",
        "Wisconsin": "WI",
        "Wyoming": "WY",
        "District of Columbia": "DC",
        "American Samoa": "AS",
        "Guam": "GU",
        "Northern Mariana Islands": "MP",
        "Puerto Rico": "PR",
        "United States Minor Outlying Islands": "UM",
        "U.S. Virgin Islands": "VI",
    }
    total_session_starts_by_region_chart_data = {}
    for region in total_session_starts_by_region:
        if region["label"] in us_states:
            total_session_starts_by_region_chart_data[us_states[region["label"]]] = {
                "numberOfThings": region["count"]
            }
        elif region["label"] in us_states.values():
            total_session_starts_by_region_chart_data[region["label"]] = {
                "numberOfThings": region["count"]
            }
    context[
        "total_session_starts_by_region_chart_data"
    ] = total_session_starts_by_region_chart_data

    if request.GET.get("report") == "":
        context["print"] = True
        html = render_to_string("properties/property.html", context)
        filename = f"reports/{uuid.uuid4()}.pdf"
        generate_pdf_from_html(html, filename)
        with open(default_storage.path(filename), "rb") as pdf:
            response = HttpResponse(pdf.read(), content_type="application/pdf")
            response["Content-Disposition"] = "inline; filename=report.pdf"
            return response

    return render(request, "properties/property.html", context)
