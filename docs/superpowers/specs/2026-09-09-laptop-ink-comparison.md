# Laptop Living Ink comparison

The website was verified still serving 640×360 via its non-waking metrics endpoint. Earlier 540p comparisons were never deployed. The user continues to prioritize laptop image quality and the existing approximately $5/month target.

## Visual change

Added optional `stroke_body` (default 0, range 0–1). At 1, smooth gradient shoulders remain visible instead of reducing every contour to a one-pixel ridge. Flat regions still have no source fills. This provides fuller, softer white strokes on charcoal, closer to the supplied reference. The full-body path skips ridge-direction and suppression work. A zero broad-line weight skips its blur and contour passes.

The selected local preset is `2026-09-09-laptop-ink-preset.json`. The native renderer generated `artifacts/living-ink/laptop-720p.mp4` and its PNG still. Source is the same captured 180-frame, 6.006-second Abbey Road clip used in earlier comparisons. This sample uses 1280×720 at 14.985 fps, versus deployed 640×360 at 29.97 fps.

## Isolated local release measurements

| Output | Wall render time | CPU user + system | Bytes per 6.006 s | Peak RSS KiB |
|---|---:|---:|---:|---:|
| Previously measured deployed 360p/30 renderer | 3.009 s | 3.461 s | 263,952 | ~234,000 |
| Thin contours, 1080p/15, smoothing 3 | 8.920 s | 10.596 s | 495,756 | 1,609,596 |
| Full strokes, 720p/15, before optimizations | 4.541 s | 5.249 s | 229,924 | 754,996 |
| Full strokes, 720p/15, selected efficient preset | 3.035 s | 3.929 s | 230,488 | 529,868 |

Selected candidate: 1.273 s decode, 0.935 s effects, 0.823 s encode, effect p50 10.29 ms/p95 11.57 ms. CPU is about 14% above the original measured baseline, stream bytes about 13% below it. Peak memory is still higher despite selecting frames before RGB allocation. Under the existing 50-active-hour, 50–250-viewer-hour model, treating the additional peak memory as resident throughout all active hours gives roughly $0.06–$0.26 additional monthly engine usage before headroom, using the previously verified Railway rates. This remains a budget projection, not identical resource usage or a billing cap. Scene changes and production hardware can change results. The 1080p test is unsuitable: it fails to process the source in real time.

## Verification and scope

101 Rust tests passed. New tests cover fuller strokes without source fills and sampled decoding that preserves selected pixels, source FPS, and initial timestamp. The default 360p output was byte-for-byte identical to the previous optimized fixture (`cmp` succeeded, 263,952 bytes). `git diff --check` passed.

Offline width limit now allows 1920 for reference tests. `RENDER_FRAME_STEP` now selects decoded frames before RGB scaling/allocation. The 720-source-frame safety limit still counts all input frames. Production still uses step 1, its current width, and default stroke_body 0; no live settings or deployments were changed.

Before production rollout, get the user's response to the new visual sample and the 15 fps tradeoff, then implement source-timed output cadence in production (including odd segment frame counts and cross-segment phase), permit 1280 output width, and verify consecutive segments and production performance. The existing offline cadence comparison only claims correct duration for this evenly divisible 180-frame sample.
