FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates coreutils ffmpeg \
    && rm -rf /var/lib/apt/lists/*

COPY broadcast.sh /usr/local/bin/broadcast
RUN chmod 755 /usr/local/bin/broadcast

USER 1000:1000
ENTRYPOINT ["/usr/local/bin/broadcast"]
