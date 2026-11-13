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
when processing averages above 95% of playback duration across three to five
recent segments at strides up to 3. Slower cadences retain a 90% threshold.
An individual segment exceeding 125% slows painting immediately.
The rolling buffer absorbs isolated smaller spikes without a prolonged FPS drop.
Steps between strides 2 and 3 are 0.5, allowing 15 → 12 → 10 FPS instead of
dropping directly from 15 to 10. Larger strides still change by one.
It recovers one step only after six consecutive rolling cost windows below
78% at strides up to 3, or 75% at slower rates, never exceeding the configured
rate. Using rolling cost for recovery prevents a small isolated spike from
restarting an otherwise sustainable recovery. Each cadence change clears the
cost window so old measurements cannot trigger repeated changes at the new rate.
The approved recorded study is 15 paint updates per second. No resolution, material or wave
amplitude is reduced. Painting the next target overlaps wave/encoding of the
preceding canvas. Reported frame timings include this overlap, not CPU time.

## Validation

- Production deployment `1a639f24-7a98-4564-bb64-8dc834b1f0b0` (2026-09-11,
  commit `18b4d0b`) targets stride 2.5 on the existing eight-vCPU EPYC allocation.
  A 304-second observation captured 51 distinct completed segments: eight at
  11.988 paint updates/sec, averaging 5.581 s of processing per 6.006 s segment
  (range 5.050–5.942 s), and 43 at 9.990, averaging 4.902 s (4.269–5.401 s).
  The guard recovered to 12 and then returned to 10 as night-scene cost grew.
  All 61 status samples were live with the full ten-segment buffer. This is an
  adaptive 12-FPS ceiling, not a claim of sustained 12 or 15 FPS. Logs confirmed
  the full 14,000-base / 20,000-detail populations. Browser playback was checked;
  ffprobe confirmed 1920×1080 H.264 at 30000/1001 with 180 frames in a segment.
  The production build passed 169 engine and five native tests; the unchanged
  Python path passed 19 tests. No service, quota or replica was added; actual
  usage cost was not measured. The new fractional-cadence tests cover held-frame
  order across segment boundaries, bounded fallback, recovery and severe stalls.
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
- 19 Python tests and 169 engine tests pass. Native raster tests cover overlapping
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

## Thirty-FPS optimization baseline (2026-09-21)

`bench_paint_pair.py` now runs two complete painting workers on identical
recorded frames, alternates which worker runs first, and rejects any RGB, ground,
base-mark, or detail-mark difference. Decode, Rust socket, wave, encoding, and
comparison time are excluded. The JSON result reports mean, median, and linear
p95 wall/CPU time plus every worker stage. The recording is an external fixture,
not a repository dependency.

The initial gate used a fresh 18.015-second Abbey Road capture at 1920×1080 and
30000/1001 FPS inside the ARM64 production container. Two unchanged workers ran
300 frames with 60 warmup frames and matched every pixel and brush population.
The candidate-order worker measured 85.464 ms mean, 70.174 ms median, and
148.374 ms p95 wall time per painting update; process CPU averaged 296.705 ms.
Mean stages were 6.702 ms flow, 38.888 ms base planning, 3.864 ms waiting for
adaptive work, 17.110 ms raster, and 18.473 ms refinement. Refinement is bursty:
its median was 0.002 ms and p95 75.560 ms. These figures replace no Railway
measurement and make no x86 throughput claim; they are the paired local baseline
for subsequent changes on the same fixture and host.

Reproduce with a separately captured video mounted into the production image:

```sh
python bench_paint_pair.py abbey.ts --baseline-dir . --frames 300 --warmup 60
```

The first retained incremental pass removed two full-resolution color grades
from transport-only frames and used the already prepared adjacent grayscale
pair for motion confidence. Two identical incremental workers matched every
pixel and brush population over 120 frames (30 warmup, 90 measured). Candidate
wall time fell from the prior 96.049 ms mean / 38.017 ms median to 87.432 ms /
22.714 ms; process CPU fell from 198.592 ms to 182.464 ms per update. The mean
remains above the 33.24 ms gate, so this is retained as a measured intermediate
improvement rather than a throughput claim.

The second retained pass parallelizes phase preparation and dirty-tile row
replay, but uses one row worker on ordinary transport frames and two only on
planner frames. It also routes the planner's RGB maximum-error field and tile
blending through exact runtime-selected NEON/AVX2 kernels, and skips the
analysis resize on transport-only frames. On the same 1920×1080 recording, a
300-frame alternating run with 60 warmup frames compared this candidate with
commit `c1d4be1` and verified 240 measured frames with zero RGB or brush-state
difference. Mean wall time fell from 77.713 to 66.878 ms (13.9%) and p95 from
214.790 to 179.938 ms (16.2%). Mean process CPU fell from 164.912 to 162.416 ms
per update (1.5%); peak paired-process RSS fell from 2,952,511,488 to
2,946,875,392 bytes. Candidate telemetry reported 80 planner updates,
15,039,384 mutated pixels, 8.178 ms mean transport, 21.509 ms planner,
24.615 ms dirty-tile work, and 6.774 ms full-raster work.

This exact-output candidate still misses the local 33.24 ms wall and 114 ms CPU
gates, so it is not a 30-FPS production candidate and must not replace the
current Railway deployment. Rejected measured variants included two and four
workers on every frame (more CPU), 32- and 48-pixel tiles, dense full-frame
fallback, cached tile memberships, cached acceptance coverage, a persistent
Rayon pool, parallel optical flow, and lowering refinement cadence. The cadence
change was specifically rejected after it changed 1.74% of RGB elements on
frame 3. These results make the remaining limit explicit: exact planner and
dirty-canvas semantics need an architectural redesign, not hand-written
assembly in an isolated blend loop.

Reproduce the retained gate inside the production ARM64 container:

```sh
LOADED_PAINT_MODE=incremental LOADED_PLANNER_STRIDE=3 \
python bench_paint_pair.py abbey.ts --baseline-dir BASELINE_SOURCE \
  --baseline-native BASELINE_SO --candidate-native CANDIDATE_SO \
  --frames 300 --warmup 60
```

## Borders look (2026-11-13, not deployed)

`LOADED_LOOK=borders` runs the worker with the look chosen in the brush studies:
tone mixed in six piles, borders round still things in their own colour that
swell and thin, and the paint drifting and bending a little everywhere. Unset,
the worker paints the approved look and none of this code runs; an unknown value
is reported on stderr and ignored. The engine needs no change: the variable is
inherited by the worker it starts.

It is a first port, measured only on an arm64 Mac, and not a production
candidate yet:

- With the variable unset, 90 updates of a 1920×1080 recording matched the
  worker and native library of commit `5953d06` in every pixel and in the base,
  ground and finishing mark populations.
- With it set, the worker laid the same borders as the study on the same
  recording (184 showing on average over 270 updates) and bent the paint on the
  same share of the picture.
- It costs about 1.6 times as much: 85.5 ms mean and 71.7 ms median per update
  against 52.2 and 43.0 for the approved look, over 90 updates of the same
  recording on the same machine (timings there vary by several ms run to run).
  The first port cost 109.3 ms. Three cuts brought it down with every pixel and
  mark of those 90 updates unchanged: the widest edge field is no longer built
  for hands that never read it, the reference is recoloured into piles with a
  16-bit lookup, and a border walker asks its questions of one stacked picture
  instead of several. What is left is the look's own analysis running at every
  paint update (edge fields 8 ms, finding things and walking new borders 14 ms
  spread over four updates, colour lookups for the lines) plus about 13 ms of
  fixed work (bending the picture 3 ms, pile mixing, line stamps).
- A further cut is not pixel-exact and applies to the live look only
  (`pace=4`; the study keeps `pace=1` and its frames are unchanged). The still
  scene is looked at on every fourth paint update instead of every one: edge
  fields are built then, and each line's footing is judged then and the verdict
  kept in between. Whether something is crossing a line is still asked at every
  update, so a border still dissolves under a passing car at once. Finding
  things, outlining them and restating markings take turns, one to a look, each
  at three times as many sites, so no update carries all three. Cost on the
  same 90 updates: 63.1 ms mean and 53.9 ms median against 46.9 and 40.0 for
  the approved look measured alongside, about 1.3 times; 95th percentile over
  270 updates fell from 181 to 107 ms. The price: over 72 seconds of the
  recording played forwards and backwards, borders showing averaged 235 in the
  last 18 seconds against 250 when looking at every update (161 against 174 in
  the first 18), about 6% fewer, because a border lost for good is found again
  a little later. At 1.3 times the cost the cadence guard would settle near
  three quarters of the present paint rate.
- A third round is exact again (150 updates identical to the round before).
  The reference is recoloured into piles by the native library (`paint_piles`;
  an older library without it falls back to NumPy), the finishing pass's error
  is cleared along the lines by drawing them into it instead of through a mask
  picture, and the drift reuses its buffers. Over four alternating runs of 150
  updates the look's mean fell from 64.6 to 61.2 ms, with the approved look at
  47.6 alongside: about 1.29 times. What remains, per update: on every fourth
  update the main thread studies the scene while the finishing thread waits
  (about 26 ms over, 6.5 amortised), and about 5 ms of work after both threads
  join (the drift, the lean of the finishing marks, fresh stamps for lines that
  changed width). Closing the first needs that study moved to a thread of its
  own or into the native library.
- The owner then relaxed the brief: it need not be pixel-exact, and fewer
  borders are acceptable. The live look now sets `pace=8`, looking at the scene
  once or twice a second. With that, a line that something has crossed waits
  for the next look before it comes back, and no line rises while its footing is
  in doubt, so a border cannot fade up over a car that has stopped on it. Over
  72 seconds borders showing averaged 227 in the last 18 against 233 at pace 4
  and 250 at pace 1; at pace 12 they fell to 197, so 8 is where it was left.
  Timed on a busy machine (another program held a core), over seven paired
  runs of 150 updates: pace 8 was cheaper than or equal to pace 4 in every
  pair, median of means 61.3 ms against 70.0, with the approved look at 52.9
  alongside: about 1.16 times against 1.32. On a quiet machine pace 4 had
  measured 1.29 times, so these figures are relative, not absolute.
- A leaner search for new lines followed. A border walker that has stopped is
  no longer carried through the remaining steps (exact, and the largest single
  gain: 16 ms to 5 for one search). At a pace above 1 a site reads each field at
  its nearest pixel, and no walker is sent out from a site that a line of its
  kind already runs through; borders showing rose slightly (231 in the last 18
  of 72 seconds, against 227). Over five alternating runs of 150 updates on a
  quieter machine: 54.8 ms median of means against 57.0 for the commit before
  and 47.9 for the approved look, so about 1.14 times against 1.19.
- Tried and withdrawn in that round: finding things on every other round only.
  It saved under a millisecond and drew two dark strokes along a kerb that no
  earlier clip had, because lines were then laid against where moving things
  had been. Things are found once a round.
- Incremental painting refuses it: lines are not yet carried through
  transport-only frames.
- Its slow motions count paint updates, at the rate `LOADED_PAINT_STRIDE` asks
  for. When the guard slows painting they slow with it.
- The engine's night grade makes this look murky; the study's night clips were
  painted with the day grade. Unresolved.
