import json
import uuid

from django.conf import settings
from django.contrib import messages
from django.core.cache import cache
from django.core.files.storage import default_storage
from django.http import HttpResponse, JsonResponse
from django.shortcuts import redirect, render
from django.template.loader import render_to_string
from django.utils import timezone

from analytics.chromium import generate_pdf_from_html

from . import queries as q
from .forms import PropertyForm
from .models import Property


DASHBOARD_CACHE_TTL = 300  # seconds


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
    search = request.GET.get("q", None)
    if search:
        properties = properties.filter(name__icontains=search)

    return render(
        request,
        "properties/properties.html",
        {
            "form": form,
            "title": "Properties",
            "description": "Manage your properties.",
            "properties": properties,
            "q": search,
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
        return JsonResponse({"success": True})

    return JsonResponse({"success": False})


def adjust_is_public_property(request, property_id):
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


def _dashboard_context(property_obj, date_start_obj, date_end_obj, date_range, filter_url):
    """
    Heavy context built from DB queries. Cacheable — no request-specific state.
    """
    events_all = property_obj.events.filter(
        created_at__gte=date_start_obj, created_at__lte=date_end_obj
    )
    if filter_url:
        events_all = events_all.filter(data__url=filter_url)

    events_filtered = q.human_events(events_all)

    date_start_obj_prev = date_start_obj - timezone.timedelta(days=date_range)
    date_end_obj_prev = date_end_obj - timezone.timedelta(days=date_range)
    events_filtered_prev = q.human_events(
        property_obj.events.filter(
            created_at__gte=date_start_obj_prev, created_at__lte=date_end_obj_prev
        )
    )
    if filter_url:
        events_filtered_prev = events_filtered_prev.filter(data__url=filter_url)

    event_cards = list(q.standard_event_cards(events_filtered, events_filtered_prev))
    custom_cards, custom_events = q.custom_event_cards(
        property_obj, events_filtered, events_filtered_prev
    )
    event_cards.extend(custom_cards)

    return {
        "event_cards": event_cards,
        "custom_events": custom_events,
        "total_events_graph": q.events_graph(events_filtered, date_end_obj, date_range),
        "total_events_by_screen_size": q.events_by_screen_size(events_filtered),
        "total_events_by_device": q.events_by_device(events_filtered),
        "total_events_by_browser": q.events_by_browser(events_filtered),
        "total_events_by_platform": q.events_by_platform(events_filtered),
        "total_events_by_page_url": q.events_by_page_url(events_filtered),
        "total_page_views_by_page_url": q.page_views_by_page_url(events_filtered),
        "total_events_by_custom_event": q.events_by_custom_event(events_filtered),
        "total_session_starts_by_referrer": q.session_starts_by_referrer(events_filtered),
        "total_page_views_by_utm_medium": q.page_views_by_utm(events_filtered, "medium"),
        "total_page_views_by_utm_source": q.page_views_by_utm(events_filtered, "source"),
        "total_page_views_by_utm_campaign": q.page_views_by_utm(events_filtered, "campaign"),
        "session_starts_by_country": q.session_starts_by_country(events_filtered),
        "session_starts_by_country_region": q.session_starts_by_country_region(events_filtered),
        "bot_traffic": q.bot_traffic(events_all),
    }


def property(request, property_id):
    try:
        property_obj = Property.objects.get(pk=property_id)
    except Property.DoesNotExist:
        return redirect("properties")

    if not property_obj.is_public and property_obj.user != request.user:
        return redirect("properties")

    date_start = request.GET.get(
        "date_start",
        (timezone.now() - timezone.timedelta(days=28)).strftime("%Y-%m-%d"),
    )
    date_end = request.GET.get("date_end", timezone.now().strftime("%Y-%m-%d"))
    date_range = request.GET.get("date_range", 28)

    date_start_obj = timezone.make_aware(
        timezone.datetime.strptime(date_start, "%Y-%m-%d"),
        timezone.get_current_timezone(),
    )
    date_end_obj = timezone.make_aware(
        timezone.datetime.strptime(date_end, "%Y-%m-%d")
        + timezone.timedelta(hours=23, minutes=59, seconds=59),
        timezone.get_current_timezone(),
    )

    if date_range == "custom":
        date_range = (date_end_obj - date_start_obj).days
    else:
        date_range = int(date_range)

    filter_url = request.GET.get("filter_url", None)

    # Heavy DB work goes through a 5-min cache. Live users stays uncached so
    # "live" actually means live. ?report bypasses the cache so exports match
    # whatever the user sees in the dashboard right now.
    # Include updated_at so custom-cards/visibility changes bust the cache.
    ver = int(property_obj.updated_at.timestamp())
    cache_key = (
        f"dash:{property_obj.id}:{ver}:{date_start}:{date_end}:{date_range}:{filter_url or ''}"
    )
    if "report" in request.GET:
        dashboard = _dashboard_context(property_obj, date_start_obj, date_end_obj, date_range, filter_url)
    else:
        dashboard = cache.get(cache_key)
        if dashboard is None:
            dashboard = _dashboard_context(property_obj, date_start_obj, date_end_obj, date_range, filter_url)
            cache.set(cache_key, dashboard, DASHBOARD_CACHE_TTL)

    context = {
        "property": property_obj,
        "title": property_obj.name,
        "description": "Analytics for " + property_obj.name,
        "BASE_URL": settings.BASE_URL,
        "date_start": date_start,
        "date_end": date_end,
        "date_range": date_range,
        "filter_url": filter_url,
        "total_live_users": q.total_live_users(property_obj),
        **dashboard,
    }

    if "report" in request.GET:
        fmt = request.GET.get("report") or "pdf"
        if fmt == "md":
            md = render_to_string("properties/property_report.md", context)
            response = HttpResponse(md, content_type="text/markdown; charset=utf-8")
            response["Content-Disposition"] = f'inline; filename="{property_obj.name}.md"'
            return response
        if fmt == "pdf":
            context["print"] = True
            html = render_to_string("properties/property.html", context)
            filename = f"reports/{uuid.uuid4()}.pdf"
            generate_pdf_from_html(html, filename)
            with open(default_storage.path(filename), "rb") as pdf:
                response = HttpResponse(pdf.read(), content_type="application/pdf")
                response["Content-Disposition"] = "inline; filename=report.pdf"
                return response

    return render(request, "properties/property.html", context)
