FROM rust:1-bookworm AS builder
RUN apt-get update && apt-get install -y libasound2-dev pkg-config && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY . .
RUN cargo build --release -p snapdog-client

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    libasound2 ca-certificates dumb-init \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/snapdog-client /usr/local/bin/
ENTRYPOINT ["dumb-init", "snapdog-client"]
CMD ["--url", "tcp://host.docker.internal"]
