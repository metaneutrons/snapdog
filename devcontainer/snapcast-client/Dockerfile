FROM alpine:3.21
RUN apk add --no-cache dumb-init snapcast-client
ENTRYPOINT ["dumb-init", "snapclient"]
CMD ["--host", "snapcast-server"]
