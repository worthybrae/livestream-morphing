#!/bin/sh
set -u

stream_url=${STREAM_URL:-http://127.0.0.1:18080/api/stream}
ingest_url=${TWITCH_INGEST_URL:-rtmp://ingest.global-contribute.live-video.net/app}
key_file=/run/secrets/twitch_key
session_seconds=${SESSION_SECONDS:-169200}
test_mode=${TWITCH_BANDWIDTH_TEST:-0}
run_once=${RUN_ONCE:-0}

while :; do
  if [ ! -s "$key_file" ]; then
    printf 'Twitch key is missing; retrying in 30 seconds.\n' >&2
    sleep 30
    continue
  fi

  stream_key=$(cat "$key_file")
  output_url="$ingest_url/$stream_key"
  if [ "$test_mode" = 1 ]; then
    output_url="$output_url?bandwidthtest=true"
  fi

  printf 'Starting %s Twitch broadcast.\n' "$(if [ "$test_mode" = 1 ]; then printf 'private test'; else printf 'public'; fi)"
  timeout --signal=INT --kill-after=15s "$session_seconds" \
    ffmpeg -nostdin -hide_banner -loglevel quiet \
      -stats_period 60 -progress pipe:1 \
      -rw_timeout 15000000 -re -i "$stream_url" \
      -f lavfi -i anullsrc=channel_layout=stereo:sample_rate=48000 \
      -map 0:v:0 -map 1:a:0 \
      -c:v copy -c:a aac -b:a 96k -ar 48000 \
      -rw_timeout 15000000 -flvflags no_duration_filesize -f flv "$output_url"
  status=$?
  unset stream_key output_url
  printf 'Twitch session ended with status %s.\n' "$status"

  if [ "$run_once" = 1 ]; then
    exit "$status"
  fi
  sleep 10
done
