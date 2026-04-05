# ── WebUI build stage ─────────────────────────────────────────
FROM node:22-slim AS webui-builder

WORKDIR /build/webui
COPY webui/package.json webui/package-lock.json ./
RUN npm ci
COPY webui/ .
RUN npm run build

# ── Rust build stage ──────────────────────────────────────────
FROM rust:1-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY snapdog/ snapdog/
COPY --from=webui-builder /build/webui/out webui/out
RUN cargo build --release

# ── Snapserver build ──────────────────────────────────────────
FROM debian:bookworm-slim AS snapserver-builder

ARG SNAPCAST_VERSION=0.32.3

RUN apt-get update && apt-get install -y \
    build-essential cmake git \
    libboost-dev libasound2-dev libflac-dev libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
RUN git clone --depth 1 --branch v${SNAPCAST_VERSION} https://github.com/badaix/snapcast.git && \
    cmake -B build -S snapcast \
        -DCMAKE_BUILD_TYPE=Release \
        -DBUILD_WITH_FLAC=ON \
        -DBUILD_WITH_VORBIS=OFF \
        -DBUILD_WITH_OPUS=OFF \
        -DBUILD_WITH_AVAHI=OFF \
        -DBUILD_WITH_EXPAT=OFF \
        -DBUILD_TESTS=OFF \
        -DBUILD_CLIENT=OFF && \
    cmake --build build -j$(nproc) && \
    cmake --install build

# ── Runtime stage ─────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    libasound2 libflac12 libssl3 \
    ca-certificates dumb-init \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/snapdog /usr/local/bin/
COPY --from=snapserver-builder /usr/local/bin/snapserver /usr/local/bin/

RUN useradd -r -s /bin/false snapdog && \
    mkdir -p /var/lib/snapdog /snapsinks && \
    chown snapdog:snapdog /var/lib/snapdog /snapsinks

USER snapdog
WORKDIR /var/lib/snapdog

EXPOSE 5555 1704

ENTRYPOINT ["dumb-init", "snapdog"]
CMD ["--config", "/etc/snapdog/snapdog.toml"]
