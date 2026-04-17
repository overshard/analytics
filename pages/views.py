from django.shortcuts import render, redirect
from django.http import HttpResponse

from properties.models import Event, Property
from accounts.models import User


def home(request):
    if request.user.is_authenticated:
        return redirect('properties')

    context = {}
    context['title'] = 'Home'
    context['description'] = 'Made by Isaac Bythewood, simple analytics for people who want to host their own and hack on it a bit.'

    total_events = Event.objects.all().count()
    context['total_events'] = total_events

    total_properties = Property.objects.all().count()
    context['total_properties'] = total_properties

    total_users = User.objects.all().count()
    context['total_users'] = total_users

    first_event_created_at = Event.objects.all().order_by('created_at').first().created_at
    context['first_event_created_at'] = first_event_created_at

    return render(request, 'pages/home.html', context)


def changelog(request):
    context = {}
    context['title'] = 'Changelog'
    context['description'] = 'An ongoing changelog and upcoming list of features for Analytics.'
    return render(request, 'pages/changelog.html', context)


def documentation(request):
    context = {}
    context['title'] = 'Documentation'
    context['description'] = 'Documentation for Analytics.'
    return render(request, 'pages/documentation.html', context)


def favicon(request):
    # Rising-bars mark — a four-bar histogram in mossy green with an amber
    # cap on the tallest column. Matches the in-app logo, evokes "aggregate
    # counts over time" which is what Analytics actually tracks.
    svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">'
        '<rect x="6"  y="38" width="10" height="22" rx="1.5" fill="#6b9e78"/>'
        '<rect x="20" y="28" width="10" height="32" rx="1.5" fill="#6b9e78"/>'
        '<rect x="34" y="18" width="10" height="42" rx="1.5" fill="#6b9e78"/>'
        '<rect x="48" y="8"  width="10" height="52" rx="1.5" fill="#6b9e78"/>'
        '<rect x="48" y="8"  width="10" height="6"  rx="1.5" fill="#c9a84c"/>'
        "</svg>"
    )
    return HttpResponse(svg, content_type="image/svg+xml")


def robots(request):
    return render(request, 'robots.txt', content_type='text/plain')


def sitemap(request):
    return render(request, 'sitemap.xml', content_type='text/xml')
