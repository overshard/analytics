from django.urls import path

from . import views


urlpatterns = [
    path('track/', views.track, name='track'),
]
