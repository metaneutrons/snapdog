# ── WebUI build stage ─────────────────────────────────────────
FROM node:25-slim AS webui-builder

WORKDIR /build/webui
COPY webui/package.json webui/package-lock.json ./
RUN npm ci
COPY webui/ .
RUN npm run build

# ── Rust build stage ──────────────────────────────────────────
FROM rust:1-bookworm AS builder

RUN apt-get update && apt-get install -y cmake pkg-config && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY snapdog/ snapdog/
COPY snapdog-client/ snapdog-client/
COPY --from=webui-builder /build/webui/out webui/out
RUN cargo build --release -p snapdog

# ── Runtime stage ─────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    libasound2 ca-certificates dumb-init \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/snapdog /usr/local/bin/

RUN useradd -r -s /bin/false snapdog && \
    mkdir -p /var/lib/snapdog /snapsinks && \
    chown snapdog:snapdog /var/lib/snapdog /snapsinks

USER snapdog
WORKDIR /var/lib/snapdog

EXPOSE 5555 1704

ENTRYPOINT ["dumb-init", "snapdog"]
CMD ["--config", "/etc/snapdog/snapdog.toml"]
