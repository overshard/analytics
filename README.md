# Analytics

A self-hostable analytics service with a straightforward API to collect events
from any source.


## Motivation

I was bored and felt like writing my own analytics service over the weekend.


## Features

- Standard website analytics collection
- Custom metrics collection
- UTM query collection
- Optional anonymized location collection
- Customizable UI
- Date range selection and comparison
- Public URL sharing
- Customizable


## Requirements

You need docker + docker-compose installed for a quick production start or you
can figure out how we install and run things via the `Dockerfile` and set it up
yourself.

If you want to install things without docker then you'll need the following
dependencies:

- python
- uv
- bun
- chromium (used for server-side PDF report generation via a subprocess wrapper)

You can also check the `Dockerfile` for an exact list of dependencies and adjust
package names for your desired platform.

This is a standard Django project. If you know how to run Django, or want to
look up any Django tutorial on how to run Django, you shouldn't have a problem
getting this project running on almost anything.


## Running locally

If you have all of the above dependencies installed you can use my Makefile to
run and install python and node dependencies locally. Running `make` will check
that you have the proper dependencies installed and if not it will try and
install them for you. It will then create you a fresh database and run
everything.


## Checking outdated dependencies

This can be done in both bun and uv with the following two commands:

    uv lock --upgrade --dry-run
    bun outdated

You can then upgrade all dependencies at once with:

    make update

I recommend testing everything after this to make sure it's all working.


## Optimizing images with webp

My development system runs Ubuntu so I installed the official webp utils from
Google with `apt install webp`.

    cwebp -q 90 -m 6 -o output.webp input.png


## Using docker-compose

The easiest way to run this project is to run it using
`docker-compose up --build -d` if you have `docker-compose` and `docker`
installed. This will start the server and have you running at port 8000. The
first time you do this make sure you run migrations with
`docker-compose run web python manage.py migrate`. Make sure you setup the
`.env` file before running, you can copy the sample from
`samplefiles/env.sample` into the root of the project as `.env` and change the
variables.


## Default user

The default user is `admin` with the password `admin`. We also create an example
property so you can see how the analytics look and a property to collect metrics
from ourselves.


## User location data

I'm not interested in someone's personal location but I do like to know where
people are coming from region wise. This helps me know if I need to add
translations to my projects or if I need to add a CDN/caching/server to a new
region. We don't store user IPs so location data isn't retroactive.

The dashboard ships with a `refresh_geoip` management command that downloads
the [DB-IP City Lite](https://db-ip.com/db/download/ip-to-city-lite) database
(CC-BY-4.0, no signup, MaxMind-compatible MMDB format) into `GEOIP_PATH`. It
runs automatically on container start; if the file already exists and is less
than 30 days old it skips the download.

To keep it fresh, add a host cron entry on the server that re-runs it monthly:

```cron
# /etc/crontabs/root  — refresh GeoIP on the 4th of each month
0 3 4 * * docker exec analytics_web python manage.py refresh_geoip --force
```

The 4th gives DB-IP a few days to publish their monthly build (1st of the
month). Failures are non-fatal — the collector silently skips GeoIP
enrichment if the database is missing.

If you'd rather use a different MMDB (MaxMind GeoLite2, IPLocate, etc.) just
drop it at `GEOIP_PATH` (defaults to `/data/db.mmdb` in production) and
disable the cron — any MaxMind-format database works.


## Backups

All data is stored in `/srv/data/analytics/` and your repo is stored in
`/srv/git/analytics.git/`. You can backup both of these folders and you'll have
a 100% backup of everything except changes you may have made to the `Caddyfile`
and the `.env` file which should be easy enough to recreate but you can back
those up too!


## Server guide

This quickstart requires that you have an Alpine Linux server running with a
domain name pointed to it. I'm currently using Linode as my host since they
support Alpine Linux nicely. If you don't want to use Linode or Alpine Linux
you can use these instructions and just change the apk commands at the start to
whatever Linux distro you're using.

**IMPORTANT NOTE**: Change `analytics.bythewood.me` to your domain name where
relevant in these instructions.

**TIP**: During the ufw portion to enable the firewall I recommend only allowing
your IP address or your ISP's IP address range which you can find on whois
lookups at the top. For example, replace `192.230.176.0/20` with your IP or your
ISP's IP range.

    ufw allow from 192.230.176.0/20 proto tcp to any port 22

I allow my local ISP's range because I have a DHCP lease from them and I get
tired of logging into my server from my hosting provider's UI to update it. It's
good enough security and much better than nothing!

Server:

    apk update && apk upgrade && apk add docker docker-compose caddy git iptables ip6tables ufw
    ufw allow 22/tcp && ufw allow 80/tcp && ufw allow 443/tcp && ufw --force enable
    echo -e "#!/bin/sh\napk upgrade --update | sed \"s/^/[\`date\`] /\" >> /var/log/apk-autoupgrade.log" > /etc/periodic/daily/apk-autoupgrade && chmod 700 /etc/periodic/daily/apk-autoupgrade
    rc-update add docker boot && service docker start
    mkdir -p /srv/git/analytics.git && cd /srv/git/analytics.git && git init --bare

Local:

    git clone git@github.com:overshard/analytics.git && cd analytics
    git remote remove origin && git remote add origin root@analytics.bythewood.me:/srv/git/analytics.git
    git push --set-upstream origin master

Server:

    mkdir -p /srv/docker && cd /srv/docker && git clone /srv/git/analytics.git analytics && cd /srv/docker/analytics
    cp samplefiles/Caddyfile.sample /etc/caddy/Caddyfile && sed -i 's/analytics.example.com/analytics.bythewood.me/g' /etc/caddy/Caddyfile
    cp samplefiles/env.sample .env && sed -i 's/analytics.example.com/analytics.bythewood.me/g' .env
    cp samplefiles/post-receive.sample /srv/git/analytics.git/hooks/post-receive
    mkdir -p /srv/data/analytics/db && chown -R 1000:1000 /srv/data/analytics
    docker-compose up --build --detach && docker-compose run web python3 manage.py migrate --noinput && docker-compose run web sqlite3 db.sqlite3 "PRAGMA journal_mode=WAL;" ".exit"
    rc-update add caddy boot && service caddy start


## Scaling

I choose to use an sqlite3 database since that handles all my usecases just
fine. My first recommendation for scaling this project would be to use a
PostgreSQL database. If you want to get fancy then a time-series database like
Timescale would make a lot of sense. The foundation of this project is pure
Django so it shouldn't be hard to swap in a different database.


## Support

I won't be providing any user support for this project. I'm more than happy to
accept good pull requests and fix bugs but I don't have the time to help people
run or use this project. I appologize in advance for this. Maintaining
mutliple OSS projects has taught me that I need to step back from trying to
provide support to avoid burnout.
