import json

from django.contrib.gis.geoip2 import GeoIP2
from django.contrib.gis.geoip2.base import GeoIP2Exception
from django.http import HttpResponse
from django.views.decorators.csrf import csrf_exempt
from geoip2.errors import AddressNotFoundError
from user_agents import parse as ua_parse

from properties.models import Event, Property


@csrf_exempt
def collect(request):
    """
    Processes collector events sent to our server, stores them using Event for
    the relevant Site.
    """
    body = json.loads(request.body)

    try:
        property_obj = Property.objects.get(id=body['collectorId'])
    except Property.DoesNotExist:
        return HttpResponse(status=404)

    event_obj = Event(
        property=property_obj,
        event=body['event'],
        data=body.get('data', {}),
    )

    # If we have a data__referrer then strip the url down to just the hostname
    # ex. "example.com" all lowercase.
    if 'referrer' in event_obj.data:
        # Some urls have a query string, some have a fragment, some have more
        # need to strip everything before the protocol and after the tld
        # ex. "http://example.com/foo?bar=baz#frag" -> "example.com"
        event_obj.data['referrer'] = event_obj.data['referrer'].split('://')[-1].split('/')[0].lower().replace('www.', '')

    try:
        if event_obj.event == 'session_start':
            # Check HTTP_X_FORWARDED_FOR first item after split , for the client IP
            # if it exists else use REMOTE_ADDR
            ip = request.META.get('HTTP_X_FORWARDED_FOR', request.META.get('REMOTE_ADDR')).split(',')[0]
            if ip != '127.0.0.1':
                g = GeoIP2()
                g_data = g.city(ip)
                if g_data:
                    event_obj.data['country'] = g_data['country_code']
                    event_obj.data['region'] = g_data['region']
                    event_obj.data['city'] = g_data['city']
                    event_obj.data['loc'] = [g_data['latitude'], g_data['longitude']]
    except (GeoIP2Exception, AddressNotFoundError):
        pass

    # If we have a "user_agent" in "data" then parse it and store the results in
    # data under "platform", "device" and "browser".
    ua = None
    if 'user_agent' in event_obj.data:
        ua = ua_parse(event_obj.data['user_agent'])

    # If we don't have a ua in the event_obj.data lets see if the request has
    # one to parse.
    if not ua and request.META.get('HTTP_USER_AGENT'):
        ua = ua_parse(request.META.get('HTTP_USER_AGENT'))

    if ua:
        event_obj.data['platform'] = ua.os.family
        event_obj.data['browser'] = ua.browser.family
        if ua.is_mobile:
            event_obj.data['device'] = 'Mobile'
        elif ua.is_tablet:
            event_obj.data['device'] = 'Tablet'
        else:
            event_obj.data['device'] = 'Desktop'
        if not ua.is_bot:
            # I've decided I don't want to save bots in the database but you
            # are free to change this!
            event_obj.save()
    else:
        # If we don't have a user agent let's save just in case because it might
        # be a server side event or the latest chrome which sometimes doesn't
        # have one. We do try to get userAgentData in our collector.js too which
        # gets auto set into the correct data attributes.
        event_obj.save()

    return HttpResponse(status=204)
