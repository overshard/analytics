import random

from django.db import migrations
from django.utils import timezone


def create_proprium(apps, schema_editor):
    """
    Makes an initial Property named 'Proprium' for us to use for our own
    analytics. Sets the user to our initial "admin" user.
    """
    User = apps.get_model('accounts', 'User')
    Property = apps.get_model('properties', 'Property')

    user = User.objects.get(username='admin')
    Property.objects.create(name='Proprium', user=user, is_protected=True)


def create_exemplar(apps, schema_editor):
    """
    Makes an initial Property named 'Exemplar' for us to add a bunch of sample
    events to it for testing. Sets the user to our initial "admin" user.
    """
    User = apps.get_model('accounts', 'User')
    Property = apps.get_model('properties', 'Property')
    Event = apps.get_model('properties', 'Event')

    user = User.objects.get(username='admin')
    property = Property.objects.create(name='Exemplar', user=user)

    url_paths = [
        {'path': '/', 'title': 'Home'},
        {'path': '/about', 'title': 'About'},
        {'path': '/contact', 'title': 'Contact'},
        {'path': '/blog', 'title': 'Blog'},
        {'path': '/blog/post-1', 'title': 'Post 1'},
        {'path': '/blog/post-2', 'title': 'Post 2'},
        {'path': '/blog/post-3', 'title': 'Post 3'},
    ]

    screen_sizes = [
        {'width': 1024, 'height': 768},
        {'width': 1280, 'height': 1024},
        {'width': 1366, 'height': 768},
        {'width': 1920, 'height': 1080},
        {'width': 2560, 'height': 1440},
    ]

    platforms = [
        'Windows',
        'Macintosh',
        'Linux',
        'Android',
        'iOS',
    ]

    user_ids = [
        random.randint(100000000, 999999999) for _ in range(random.randint(100, 150))
    ]

    custom_events = [
        "Newsletter Signup",
        "Checkout Success",
        "New User Signup",
    ]

    referrer_urls = [
        'google.com',
        'bing.com',
        'yahoo.com',
        'duckduckgo.com',
        'ask.com',
        'baidu.com',
        'yandex.com',
        'facebook.com',
        'twitter.com',
        'linkedin.com',
        'reddit.com',
        'pinterest.com',
        'youtube.com',
        'instagram.com',
        'flickr.com',
        'tumblr.com',
    ]

    # Create a list of dicts that include "city", "region", "country", and
    # "loc" which is "lat,lon" in the US.
    random_locations = [
        { 'region': 'New York', },
        { 'region': 'California', },
        { 'region': 'Texas', },
        { 'region': 'North Carolina', },
        { 'region': 'Florida', },
        { 'region': 'Illinois', },
        { 'region': 'Ohio', },
        { 'region': 'Michigan', },
        { 'region': 'Pennsylvania', },
        { 'region': 'Georgia', },
        { 'region': 'New Jersey', },
        { 'region': 'Virginia', },
        { 'region': 'North Dakota', },
        { 'region': 'South Carolina', },
        { 'region': 'Indiana', },
    ]

    random_utm_medium = [
        'email',
        'social',
        'search',
        'referral',
        'paid',
    ]

    random_utm_source = [
        'google',
        'bing',
        'duckduckgo',
        'facebook',
        'twitter',
        'instagram',
        'linkedin',
        'reddit',
    ]

    random_utm_campaign = [
        '2022 search',
        '2022 email',
        '2022 social',
        '2022 referral',
    ]

    # Generate some session_start events
    for user_id in user_ids:
        page = random.choice(url_paths)
        screen = random.choice(screen_sizes)
        location = random.choice(random_locations)
        event = Event.objects.create(
            property=property,
            event='session_start',
            data={
                'user_id': user_id,
                'url': page['path'],
                'title': page['title'],
                'referrer': random.choice(referrer_urls),
                'user_agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_12_6) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/61.0.3163.100 Safari/537.36',
                'screen_width': screen['width'],
                'screen_height': screen['height'],
                'platform': random.choice(platforms),
                'device': random.choice(['Desktop', 'Tablet', 'Mobile']),
                'region': location['region'],
            },
        )
        Event.objects.filter(id=event.id).update(created_at=timezone.now() - timezone.timedelta(days=random.randint(0, 56)))

    # Generate some session_start events from random users to drive down user engagement
    for _ in range(random.randint(50, 100)):
        page = random.choice(url_paths)
        screen = random.choice(screen_sizes)
        event = Event.objects.create(
            property=property,
            event='session_start',
            data={
                'user_id': random.randint(100000000, 999999999),
                'url': page['path'],
                'title': page['title'],
                'referrer': random.choice(referrer_urls),
                'user_agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_12_6) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/61.0.3163.100 Safari/537.36',
                'screen_width': screen['width'],
                'screen_height': screen['height'],
                'platform': random.choice(platforms),
                'device': random.choice(['Desktop', 'Tablet', 'Mobile']),
                'browser': random.choice(['Chrome', 'Firefox', 'Safari', 'Edge', 'Opera']),
            },
        )
        Event.objects.filter(id=event.id).update(created_at=timezone.now() - timezone.timedelta(days=random.randint(0, 56)))

    # Generate some page_view events
    for _ in range(random.randint(300, 600)):
        page = random.choice(url_paths)
        event = Event.objects.create(
            property=property,
            event='page_view',
            data={
                'user_id': random.choice(user_ids),
                'url': page['path'],
                'title': page['title'],
                'utm_medium': random.choice(random_utm_medium),
                'utm_source': random.choice(random_utm_source),
                'utm_campaign': random.choice(random_utm_campaign),
            },
        )
        Event.objects.filter(id=event.id).update(created_at=timezone.now() - timezone.timedelta(days=random.randint(0, 56)))


    # Generate some scroll events
    for _ in range(random.randint(300, 600)):
        page = random.choice(url_paths)
        event = Event.objects.create(
            property=property,
            event='scroll',
            data={
                'user_id': random.choice(user_ids),
                'url': page['path'],
                'title': page['title'],
            },
        )
        Event.objects.filter(id=event.id).update(created_at=timezone.now() - timezone.timedelta(days=random.randint(0, 56)))

    # Generate some click events
    for _ in range(random.randint(300, 600)):
        page = random.choice(url_paths)
        screen = random.choice(screen_sizes)
        event = Event.objects.create(
            property=property,
            event='click',
            data={
                'user_id': random.choice(user_ids),
                'url': page['path'],
                'title': page['title'],
                'x': random.randint(0, screen['width']),
                'y': random.randint(0, screen['height']),
                'target': random.choice(['a', 'button', 'input', 'link', 'select', 'textarea']),
                'text': random.choice(['', 'Hello World', 'This is a test']),
            },
        )
        Event.objects.filter(id=event.id).update(created_at=timezone.now() - timezone.timedelta(days=random.randint(0, 56)))

    # Generate some page_leave events
    for _ in range(random.randint(300, 600)):
        page = random.choice(url_paths)
        event = Event.objects.create(
            property=property,
            event='page_leave',
            data={
                'user_id': random.choice(user_ids),
                'url': page['path'],
                'title': page['title'],
                'time_on_page': random.randint(0, 100000),
            },
        )
        Event.objects.filter(id=event.id).update(created_at=timezone.now() - timezone.timedelta(days=random.randint(0, 56)))

    # Generate some custom events
    for _ in range(random.randint(300, 600)):
        event = Event.objects.create(
            property=property,
            event=random.choice(custom_events),
            data={
                'user_id': random.choice(user_ids),
            },
        )
        Event.objects.filter(id=event.id).update(created_at=timezone.now() - timezone.timedelta(days=random.randint(0, 56)))


class Migration(migrations.Migration):

    dependencies = [
        ('accounts', '0002_initial_data'),
        ('properties', '0001_initial'),
    ]

    operations = [
        migrations.RunPython(create_proprium),
        migrations.RunPython(create_exemplar),
    ]
