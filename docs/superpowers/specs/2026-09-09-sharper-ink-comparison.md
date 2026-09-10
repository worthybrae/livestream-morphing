# Sharper Living Ink at the same budget

User rejected fullscreen 360p pixelation and requested a larger image without increasing cost. Production has not changed pending the frame-rate preference requested in chat.

A cache-friendly vertical blur and bounds-check-free composition preserve the renderer's output. The 640p-width and 960p-width encoded outputs remained byte-for-byte the same sizes as before. A direct blur equivalence test and the full 99-test Rust suite pass.

Identical captured Abbey Road source: 180 frames at 30000/1001 fps, 6.006 seconds. Isolated local Docker release runs, not production billing measurements.

| Candidate | Total render+encode | CPU user+system | Encoded bytes | Peak RSS |
|---|---:|---:|---:|---:|
| Current 360p/30, original implementation | 3.009 s | 3.461 s | 263,952 | ~234 MB |
| Optimized 360p/30 | 2.483 s | 2.963 s | 263,952 | 233,168 KiB |
| Optimized 540p/30 | 4.747 s | 5.529 s | 402,884 | 464,816 KiB |
| Optimized 720p/30 | 7.990 s | 9.193 s | 555,164 | 774,220 KiB |
| Optimized 540p/15, smoothing radius 2 | 2.942 s | 3.412 s | 265,268 | 453,436 KiB |
| Optimized 720p/15, smoothing radius 2 | 4.667 s | 5.394 s | 371,864 | 755,200 KiB |

Recommendation: 960×540 at 14.985 fps, smoothing radius 2. It has 2.25× the pixels with essentially the same measured CPU and outbound bytes as the current 360p/29.97 output. Preview currently decodes all input RGB frames before selecting alternate frames, so temporary memory remains higher (roughly $0.15 extra per month if the additional ~215 MiB remained resident during 50 active hours). Before deployment, discard unselected frames before RGB scaling/allocation to avoid this overhead. Preserve the source cadence in telemetry separately from the output cadence and retain elapsed-time trail decay and exact segment duration. No new infrastructure required.

The preview is at `artifacts/living-ink/living-ink-540p-15fps.mp4` and a still at `artifacts/living-ink/living-ink-540p-15fps.png`. Reproduce offline using `RENDER_FRAME_STEP=2 morphing render source.ts output.ts 960 ink params.json` with `{"smoothing":2}`. `RENDER_FRAME_STEP` only affects offline rendering; it does not change the live engine. These comparisons use an evenly divisible 180-frame captured input. Broader cadence support is not claimed for this exploratory replay option.

The original $5 budget remains a projection conditional on viewing hours and scene bitrate, not a hard billing cap. 720p and 540p/30 do not offer the same cost profile in these measurements.
