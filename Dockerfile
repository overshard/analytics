# syntax=docker/dockerfile:1
# ----- builder -----
FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev pkgconfig openssl-dev

COPY --from=oven/bun:alpine /usr/local/bin/bun /usr/local/bin/bun

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY frontend ./frontend

RUN cd frontend && bun install --frozen-lockfile && bun run build && bun run build:maps

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp target/release/analytics /app/analytics

# ----- runtime -----
FROM alpine:3.23

RUN apk add --no-cache \
    font-jetbrains-mono ttf-dejavu ttf-liberation fontconfig

WORKDIR /app

COPY --from=builder /app/analytics ./analytics
COPY --from=builder /app/dist ./dist
COPY --from=builder /app/static_maps ./static_maps
COPY templates ./templates
COPY migrations ./migrations

RUN addgroup -S -g 1000 app && \
    adduser -S -h /app -s /sbin/nologin -u 1000 -G app app && \
    mkdir -p /data && chown -R app:app /app /data
USER app

ENV PORT=8000
ENV ANALYTICS_DATA_DIR=/data
EXPOSE 8000

CMD ["./analytics"]
