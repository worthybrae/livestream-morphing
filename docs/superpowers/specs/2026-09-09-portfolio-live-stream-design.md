# Portfolio Live Stream Design

**Date:** 2026-09-09  
**Status:** Approved
**Repositories:** `livestream-morphing`, `/Users/worthy/Test/personal`

## Purpose

Publish the transformed Abbey Road feed on the existing portfolio art detail at
`/art/livestream-art`. Opening that page starts the live system automatically.
The feature is designed for one to five simultaneous viewers and a monthly
infrastructure ceiling of $5.

The experience must remain visually complete while compute wakes. The existing
prerecorded Abbey Road video plays immediately, then crossfades to the live feed
when it is ready. If live playback cannot start or later stalls, the recording
remains available and the page retries without presenting a blank player.

## Scope

This design includes:

- An on-demand production deployment of the Rust video processor on Railway.
- Cloudflare proxying and caching for generated HLS segments.
- A public, read-only live status interface.
- A portfolio player that manages wake-up, fallback video, HLS playback, and
  recovery.
- Production access restrictions for the existing administrative APIs.
- Deployment benchmarking and cost checks.

This design does not include viewer accounts, chat, recording or replay of live
sessions, multiple bitrates, a general-purpose streaming platform, or moving the
portfolio application to another provider.

## Architecture Decision

Use Railway Serverless for the Rust processor and Cloudflare's CDN for HLS
delivery. Do not introduce AWS compute or Cloudflare R2 in the first version.

Railway already supports the container and supplies the required wake/sleep
behavior. The service can stop accruing compute charges while asleep. Cloudflare
prevents multiple nearby viewers from repeatedly fetching the same immutable
segment from Railway. At the expected audience size, publishing every segment to
R2 would add credentials, writes, cleanup, and failure modes without removing the
single origin transfer required for each new segment.

AWS remains a fallback only if a production benchmark shows that Railway cannot
process the source in real time within the cost ceiling. An AWS Fargate design
would require a separate wake coordinator and task lifecycle management and is
not justified before that evidence exists.

## System Boundaries

### Rust stream service

The Rust service owns source ingestion, frame processing, H.264 encoding, the
short in-memory HLS window, activation, idling, and public stream status. It does
not own portfolio presentation state.

Production public routes are limited to:

- `GET /health`: liveness only. It does not activate processing.
- `GET /api/live/status`: activates or refreshes the viewer lease and returns
  the public stream state.
- `GET /api/stream`: activates or refreshes the viewer lease and returns the
  current HLS media playlist.
- `GET /api/segments/{segment_id}`: refreshes the viewer lease and returns an
  immutable MPEG-TS segment.

The public status response has this stable shape:

```json
{
  "state": "starting",
  "buffer_count": 2,
  "ready": false,
  "last_segment_at": 1788969600000
}
```

`state` is one of `starting`, `live`, or `unavailable`. `ready` becomes true
only when at least three playable segments are buffered and the most recent
segment is fresh. Internal timing details, source URLs, effect configuration,
and mutable controls are not exposed by this response.

The current five-minute viewer idle timeout remains the initial default. When
it expires, the service stops fetching the source, clears its in-memory HLS
buffer, and ceases outbound activity. Railway Serverless may then put the
container to sleep. A later status request wakes the container and starts a new
buffer.

### Portfolio application

The portfolio owns the user-visible player state. A new `LiveStreamPlayer`
component is rendered in place of the generic prerecorded `MediaBlock` only for
the `livestream-art` detail. Other portfolio video entries remain unchanged.

The component receives the fallback MP4 URL and the configured live-service
base URL. The URL is supplied through a Vite environment variable so preview
and production deployments can use different stream origins without code
changes.

### Cloudflare

A Cloudflare-proxied custom hostname such as `live.<portfolio-domain>` points to
the Railway service.

- HLS playlists, status, health, and error responses are not cached.
- Successful segment responses use
  `Cache-Control: public, max-age=3600, immutable` and a matching Cloudflare
  cache rule.
- Segment identifiers are unique for their content, so cached bytes cannot be
  confused with a later stream session.
- The allowed browser origin is the exact production portfolio origin, with an
  explicit development origin used only outside production.

Cloudflare is a delivery cache, not durable storage. Losing old live segments is
expected and does not affect the current stream.

## Player State and Data Flow

The player has six explicit states: `fallback`, `waking`, `buffering`, `live`,
`recovering`, and `unavailable`.

1. On mount, the muted, looping, inline fallback MP4 starts immediately.
2. The player requests `/api/live/status`. This wakes a sleeping Railway service
   and starts or refreshes the processor's viewer lease.
3. While status is not ready, the player polls every two seconds. The fallback
   remains fully visible and a restrained `STARTING LIVE FEED` label is shown.
4. When status reports `ready`, the player creates a live video element. Safari
   uses native HLS. Other supported browsers use `hls.js`.
5. The live element loads `/api/stream` while hidden. The fallback remains
   visible until the live element emits `canplay`.
6. On `canplay`, playback begins and the two layers crossfade. A `LIVE` indicator
   appears after the transition completes.
7. Normal HLS playlist and segment requests keep the viewer lease active. Status
   polling slows or stops once playback is healthy.
8. On route change or unmount, timers, listeners, and the HLS instance are
   destroyed. No keepalive request is sent.

Autoplay is muted to comply with browser policies. The first version provides
no audio because the visual transformation is the product and audio would add
source, synchronization, and autoplay concerns.

## Failure and Recovery

The fallback is never removed from the page; it is only visually covered by the
live layer. This makes recovery independent of another network fetch.

- If the service does not become ready within 90 seconds, the player enters
  `unavailable`, continues the recording, displays `LIVE FEED UNAVAILABLE ·
  RETRYING`, and retries status with capped backoff.
- A status request failure is treated as temporary. It does not interrupt the
  fallback video.
- An HLS fatal network or media error destroys the current HLS instance,
  crossfades back to the fallback, and begins a fresh readiness check.
- A live stall detected through media events or lack of playback progress moves
  the player to `recovering`; it returns to `live` only after a new player can
  play.
- A stale buffer is never advertised as ready. The backend compares the most
  recent segment timestamp with a freshness threshold derived from the six-second
  segment cadence.
- Upstream source failures are represented as `unavailable` without leaking the
  upstream URL or internal error text.

All transitions are idempotent so React development remounts and repeated retry
callbacks do not create duplicate players or polling loops.

## Production Safety

The existing effects, pipeline, source, and preset mutation endpoints must not
remain anonymously writable on the public production hostname. Production mode
will either omit those routes from the public router or require an administrative
secret before the Railway deployment is exposed. The preferred first version is
to omit mutable routes in production and keep the studio available locally.

CORS allows only the configured portfolio origin in production. Route parameters
retain the existing traversal protections. Public status responses contain only
the fields needed by the player. The service stores no viewer identity and sets
no cookies.

## Cost Model and Guardrails

As of 2026-09-09, Railway lists a $5 Hobby minimum that includes $5 of monthly
usage. Published usage rates are approximately $20 per vCPU-month, $10 per
GB-month of memory, and $0.05 per GB of service egress. Railway bills active
resources by time used. The $5 allowance is workspace-wide, not reserved for
this service.

The planning estimate is $0.05 to $0.10 per active streaming hour, including
processor compute and one origin transfer of each segment. This estimate implies
roughly 50 to 100 active hours per month within $5, but it is not an acceptance
claim. Actual measurements determine whether Railway is retained.

Cost controls are:

- Railway Serverless enabled for the stream service.
- Processing stops five minutes after the final viewer request.
- Cloudflare caches immutable segments so nearby concurrent viewers do not
  multiply Railway origin delivery.
- A Railway billing alert is configured. An account-wide hard limit is not set
  if it would also stop the existing portfolio workload.
- The first optimization, if needed, is reducing output frame rate or dimensions;
  AWS migration is not the first response to a marginal overage.

Current pricing references:

- <https://docs.railway.com/pricing>
- <https://docs.railway.com/guides/cut-idle-costs-serverless>
- <https://developers.cloudflare.com/r2/pricing/>

## Verification

### Automated tests

Rust tests cover:

- Activation through the public status endpoint.
- Readiness only after three fresh segments.
- Stale and empty buffers reporting not ready.
- Viewer lease refresh through playlist and segment requests.
- Idle transition clearing the buffer and stopping source work.
- Public production routing excluding anonymous mutation endpoints.
- Cache and no-cache headers on each response type.
- Exact-origin production CORS.

Portfolio tests cover:

- Immediate fallback rendering.
- Wake/status polling and readiness transition.
- Native HLS selection when supported.
- `hls.js` selection otherwise.
- Crossfade only after `canplay`.
- Timeout and status-error fallback behavior.
- Fatal HLS error and stall recovery.
- Complete timer, listener, and HLS cleanup on unmount.
- Existing non-livestream media entries remaining unchanged.

### Deployment acceptance checks

The deployed system is accepted when:

1. A sleeping service reaches live playback through the portfolio page while the
   fallback remains visible throughout startup.
2. One and five simultaneous clients play continuously without segment gaps.
3. Repeat segment requests are served from Cloudflare cache as shown by response
   cache headers.
4. CPU, memory, output bitrate, segment size, cold-start time, and projected
   monthly cost are recorded from a representative run.
5. The measured processor keeps pace with the six-second input cadence.
6. Closing all clients leads to processor idle and then Railway sleep.
7. Mutable admin requests to the public hostname are rejected or absent.
8. The measured projection fits the $5 monthly ceiling at the expected usage.

If real-time processing or the cost projection fails, tune output dimensions and
frame rate once and repeat the benchmark. Only if that still fails should the
team design the AWS Fargate fallback.

## Rollout and Rollback

Deploy the stream service before the portfolio change and verify its public
read-only behavior. Then configure the live-service URL in the portfolio preview
deployment and run the player acceptance checks. Production portfolio rollout
follows only after the preview succeeds.

Rollback requires only restoring the original generic prerecorded `MediaBlock`
for `livestream-art` or removing the live-service environment variable. The
portfolio remains useful because the fallback asset is unchanged. The Railway
stream service can then be disabled independently.
