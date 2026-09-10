# Full-rate psychedelic contour reference

User rejected the 15 fps motion, requested psychedelic distortion, and found the fuller strokes blurry. This supersedes the tentative half-rate direction. Preserve no trails.

Created a local native pipeline mode `psychedelic`: existing distortion effect (amplitude .012, frequency 8, cycle 240 frames) followed by LivingInk. Preset is `2026-09-09-psychedelic-ink-preset.json`: smoothing 1, stroke_body .4, no broad pass, no trails. Coordinates for distortion columns are calculated once per frame rather than once per pixel. Default pipeline and production settings are unchanged.

Artifact: `artifacts/living-ink/psychedelic-720p-30fps.mp4`. Reproduction: `morphing render source.ts out.ts 1280 psychedelic PRESET.json`, with no RENDER_FRAME_STEP override. Actual native captured source: 180 frames, 29.970 fps, 6.006 seconds. Source frame 30 shows headlight glare obscuring the car, which limits recoverable contour detail independently of renderer blur.

Release measurement: 9.773 s total, 2.086 s decode, 5.978 s effects, 1.708 s encode; 12.024 s user+system CPU; 686,200 bytes; peak RSS 771,748 KiB; effect p50 31.981 ms / p95 40.895 ms. This does not keep up with live and is NOT a validated $5 candidate. Do not deploy it as-is or reuse the earlier half-rate cost claim for this version. Further substantial optimization or a budget/quality decision is required.

101 Rust tests passed before the release render; git diff --check passed. This is a visual reference only; no deployment performed.
