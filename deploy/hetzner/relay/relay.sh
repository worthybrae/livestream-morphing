#!/bin/sh
set -eu
set +x

: "${HLS_URL:?HLS_URL is required}"
: "${TWITCH_INGEST_URL:?TWITCH_INGEST_URL is required}"

secret_file=/run/secrets/twitch_stream_key
if [ ! -r "$secret_file" ]; then
    echo "Twitch stream key file is not readable" >&2
    exit 1
fi

stream_key=$(awk '{$1=$1; print}' "$secret_file" | tr -d '\r\n')
if [ -z "$stream_key" ]; then
    echo "Twitch stream key file is empty" >&2
    exit 1
fi

rm -f /tmp/relay-progress

exec setpriv --reuid=65534 --regid=65534 --clear-groups ffmpeg \
    -hide_banner \
    -loglevel quiet \
    -reconnect 1 \
    -reconnect_streamed 1 \
    -reconnect_delay_max 5 \
    -i "$HLS_URL" \
    -f lavfi \
    -i anullsrc=channel_layout=stereo:sample_rate=48000 \
    -map 0:v:0 \
    -map 1:a:0 \
    -c:v copy \
    -c:a aac \
    -b:a 64k \
    -ar 48000 \
    -progress /tmp/relay-progress \
    -f flv \
    "${TWITCH_INGEST_URL}/${stream_key}"
