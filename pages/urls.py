from django.urls import path

from . import views


urlpatterns = [
    path('documentation/', views.documentation, name='documentation'),
    path('changelog/', views.changelog, name='changelog'),
    path('favicon.ico', views.favicon, name='favicon'),
    path('robots.txt', views.robots, name='robots'),
    path('sitemap.xml', views.sitemap, name='sitemap'),
    path('', views.home, name='home'),
]
