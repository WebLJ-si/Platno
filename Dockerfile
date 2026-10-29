FROM rust:1-bookworm AS build
WORKDIR /build
# Compile the dependencies first so edits to src/ and static/ do not rebuild them.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs && cargo build --release --locked && rm -rf src target/release/smart-media-scanner target/release/deps/smart_media_scanner*
COPY src ./src
COPY static ./static
RUN cargo build --release --locked

FROM debian:bookworm-slim
LABEL org.opencontainers.image.title="Platno" \
      org.opencontainers.image.description="Local media library with a web player" \
      org.opencontainers.image.source="https://github.com/WebLJ-si/PlexV2"
RUN apt-get update && apt-get install -y --no-install-recommends ffmpeg ca-certificates curl && rm -rf /var/lib/apt/lists/* && useradd --uid 10001 --create-home media && mkdir /data && chown media:media /data
COPY --from=build /build/target/release/smart-media-scanner /usr/local/bin/smart-media-scanner
ENV SMART_MEDIA_HOST=0.0.0.0 SMART_MEDIA_PORT=8787 SMART_MEDIA_DATA=/data
USER media
VOLUME ["/data"]
EXPOSE 8787
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s CMD curl -fsS http://127.0.0.1:8787/api/auth/setup-status || exit 1
ENTRYPOINT ["/usr/local/bin/smart-media-scanner"]
