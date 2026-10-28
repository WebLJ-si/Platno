# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ffmpeg ca-certificates curl && rm -rf /var/lib/apt/lists/* && useradd --uid 10001 --create-home media && mkdir /data && chown media:media /data
COPY --from=build /build/target/release/smart-media-scanner /usr/local/bin/smart-media-scanner
ENV SMART_MEDIA_HOST=0.0.0.0 SMART_MEDIA_PORT=8787 SMART_MEDIA_DATA=/data
USER media
VOLUME ["/data"]
EXPOSE 8787
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s CMD curl -fsS http://127.0.0.1:8787/api/auth/setup-status || exit 1
ENTRYPOINT ["/usr/local/bin/smart-media-scanner"]
