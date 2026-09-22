# Hetzner and Twitch livestream migration design

## Goal

Run the approved Abbey Road painter continuously at its current resolution,
appearance, and paint cadence for less than $1 per day. Twitch is the public
distribution and discovery surface. The existing website embeds the Twitch
player. No painter behavior changes are part of this migration.

## Success criteria

- The painter broadcasts continuously, including when Twitch has no viewers.
- Painter output remains 1920 x 1080 at 30000/1001 output frames per second.
- The existing Loaded Bristle settings, brush populations, wave, grading, and
  adaptive paint scheduler remain unchanged.
- The Hetzner host sustains at least the cadence achieved by the current Railway
  deployment without draining the ten-segment buffer.
- A user-approved Twitch preview shows no unacceptable visual degradation.
- The recurring infrastructure cost remains below $30 per month before tax.
- A failed migration can return the website to the Railway player without a
  new build.

## Selected architecture

```text
Abbey Road source
        |
        v
Existing painter container on Hetzner
        |
        v
Private local HLS endpoint
        |
        v
FFmpeg relay with restart supervision
        |
        v
Twitch ingest -> Twitch player embedded on worthyrae.com
```

The painter continues producing the same HLS stream it produces today. A
separate FFmpeg relay reads that stream over the private Docker network and
publishes one RTMP or RTMPS stream to Twitch. The relay is the permanent local
viewer, so the existing demand-driven renderer remains active around the clock
without changing its lifecycle code.

The relay stream-copies H.264 when Twitch accepts the existing encoded profile.
If Twitch requires a different keyframe interval or profile, the relay performs
the minimum required video transcode. When the source has no audio, the relay
adds a low-bitrate silent AAC track for broad Twitch player compatibility. The
painting itself is never recomputed or altered by the relay.

## Host selection

The first candidate is a Hetzner CX43 in Nuremberg with eight x86 vCPUs, 16 GB
RAM, and Ubuntu 24.04. It is billed hourly and currently has a monthly cap below
the $30 target. The x86 host is intentional: the rasterizer and grade have AVX2
paths, while the recorded ARM replay did not sustain real time.

The CX43 uses shared CPU capacity. Provisioning it is a benchmark, not an
assumption that its advertised vCPU count equals the Railway EPYC allocation.
If it misses the acceptance gates, it is deleted immediately and OVHcloud
VPS-4 is the fallback candidate. No lower-powered production server is used to
meet the budget by silently reducing visual quality.

## Server layout and security

- Docker Compose owns the painter and relay containers.
- Containers communicate on a private Docker network. Painter ports are not
  exposed publicly during normal operation.
- The Hetzner firewall permits SSH only. SSH password authentication and direct
  root login are disabled after key-based administration is verified.
- The Twitch stream key is stored only in a root-owned environment file with
  mode `0600`. It is not printed in logs, placed in shell history, committed,
  baked into an image, or copied into Railway.
- Docker restart policies and systemd bring both containers back after process
  failure or host reboot.
- Automatic security updates are enabled. Reboots are explicit so an update
  cannot unexpectedly interrupt the artwork.
- Logs are size-bounded. The server stores no long-lived video archive.

## Health and recovery

The painter health check verifies that its local playlist advances and that a
new segment becomes available within a bounded interval. The relay health check
verifies that FFmpeg remains connected and continues consuming the playlist.
Repeated failures restart only the unhealthy component first; Docker restarts
the complete stack after a bounded number of failed recoveries.

Startup ordering prevents the relay from failing permanently while the painter
warms. Every recovery is safe to repeat. The Twitch key remains outside the
container filesystem, so replacing an image does not rotate or expose it.

## Qualification procedure

### 1. Private host benchmark

Provision the CX43 hourly and deploy the exact production image and runtime
variables. Keep all public server ports closed. A local sink consumes the HLS
playlist continuously for at least one hour while the painter reads the live
source. Railway remains active for comparison.

The candidate passes only if:

- there are no renderer or worker restarts;
- every sampled six-second output segment completes within its playback window;
- the ten-segment buffer remains stable rather than trending downward;
- the adaptive scheduler does not fall below the cadence observed on Railway
  over the same scene window;
- memory remains comfortably below the 16 GB host limit; and
- sustained CPU contention does not cause accumulating latency.

A recorded-source replay on both hosts supplements the live run when scene
timing prevents a fair comparison. Decoded frames are sampled to confirm the
painter is unchanged before Twitch recompression.

### 2. Twitch transport test

Connect the relay to the user's existing Twitch channel only after the private
benchmark passes. Confirm ingest stability, keyframe compatibility, resolution,
frame rate, silence behavior, and player latency. Capture a short Twitch-player
sample for the user to compare with the current website stream.

No website or Railway change occurs until the user explicitly approves this
Twitch presentation.

### 3. Website cutover

Replace the current video element with Twitch's supported HTTPS embed. Configure
`parent` for every production hostname that embeds the player, including
`worthyrae.com` and `www.worthyrae.com` when both are served. Preserve the page's
responsive full-screen presentation and provide a clear offline state.

### 4. Observation and rollback window

Keep the Railway livestream deployment available for 24 hours after the website
cutover. Observe Hetzner resource use, segment cadence, relay reconnects, and
Twitch availability. Rollback changes only the website player source and does
not require rebuilding either renderer.

After 24 healthy hours, stop the Railway livestream service to end its compute
and egress charges. Preserve its configuration and deployment history long
enough for a recoverable rollback; do not delete the Railway project or the
separate personal website service.

## Cost model

The target host costs approximately $18.49 per month before IPv4 and tax at the
price observed during design. Twitch carries audience distribution, so viewer
count does not multiply Hetzner egress. The existing lightweight website remains
on Railway. Expected total recurring infrastructure is approximately $21 to $24
per month, subject to tax and future provider pricing.

The server is created with hourly billing for qualification. If it fails, the
test ends after roughly one hour rather than creating a monthly commitment.

## Alternatives considered

### Direct HLS from Hetzner

This preserves the custom player and Hetzner includes enough traffic for many
continuous viewers. It was rejected as the primary path because the user wants
Twitch discovery and publicity. It remains a possible future fallback.

### Dual Twitch and public HLS output

This preserves both players but adds monitoring, public attack surface, and two
distribution paths without serving the immediate goal. It is intentionally not
part of the initial migration.

### AWS

AWS can run the workload, but a suitable continuous x86 instance and outbound
transfer substantially exceed the $1-per-day budget. Its managed ecosystem does
not offset that cost for this single persistent process.

### Cheap ARM hosting

The repository builds on ARM64, but the measured ARM replay was slower than real
time at the approved quality. ARM is not used merely to satisfy the budget.

## Out of scope

- Changing paint style, brush counts, resolution, or cadence.
- Reintroducing motion warping or synthetic in-between frames.
- Archiving Twitch video or operating a second public HLS service.
- Multi-host high availability.
- Deleting Railway resources before the rollback window passes.
