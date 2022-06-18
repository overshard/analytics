from django.conf import settings

from properties.models import Property


def collector(request):
    """
    Gets the id of our "Proprium" property and sets it as a context variable
    to be used to collect metrics from ourselves.
    """
    try:
        prop = Property.objects.get(name='Proprium')
        return {'collector_server': settings.BASE_URL, 'collector_id': prop.id}
    except Property.DoesNotExist:
        return {}


def canonical(request):
    """
    Gets the canonical URL for the current request.
    """
    return {'canonical': request.build_absolute_uri(request.path)}


def base_url(request):
    """
    Provides the BASE_URL from settings.
    """
    return {'BASE_URL': settings.BASE_URL}
