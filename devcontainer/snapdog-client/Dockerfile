FROM rust:1.94-alpine AS builder
RUN apk add --no-cache musl-dev alsa-lib-dev alsa-lib-static pkgconf
WORKDIR /build
COPY . .
RUN cargo build --release -p snapdog-client

FROM alpine:3.21
RUN apk add --no-cache dumb-init alsa-lib
COPY --from=builder /build/target/release/snapdog-client /usr/local/bin/
ENTRYPOINT ["dumb-init", "snapdog-client"]
CMD ["--url", "tcp://host.docker.internal"]
