from django.contrib import admin

from .models import Property, Event


class PropertyAdmin(admin.ModelAdmin):
    list_display = ('id', 'name', 'user', 'total_events', 'total_page_views', 'total_clicks',)
    list_filter = ('user__username',)
    search_fields = ('name', 'user__username',)
    ordering = ('user',)


admin.site.register(Property, PropertyAdmin)


class EventAdmin(admin.ModelAdmin):
    list_display = ('property', 'event', 'created_at',)
    list_filter = ('property__name', 'created_at')


admin.site.register(Event, EventAdmin)
