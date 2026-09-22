FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ffmpeg ca-certificates util-linux \
    && rm -rf /var/lib/apt/lists/*
COPY relay.sh healthcheck.sh /usr/local/bin/
RUN chmod 0555 /usr/local/bin/relay.sh /usr/local/bin/healthcheck.sh
ENTRYPOINT ["/usr/local/bin/relay.sh"]
