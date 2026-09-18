# Stingrae Twitch broadcaster

This container copies the existing H.264 art stream from the local processor to Twitch. It adds silent AAC audio without re-encoding the video. The processor stays active while this broadcaster is connected, including when no one is watching the website.

The stream key is a host file at `/home/worthy/services/live-art/twitch.key`, mounted read-only at `/run/secrets/twitch_key`. Never put the key in this repository, the image, or Docker environment variables.

Build on Stingrae:

```sh
docker build -t stingrae-twitch-broadcast:latest /home/worthy/services/live-art/twitch
```

Run continuously:

```sh
docker run -d --name twitch-broadcast --restart unless-stopped \
  --network host --cpus 0.5 --memory 512m \
  --log-opt max-size=10m --log-opt max-file=3 \
  --mount type=bind,src=/home/worthy/services/live-art/twitch.key,dst=/run/secrets/twitch_key,readonly \
  stingrae-twitch-broadcast:latest
```

The broadcaster reconnects after an error and starts a fresh Twitch session after 47 hours, before Twitch's 48-hour limit. `docker logs twitch-broadcast` shows progress without printing the stream key. `docker stop twitch-broadcast` stops the broadcast; `docker start twitch-broadcast` resumes it.

The Twitch bandwidth test uses `-e TWITCH_BANDWIDTH_TEST=1 -e RUN_ONCE=1 -e SESSION_SECONDS=60` with the same image. That mode does not make the channel publicly live.
