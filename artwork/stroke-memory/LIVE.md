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
populations are bounded at 14,000 and 20,000. The live base limit prevents
background marks accumulating far beyond the approved 24-second study. A replay
of its 360 paint targets peaked at 13,230 base marks; all mark packets matched
the previous 30,000 limit exactly. Fine-detail capacity is unchanged; retired
base marks free space for new deposits.

`LOADED_PAINT_STRIDE` sets the fastest permitted paint cadence (2–12; default 3).
Fractional values are supported: 2.5 gives 11.99 paint updates and 29.97
wave/output frames per second. The sampling phase continues across segments;
every output frame still receives the wave. Production targets 2.5.
Live processing may relax the stride up to 12 (2.5 paint updates/sec)
when processing averages above 90% of playback duration across three to five
recent segments. An individual segment exceeding 125% slows painting immediately.
The rolling buffer absorbs isolated smaller spikes without a prolonged FPS drop.
Steps between strides 2 and 3 are 0.5, allowing 15 → 12 → 10 FPS instead of
dropping directly from 15 to 10. Larger strides still change by one.
It recovers one step only after six segments below 75%, never exceeding the
configured rate. Each cadence change clears the cost window so old measurements
cannot trigger repeated changes at the new rate.
The approved
recorded study is 15 paint updates per second. No resolution, material or wave
amplitude is reduced. Painting the next target overlaps wave/encoding of the
preceding canvas. Reported frame timings include this overlap, not CPU time.

## Validation

- Faster lookup/placement pass (`9bb761d`): all 360 approved-study base mark
  packets matched the previous planner, as did 100 synthetic updates covering
  full-capacity replenishment. Integer brush identities use a small hash mixer;
  raster bands share one ordered list of resolved brushes. A paired production
  CPU benchmark had zero RGB differences over 15 poses for both 22,794 study
  marks and a 45,000-mark stress case. Median raster times changed from
  28.55 to 27.49 ms and 58.55 to 51.85 ms respectively.
  The 15-FPS production trial could not sustain the required headroom on the
  tested night scene; the guard correctly returned to 10. The intermediate
  12-FPS target retains full brush populations, color grading and resolution.
- Production deployment `4e8706a3-7a4f-4b43-a6b6-a5b176a7f0e9` (2026-09-11,
  commit `db305e3`) held 9.990 paint updates/sec and 29.970 output fps across
  38 observed completed segments sampled over 238 seconds. Processing averaged
  4.312 s per 6.006 s segment (range 3.777–4.740 s). Every sample after startup
  was live, the buffer filled to 10, and logs confirmed the full 14,000-base /
  20,000-detail budget. Browser playback and the Loaded material were checked;
  ffprobe confirmed 1920×1080 H.264, 180 frames per sampled output segment and
  30000/1001 fps. The 166 engine, five native and 19 Python checks passed.
  These are measured results on the existing eight-vCPU EPYC allocation, not
  a guaranteed fixed cadence or a billing estimate. Separate Icelake allocations
  were substantially slower even in paired old/new renderer benchmarks; compare
  host performance before attributing a rollout slowdown to the code. The load
  guard remains enabled for busy scenes and slower allocations.
- Original vs shared raster, before fused math: frames 0 and 90 matched exactly.
- Fused palette differs from the original by at most one RGB level across tested
  day/night anchors. Native effort selection matches the reference fixture.
- Optimized 15-update source comparison at frame 90: mean RGB difference 0.086
  of 255; 99.0% of channel values unchanged. Small arithmetic differences can
  change a few near-threshold adaptive decisions.
- 19 Python tests and 166 engine tests pass. Native raster tests cover overlapping
  subpixel stamps, parallel boundaries and append equivalence. Engine tests cover
  an uneven segment boundary, skipped-source recovery, a cadence change without
  missing output frames, bounded load adaptation and stalled/invalid startup.
  Cadence regressions cover isolated spikes, sustained pressure, severe stalls,
  slow recovery and clearing the cost history while retaining the learned rate
  through a reset. Tests run in the Debian builder with production FFmpeg;
  the host's newer FFmpeg headers are incompatible with the pinned Rust wrapper.
- The x86 renderer batches two RGBA pixels with AVX2, selected at runtime, with
  the scalar path retained for other CPUs. Five native tests include odd row
  lengths, transparent coverage, pigment saturation and rounding equivalence.
  A paired 22,794-mark benchmark on the production CPU with Rust 1.86 reduced
  warm raster median from 91.03 to 63.18 ms (31%), with identical output pixels
  across eight subpixel poses. This measures brush rasterization, not total
  pipeline throughput; the cadence guard still measures complete live segments.
- New texture generation shares individual strokes between workers. A
  count-based split concentrated large strokes on one worker because input is
  ordered by coat. Pixel order is unchanged; a test compares complete mixed-size
  paintings and subsequent appends against serial brush deposition.
  A production-CPU benchmark replacing 5% of identities per frame reduced median
  raster time from 85.48 to 47.05 ms for 22,794 recorded-study marks, and from
  110.99 to 89.74 ms for a synthetic 45,000-mark stress case. Cold construction
  fell from 1,066.98 to 376.66 ms in the recorded-study case. All output pixels
  matched across seven poses per case. This measures rendering, not a guarantee
  of a fixed live frame rate.
- Texture generation, subpixel preparation and rasterization now use eight
  workers within the existing eight-vCPU allocation. Compared with four workers
  on the same production host, 5%-identity-churn raster medians fell from 47.49
  to 28.99 ms with 22,794 study marks, and from 86.15 to 56.46 ms with 45,000
  marks. All output pixels matched across seven poses per case. This is a
  wall-time measurement, not a billing estimate; no service or quota was added.
- The day/night grade processes four RGB pixels together on AVX2 hosts, with
  scalar fallback and bounded tail loads. On the production CPU, full-resolution
  grade medians fell from 11.44–12.47 to 3.31–3.92 ms across six palette times.
  Paired output pixels matched exactly, as did scalar/vector tests covering
  RGB tails, sunrise/day/night colors and highlight compression.
- Production deployment `06708d79-9966-4f04-9234-c5c20dfec91b` (2026-09-11,
  commit `7c401a1`) sustained 5.994 paint updates/sec and 29.970 output fps at
  1920×1080 across 20 completed segments sampled over 122 seconds. Processing
  averaged 3.434 s per 6.006 s segment (range 3.076–3.904 s); every status sample
  was live and the buffer filled to 10. Browser playback, material, London clock
  and raw/painted comparison were verified. These are observed results for that
  scene and host, not a guaranteed fixed cadence; adaptation remains enabled.
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
