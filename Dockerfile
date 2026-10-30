FROM rust:1-bookworm AS build
WORKDIR /build
# Compile the dependencies first so edits to src/ and static/ do not rebuild them.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs && cargo build --release --locked && rm -rf src target/release/smart-media-scanner target/release/deps/smart_media_scanner*
COPY src ./src
COPY static ./static
RUN cargo build --release --locked

# Speech engine for automatic subtitle sync, cross-compiled once on the build machine for either
# architecture (portable CPU baseline, no OpenMP) plus the SHA-256-verified multilingual base model.
FROM --platform=$BUILDPLATFORM debian:bookworm-slim AS speech
ARG TARGETARCH
ARG WHISPER_TAG=v1.9.5
ARG MODEL_SHA256=60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl git cmake make g++ g++-aarch64-linux-gnu g++-x86-64-linux-gnu && rm -rf /var/lib/apt/lists/*
RUN git clone --depth 1 --branch "$WHISPER_TAG" https://github.com/ggml-org/whisper.cpp /whisper
WORKDIR /whisper
RUN case "$TARGETARCH" in \
      amd64) T=x86_64-linux-gnu; P=x86_64; X="-DGGML_AVX=OFF -DGGML_AVX2=OFF -DGGML_FMA=OFF -DGGML_F16C=OFF -DGGML_BMI2=OFF -DGGML_AVX512=OFF" ;; \
      arm64) T=aarch64-linux-gnu; P=aarch64; X="" ;; \
      *) echo "unsupported architecture $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DCMAKE_SYSTEM_NAME=Linux -DCMAKE_SYSTEM_PROCESSOR=$P \
      -DCMAKE_C_COMPILER=$T-gcc -DCMAKE_CXX_COMPILER=$T-g++ -DBUILD_SHARED_LIBS=OFF -DGGML_NATIVE=OFF -DGGML_OPENMP=OFF \
      -DWHISPER_BUILD_TESTS=OFF -DWHISPER_BUILD_SERVER=OFF "-DCMAKE_EXE_LINKER_FLAGS=-static-libstdc++ -static-libgcc" $X \
    && cmake --build build -j"$(nproc)" --target whisper-cli
RUN curl -fsSL -o /ggml-base.bin https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin && echo "$MODEL_SHA256  /ggml-base.bin" | sha256sum -c -

FROM debian:bookworm-slim
ARG TARGETARCH
LABEL org.opencontainers.image.title="Platno" \
      org.opencontainers.image.description="Local media library with a web player" \
      org.opencontainers.image.source="https://github.com/WebLJ-si/Platno"
RUN apt-get update && apt-get install -y --no-install-recommends ffmpeg ca-certificates curl && rm -rf /var/lib/apt/lists/* && useradd --uid 10001 --create-home media && mkdir /data && chown media:media /data
COPY --from=speech /whisper/build/bin/whisper-cli /usr/local/lib/platno/whisper-cli
COPY --from=speech /ggml-base.bin /usr/local/share/platno/ggml-base.bin
COPY --from=build /build/target/release/smart-media-scanner /usr/local/bin/smart-media-scanner
ENV SMART_MEDIA_HOST=0.0.0.0 SMART_MEDIA_PORT=8787 SMART_MEDIA_DATA=/data SMART_MEDIA_DISTRIBUTION=docker
USER media
VOLUME ["/data"]
EXPOSE 8787
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s CMD curl -fsS http://127.0.0.1:8787/api/auth/setup-status || exit 1
ENTRYPOINT ["/usr/local/bin/smart-media-scanner"]
