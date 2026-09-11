# Loaded bristle live renderer

`LIVE_STYLE=loaded_bristle` selects the approved moving, adaptive brushwork with
Loaded material. The fine-knife pipeline remains available as
`LIVE_STYLE=fine_knife_london` for rollback.

Rust still decodes HLS segments, manages the bounded output/raw pairing, applies
the edge-anchored psychedelic wave and encodes H.264. A persistent local Python
worker uses the study's OpenCV motion and brush planning. Its native cdylib
raster shares the paint buffer directly with Python. No network or model runs
inside the worker, and no separate service is needed.

The worker survives contiguous segments and short skipped-source gaps. Backward
timestamps, gaps over 30 seconds, errors, size changes and idle shutdown discard
its state. Short gaps use the existing motion-confidence rejection so a slow
startup does not repeatedly destroy and rebuild its brush cache.
Local socket timeouts bound startup and frame
responses; dropping the supervisor kills and reaps the worker. Brush identities
are compacted before their float32 namespaces collide. Base and adaptive mark
populations are bounded at 30,000 and 20,000. The live base limit prevents
background marks accumulating far beyond the approved 24-second study (27,467
base marks born in total, 13,230 remaining at its end). Fine-detail capacity is
unchanged; retired base marks free space for new deposits.

`LOADED_PAINT_STRIDE` sets the fastest permitted paint cadence (2–12; default 3).
Production starts at 5: up to 5.99 paint updates and 29.97 wave/output frames per
second. Live processing may relax the stride up to 12 (2.5 paint updates/sec)
when segment processing exceeds 90% of its playback duration. It recovers one
step only after six segments below 75%, never exceeding the configured rate.
The approved
recorded study is 15 paint updates per second. No resolution, material or wave
amplitude is reduced. Painting the next target overlaps wave/encoding of the
preceding canvas. Reported frame timings include this overlap, not CPU time.

## Validation

- Original vs shared raster, before fused math: frames 0 and 90 matched exactly.
- Fused palette differs from the original by at most one RGB level across tested
  day/night anchors. Native effort selection matches the reference fixture.
- Optimized 15-update source comparison at frame 90: mean RGB difference 0.086
  of 255; 99.0% of channel values unchanged. Small arithmetic differences can
  change a few near-threshold adaptive decisions.
- 19 Python tests and 164 engine tests pass. Native raster tests cover overlapping
  subpixel stamps, parallel boundaries and append equivalence. Engine tests cover
  an uneven segment boundary, skipped-source recovery, a cadence change without
  missing output frames, bounded load adaptation and stalled/invalid startup.
- The x86 renderer batches two RGBA pixels with AVX2, selected at runtime, with
  the scalar path retained for other CPUs. Three native tests include odd row
  lengths, transparent coverage, pigment saturation and rounding equivalence.
  A paired 22,794-mark benchmark on the production CPU with Rust 1.86 reduced
  warm raster median from 91.03 to 63.18 ms (31%), with identical output pixels
  across eight subpixel poses. This measures brush rasterization, not total
  pipeline throughput; the cadence guard still measures complete live segments.
- Local arm64 container, 24.024 s / 1080p: initial integrated 15-update replay took
  50.99 s; native optimizations plus overlapped 10-update replay took 28.93 s.
  These are local replay timings, not production throughput or a billing estimate.

The existing on-demand engine stops after 60 seconds without viewers. Idle reset
now also terminates the paint worker and frees its persistent image/mask buffers.

Build with `Dockerfile.railway`; for arm64 local builds pass `--build-arg RUSTFLAGS=`.
The `morphing render INPUT OUTPUT 1920 loaded` command exercises this same live
path. `LOADED_PYTHON`, `LOADED_WORKER`, and `LOADED_RASTER` override local development
paths; production image paths are fixed under `/opt/paint` and `/opt/paint-worker`.

Linux runtime disables NumPy huge-page advice to avoid long allocation stalls.
Mask preparation is parallel, and gradient fields are only constructed when new
brushwork is proposed. Analysis buffers are reused. Empty brush coverage is
excluded exactly, including its subpixel sampling support; a paired 13,519-mark
local check reduced warm raster median from 50.67 to 31.06 ms with identical RGB.
Only paint-target frames are converted to RGB; every source frame still advances
the output timeline. The wave uses four independent row bands at large sizes.
`LOADED_CV_THREADS` bounds OpenCV workers to 1–4, default 4. The worker logs mean
stage timings and brush counts every 30 paint updates for production diagnosis.
