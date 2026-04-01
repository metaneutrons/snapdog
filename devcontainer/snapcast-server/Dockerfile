FROM alpine:3.21 AS builder

ARG SNAPCAST_VERSION=0.32.3

RUN apk add --no-cache build-base cmake git boost-dev alsa-lib-dev flac-dev openssl-dev

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

FROM alpine:3.21
RUN apk add --no-cache dumb-init alsa-lib flac openssl
COPY --from=builder /usr/local/bin/snapserver /usr/local/bin/

RUN adduser -D snapcast && mkdir -p /snapsinks && chown snapcast:snapcast /snapsinks
USER snapcast
EXPOSE 1704 1705 1780
ENTRYPOINT ["dumb-init", "snapserver"]
