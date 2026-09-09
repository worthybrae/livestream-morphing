# Living Ink

Approved direction: sparse white and soft gray contours on uniform charcoal, with moving edges leaving subdued marks that become negligible after 1–2 seconds. Rust is the sole renderer. Direct Railway HLS remains the delivery path. No new paid infrastructure.

## Cost and release conditions
Target approximately $5/month for the stream, including idle memory; this is a target to verify, not a guaranteed cap. Retain demand activation; reduce idle grace from five minutes to one minute. Decode directly at the selected output dimensions to avoid keeping two full RGB batches. Start production at 640×360 unless representative measurements justify 960×540. Local preview must not activate the production service. Never label processing throughput as source or playback FPS. A monthly estimate must state active hours, viewer-hours, bitrate, idle memory, CPU, and whether the existing plan fee is included.

## Renderer and timing
A stateful LivingInk effect uses luminance, separable smoothing, fine/broad Sobel gradients, directional thinning and soft thresholds. Composite contours onto charcoal without source fills. Maintain a floating point trail buffer; use elapsed source-frame time for exponential decay. Trails are deposited only where contours change, with no additive buildup at rest. Reset on source discontinuity, dimension change, or new session; preserve across contiguous segments. Preserve source cadence and set actual HLS segment duration. Defaults use a 0.4-second trail half-life (about 3% remains after two seconds).

## Local iteration
The same native effect powers an offline Rust render/benchmark command and the live pipeline. Capture a short source segment locally, render old/new comparison clips, record frame p50/p95, encode/decode time and peak RSS at 360p/540p. First validate visual quality and cost, then expose this engine through an isolated local studio preview. Studio parameters are drafts; publishing exports a versioned production preset rather than exposing public mutation endpoints.

## Portfolio
Artwork fills the initial viewport behind existing navigation and scrolls away into existing text. Keep fallback during startup. Public read-only telemetry includes rendered frame count, output dimensions, last-segment frame count, effect timing, processing throughput, buffer count and completion time. Telemetry is labelled engine data because HLS playback lags rendering. Hidden tabs stop demand requests.

## Verification
Synthetic tests cover uniform fields, contour output, decay across different time steps, segment continuity, reset, dimension changes and invalid settings. Replay a real captured source to visually compare defaults and quantify performance. Run existing Rust and portfolio checks. Deploy only after measured cost projection and rendered comparison support the release; retain previous deployments for rollback.
