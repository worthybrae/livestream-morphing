# 30 FPS Painted Canvas Design

## Goal

Process every 1920×1080 Abbey Road source frame into a fresh painted-canvas
state at the source rate of 30000/1001 FPS, while retaining the existing Loaded
bristle appearance, 30000/1001 H.264 output, bounded live latency, and Railway
cost at or below the restored production baseline.

This is not satisfied by encoding a held painting more often. The renderer must
ingest every decoded source frame, advance painting state for that frame, and
make the resulting canvas available to the wave and encoder.

## Baseline and Required Improvement

The restored Railway build at commit `ea4ac4e` was measured on 2026-09-21:

- 1920×1080 source and output at 30000/1001 FPS.
- 11.988 fresh paint updates per second after adaptive cadence control.
- Eight observed 6.006-second production segments averaged 4.851 seconds and
  ranged from 4.510 to 5.372 seconds.
- Output averaged 692,240 bytes per segment, approximately 0.415 GB/hour before
  multiplying by viewers.
- Production worker logs put the largest recurring stages in base planning and
  native rasterization.

A fresh 18.015-second Abbey Road capture was also replayed twice through the
unchanged worker in an ARM64 production container. Across 240 measured frames,
the two identical workers produced exactly equal pixels and brush populations.
The representative worker measured 83.09 ms mean, 67.92 ms median, 141.74 ms
p95, and 284.96 ms of process CPU per painting update. Its mean stage costs were
6.63 ms flow, 34.22 ms base planning, 7.58 ms waiting for adaptive work,
16.74 ms rasterization, and 17.85 ms refinement. Local ARM timings are a
relative development gate, not a prediction of Railway x86 performance.

Moving from 11.988 to 29.970 paint updates/sec at unchanged compute cost allows
roughly 40% of the current work per update. A production segment therefore needs
about a 60% per-update cost reduction. The existing GPU prototypes are excluded:
their complete-worker measurements were slower than the CPU path.

## Definitions and Metrics

The public metrics distinguish these rates:

- `source_fps`: decoded source frames per source second.
- `paint_fps`: source frames for which painting state was advanced, divided by
  source duration. A held canvas does not increment this counter.
- `planner_fps`: frames that run full brush birth, retirement, and error-guided
  planning, divided by source duration.
- `mutated_pixel_ratio`: pixels whose pre-wave canvas value changed, divided by
  all canvas pixels examined. A genuinely static frame may be processed with a
  zero ratio and still counts as a paint update.

Metrics also expose planner, transport, dirty-tile rebuild, full-raster,
refinement, wave, encode, total wall time, and process CPU time. The live status
must not wake a sleeping pipeline.

## Architecture

### Every-frame paint path

Each decoded frame runs a bounded paint transaction in source order:

1. Compute the existing 480×270 forward/backward optical flow against the prior
   source frame.
2. Transport retained base and detail marks, refresh pigments for changed
   regions, and identify marks that were added, moved, recolored, faded, or
   removed.
3. Convert those mark changes into dirty 64×64 canvas tiles.
4. Rebuild only dirty tiles in the native compositor, replaying every brush that
   overlaps a dirty tile in global deposition order. Clean tiles remain in the
   persistent unwarped canvas.
5. Hand the complete canvas to the existing anchored wave and H.264 encoder.

The tile index is conservative: old and new brush bounds are both invalidated,
and uncertain geometry invalidates the whole canvas. A tile rebuild begins from
the same background state as a full raster and replays overlapping marks in the
same order, so the incremental path can be checked pixel-for-pixel against the
full-raster oracle.

### Planner keyframes

Full base planning, error analysis, brush birth/retirement, and multi-scale
refinement remain more expensive than the every-frame transaction. They run on
bounded keyframes at an initially adaptive 10–12 FPS. Their output is committed
only at a frame boundary and invalidates the affected tiles. Intermediate frames
still transport marks, refresh eligible pigments, mutate the canvas, and count
as paint updates; they do not hold the prior canvas.

Planner cadence is a quality knob, not the public paint cadence. It may increase
only when complete segment measurements retain the required headroom. It may
fall under load without lowering `paint_fps` unless the overload guard must enter
the explicit legacy fallback described below.

### Preparation overlap

Source-only preparation for frame N+1 may overlap the ordered canvas commit for
frame N. This includes grayscale conversion, resize, grading inputs, flow inputs,
and planner-only blur/orientation fields. Anything that reads or mutates the
paint canvas, brush population, identity counter, or deposition order remains
serial. There is at most one prepared frame and one commit in flight; no
unbounded queue is introduced.

### Native optimization boundary

The hot-path native interface owns:

- persistent RGB canvas storage;
- mark bounds and tile membership;
- dirty-tile rebuilds;
- packed RGB difference and pigment-refresh scans;
- per-stage wall and process-CPU counters.

Portable scalar code is the correctness oracle. AVX2 on Railway x86 and NEON on
ARM may be selected at runtime after tests prove scalar parity. Hand-written
assembly is permitted only when a profile identifies a compiler-generated hot
loop, the intrinsic/scalar versions remain available, and a paired complete-
worker benchmark demonstrates additional retained improvement. Architecture-
specific work that improves only a microbenchmark is rejected.

## State, Failure, and Recovery

Painting state remains bounded at 14,000 base marks and 20,000 detail marks.
Brush identifiers retain the existing compaction rules. Backward timestamps,
source gaps over 30 seconds, dimension changes, protocol errors, worker exits,
and idle shutdown clear prepared work, tile indices, and persistent canvas state.

The new path is selected by `LOADED_PAINT_MODE=incremental`; absence of the
variable retains the current implementation. Any invariant violation or native
error resets state and retries through the current full-raster path. Sustained
segment overload switches to legacy adaptive cadence rather than allowing the
HLS buffer to drain. A production rollback requires only removing the variable
and redeploying the last accepted image.

## Quality and Correctness Gates

Tests are written before production changes and must cover:

- exact dirty-tile versus full-raster pixels for static marks, translations,
  recolors, additions, removals, clipped brushes, overlapping deposition order,
  odd dimensions, and full-canvas fallback;
- 180 distinct paint transactions for a 180-frame segment, including uneven
  segment boundaries and contiguous source timestamps;
- static scenes producing no duplicate marks and permitting zero changed pixels;
- motion transport, occlusion rejection, narrow-feature refinement, day/night
  grading, ID compaction, reset, timeout, and worker fallback;
- identical scalar and architecture-specific output;
- bounded tile index, buffers, mark populations, prepared-frame queue, and
  worker lifetime.

The unchanged renderer suites must remain green. Matched legacy/candidate video
uses identical source frames. Any intentional visual difference requires a
recorded side-by-side review and explicit acceptance; performance work defaults
to exact pixels where the architecture permits it.

## Performance and Cost Gates

Development gates on the captured Abbey Road fixture are relative to the paired
unchanged worker on the same machine and run order:

- `paint_fps` is 30000/1001 with one transaction per decoded frame.
- Mean complete-worker wall time is at most 40% of the 83.09 ms baseline
  (33.24 ms), with p95 reported separately.
- Process CPU per source second is no greater than the unchanged 11.988-FPS
  production-equivalent baseline; the initial local ceiling is 114 ms CPU per
  30-FPS update, derived from 40% of 284.96 ms.
- Pixel and brush-state comparisons satisfy the quality gates.

Production is accepted only after a canary on the existing single Railway
service and allocation demonstrates all of the following:

- at least 20 distinct ready-state segments at 29.970 paint FPS;
- mean total segment time no greater than 4.851 seconds and p95 no greater than
  5.40 seconds for 6.006 seconds of source;
- no cadence overruns, source gaps caused by processing, buffer drain, worker
  restart, or browser playback error;
- average vCPU-seconds and memory no greater than the measured restored baseline
  over comparable warm windows;
- generated bytes per source hour no greater than 0.415 GB;
- a matched raw/paint segment decodes as 1920×1080, 180 frames, 30000/1001 FPS,
  and reports 180 paint transactions.

Railway hardware variance is handled by comparing baseline and candidate on the
same allocation when possible and by retaining the overload fallback. HTTP
health alone is not an acceptance signal.

## Delivery Sequence

1. Add metrics and a reproducible paired full-worker/segment benchmark without
   changing rendering.
2. Add the scalar dirty-tile compositor behind the opt-in mode and prove exact
   parity against full rasterization.
3. Add mark-change tracking and the every-frame incremental paint transaction.
4. Overlap source-only preparation with ordered commits.
5. Profile the complete worker, then optimize only measured hot stages using
   buffer reuse, data-layout changes, SIMD, or assembly in that order.
6. Run local quality/performance gates, build the production container, and run
   the full test suites.
7. Deploy an opt-in Railway canary, collect comparable resource and segment
   windows, inspect matched video, and either accept or roll back.

No production variable, service, replica, region, source, resolution, or output
frame rate changes before the local gates pass.
