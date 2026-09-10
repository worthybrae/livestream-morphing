# Recorded ink studies

Visitors explore a recorded sample using three linework treatments (delicate, crisp, bold) and three motion treatments (still, flowing, psychedelic). The existing shared live stream remains unchanged.

The canonical starting styles are `studio/src/looks.json`. Each combination merges base → linework params → motion params. All nine clips were rendered locally from `/tmp/living-ink/source.ts` using the native release renderer at commit `924ac55`, width 1920, ink mode, RENDER_WORKERS=3, default encoder threads=2. MP4 files are stream-copy remuxes with faststart. The renderer processes all 180 source frames at 30000/1001 fps; no browser effect implementation or public render endpoint was added.

The portfolio mirrors the choices in `src/components/LiveStream/inkLooks.json`. It serves clips and their exact flat parameter JSON under `public/media/ink-studies/`. Total MP4 size is 4,980,603 bytes; individual clips are 168,465–1,125,179 bytes. No clips load until interaction, and only the selected clip is mounted. Native controls handle playback; hidden/offscreen samples pause. Downloadable settings allow reproducing each study with the native renderer. New public controls use standard radio groups and describe their effects.

The local studio provides the same starting choices, keeps its full parameter sliders, and retains local save/export. Flow amplitude displays three decimal places to match the slider step. Production defaults and pipeline settings are unchanged.

Verification: all nine MP4s probed as 1920×1080, 180 frames, 30000/1001 fps. Native stills reviewed for delicate and bold treatments. Local studio rendered bold psychedelic at 1080p; browser preview loaded successfully. Portfolio 94 tests pass, including deferred video loading, static-only selection, preserving playback position, manual pause, hidden-page pause, and load failure messaging. Portfolio and studio production builds pass.

Published portfolio commit `36ef135`, Railway deployment `d873d520-5652-4488-95fb-5edd092bf821`: SUCCESS. All nine production MP4 URLs returned 200/video-mp4 with expected byte lengths; downloaded preset JSON matched its native parameters. Browser selected Bold + Psychedelic and confirmed 1920×1080, readyState 4, unpaused playback at 4.325 s. The production engine was not redeployed or reconfigured. Local studio source commit `6875dc4`.
