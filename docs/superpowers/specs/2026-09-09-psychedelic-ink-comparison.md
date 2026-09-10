# Full-rate psychedelic contour reference

User rejected the 15 fps motion, requested psychedelic distortion, and found the fuller strokes blurry. This supersedes the tentative half-rate direction. Preserve no trails.

Created a local native pipeline mode `psychedelic`: existing distortion effect (amplitude .012, frequency 8, cycle 240 frames) followed by LivingInk. Preset is `2026-09-09-psychedelic-ink-preset.json`: smoothing 1, stroke_body .4, no broad pass, no trails. Coordinates for distortion columns are calculated once per frame rather than once per pixel. Default pipeline and production settings are unchanged.

Artifact: `artifacts/living-ink/psychedelic-720p-30fps.mp4`. Reproduction: `morphing render source.ts out.ts 1280 psychedelic PRESET.json`, with no RENDER_FRAME_STEP override. Actual native captured source: 180 frames, 29.970 fps, 6.006 seconds. Source frame 30 shows headlight glare obscuring the car, which limits recoverable contour detail independently of renderer blur.

Release measurement: 9.773 s total, 2.086 s decode, 5.978 s effects, 1.708 s encode; 12.024 s user+system CPU; 686,200 bytes; peak RSS 771,748 KiB; effect p50 31.981 ms / p95 40.895 ms. This does not keep up with live and is NOT a validated $5 candidate. Do not deploy it as-is or reuse the earlier half-rate cost claim for this version. Further substantial optimization or a budget/quality decision is required.

101 Rust tests passed before the release render; git diff --check passed. This is a visual reference only; no deployment performed.

## $10 budget feasibility follow-up

Rendered the identical psychedelic preset at native 1920×1080, 29.970 fps, 180 frames. Artifact: `artifacts/living-ink/psychedelic-1080p-30fps.mp4`. Total 22.153 s (decode 2.741 s, effects 15.494 s, encode 3.908 s); CPU user+system 26.567 s; bytes 1,106,004; peak RSS 1,645,060 KiB. This is an offline visual reference and cannot run live as currently implemented.

Verified Railway rates at https://docs.railway.com/pricing/plans: CPU $20/vCPU-month, RAM $10/GB-month, egress $0.05/GB, Hobby $5 minimum inclusive of usage. Using 50 active hours, 50–250 viewer-hours, 720 hours/month, 66 MiB idle RAM, peak RSS conservatively resident for all active time, and 20% headroom: current psychedelic 720p/30 projects $6.00–$10.94/month; 1080p/30 $11.54–$19.49/month. Engine only, excludes other services/account usage; local CPU measurements and this single scene are not production billing guarantees. Budget alone does not solve wall-time throughput; both settings need optimization or sufficient parallel execution. No budget limits or production settings changed.
