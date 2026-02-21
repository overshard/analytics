FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive \
    LANG=C.UTF-8 \
    PIPENV_VENV_IN_PROJECT=1 \
    PLAYWRIGHT_BROWSERS_PATH=/ms-playwright

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates curl gdal-bin \
    python3 python3-pip \
        # Playwright dependencies
        gstreamer1.0-libav gstreamer1.0-plugins-bad gstreamer1.0-plugins-base \
        gstreamer1.0-plugins-good libasound2t64 libatk-bridge2.0-0t64 \
        libatk1.0-0t64 libatspi2.0-0t64 libatomic1 libavif16 libcairo-gobject2 \
        libcairo2 libcups2t64 libdbus-1-3 libdrm2 libenchant-2-2 libepoxy0 \
        libevent-2.1-7t64 libflite1 libfontconfig1 libfreetype6 \
        libgdk-pixbuf-2.0-0 libgbm1 libgles2 libglib2.0-0t64 \
        libgstreamer-gl1.0-0 libgstreamer-plugins-bad1.0-0 \
        libgstreamer-plugins-base1.0-0 libgstreamer1.0-0 libgtk-3-0t64 \
        libgtk-4-1 libharfbuzz-icu0 libharfbuzz0b libhyphen0 libicu74 \
        libjpeg-turbo8 liblcms2-2 libmanette-0.2-0 libnspr4 libnss3 libopus0 \
        libpango-1.0-0 libpangocairo-1.0-0 libpng16-16t64 libsecret-1-0 libvpx9 \
        libwayland-client0 libwayland-egl1 libwayland-server0 libwebp7 \
        libwebpdemux2 libwoff1 libx11-6 libx11-xcb1 libx264-164 libxcb-shm0 \
        libxcb1 libxcomposite1 libxcursor1 libxdamage1 libxdmcp6 libxext6 \
        libxfixes3 libxinerama1 libxkbcommon0 libxml2 libxrandr2 libxrender1 \
        libxslt1.1 libxss1 libxtst6 libxi6 libxshmfence1 xvfb \
        fonts-freefont-ttf fonts-liberation fonts-noto fonts-noto-color-emoji && \
    curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && \
    apt-get install -y --no-install-recommends nodejs && \
    npm install -g yarn@1.22.22 && \
    pip3 install --no-cache-dir --break-system-packages pipenv && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Pipfile Pipfile.lock package.json yarn.lock /app/
RUN yarn install --frozen-lockfile && \
    pipenv install --deploy && \
    mkdir -p "$PLAYWRIGHT_BROWSERS_PATH" && \
    pipenv run playwright install chromium

COPY . .

ENV PATH="/app/.venv/bin:/app/node_modules/.bin:$PATH" \
    PYTHONPATH="/app/.venv/lib/python3.12/site-packages:$PYTHONPATH"

RUN webpack --config webpack.config.js --mode production && \
    python manage.py collectstatic --noinput

RUN chown -R ubuntu:ubuntu /app && \
    chown -R ubuntu:ubuntu "$PLAYWRIGHT_BROWSERS_PATH"

USER ubuntu
