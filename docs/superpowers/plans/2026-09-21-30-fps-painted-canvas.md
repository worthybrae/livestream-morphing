# 30 FPS Painted Canvas Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Advance the 1920×1080 Loaded-bristle painting state for every 30000/1001 Abbey Road source frame without increasing Railway CPU, memory, or egress over the restored production baseline.

**Architecture:** Keep full error-guided brush planning at a bounded keyframe cadence, but run motion transport and a persistent native dirty-tile canvas transaction for every decoded frame. Extend the local worker protocol and public segment metrics so fresh paint transactions, planner updates, changed pixels, CPU, RSS, and stage timings are independently measurable; retain the current renderer as the default and overload fallback.

**Tech Stack:** Rust 1.86, Python 3.11, NumPy, OpenCV DIS optical flow, C-compatible Rust cdylib via `ctypes`, FFmpeg/H.264, Railway.

**Spec:** `docs/superpowers/specs/2026-09-21-30-fps-painted-canvas-design.md`

## Global Constraints

- Output and source remain 1920×1080 at 30000/1001 FPS.
- `paint_fps` counts source frames whose painting state was advanced; held output frames do not count.
- `planner_fps` separately counts full brush birth, retirement, and error-guided planning.
- The existing 14,000 base-mark and 20,000 detail-mark bounds remain unchanged.
- The current renderer remains the default unless `LOADED_PAINT_MODE=incremental`.
- No additional Railway service, replica, region, GPU, paid API, or output bitrate increase.
- At most one prepared frame and one ordered canvas commit may be in flight.
- Scalar output is the correctness oracle; architecture-specific code must match it exactly.
- Production acceptance requires 20 ready segments at 29.970 paint FPS, mean segment time at most 4.851 seconds, p95 at most 5.40 seconds, and no resource or egress regression.
- Every production-code change follows red-green-refactor; each task ends with its focused tests and full affected suite green.

---

### Task 1: Reproducible complete-worker benchmark

**Files:**
- Create: `artwork/stroke-memory/bench_paint_pair.py`
- Create: `artwork/stroke-memory/test_bench_paint_pair.py`
- Modify: `artwork/stroke-memory/LIVE.md`

**Interfaces:**
- Consumes: a recorded video, a Git revision or worker directory, and optional native libraries.
- Produces: `summarize(samples: list[float]) -> dict[str, float]` and one JSON result containing wall, CPU, stage, pixel-parity, and brush-state measurements.

- [ ] **Step 1: Write the failing summary test**

```python
import unittest
from bench_paint_pair import summarize

class SummaryTests(unittest.TestCase):
    def test_summary_reports_mean_median_and_linear_p95(self):
        result = summarize([10., 20., 30., 40., 50.])
        self.assertEqual(result, {"mean": 30., "median": 30., "p95": 48.})
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
docker run --rm -v "$PWD/artwork/stroke-memory:/work" -w /work \
  --entrypoint /opt/paint/bin/python livestream-morphing:paint30-baseline \
  -m unittest -v test_bench_paint_pair.py
```

Expected: import failure because `bench_paint_pair.py` does not exist.

- [ ] **Step 3: Implement the benchmark API and paired runner**

Use this exact summary boundary and retain the existing alternating run order:

```python
def summarize(samples):
    values = np.asarray(samples, np.float64)
    return {
        "mean": float(values.mean()),
        "median": float(np.median(values)),
        "p95": float(np.percentile(values, 95)),
    }
```

The runner must instantiate baseline and candidate `LoadedPainting` objects,
alternate candidate-first/baseline-first each frame, exclude warmup frames,
measure `time.perf_counter()` and `time.process_time()`, and assert exact RGB,
base marks, and detail marks after every frame. CLI arguments are:

```text
video --baseline REV --baseline-dir DIR --baseline-native PATH
      --candidate-native PATH --frames N --warmup N --hour FLOAT
```

- [ ] **Step 4: Verify GREEN and benchmark the unchanged pair**

Run the focused test, then run 300 frames/60 warmup frames against the captured
Abbey Road fixture. Expected: test passes; benchmark reports
`max_rgb_difference: 0` and identical brush populations.

- [ ] **Step 5: Document the exact fixture and command**

Record the 18.015-second fixture properties, container image, warmup, measured
frame count, and ARM64 limitation in `LIVE.md`. Do not record the temporary
fixture path as a repository dependency.

- [ ] **Step 6: Commit**

```bash
git add artwork/stroke-memory/bench_paint_pair.py \
  artwork/stroke-memory/test_bench_paint_pair.py artwork/stroke-memory/LIVE.md
git commit -m "test: add paired loaded-paint benchmark"
```

---

### Task 2: Versioned paint telemetry protocol

**Files:**
- Modify: `artwork/stroke-memory/live.py`
- Modify: `artwork/stroke-memory/test_live.py`
- Modify: `livestream-morphing-rs/src/loaded.rs`
- Modify: `livestream-morphing-rs/src/renderer.rs`
- Modify: `livestream-morphing-rs/src/pipeline.rs`
- Modify: `livestream-morphing-rs/src/api.rs`

**Interfaces:**
- Produces Python `PaintTelemetry.pack() -> bytes` with the fixed 48-byte `PNT2` response header.
- Produces Rust `PaintTelemetry::decode([u8; 48]) -> Result<PaintTelemetry, Error>`.
- Extends `Rendered` and `ProcessingStats` with `paint_updates`, `planner_updates`, `mutated_pixels`, `planner_ms`, `transport_ms`, `dirty_ms`, `worker_cpu_ms`, and `worker_rss_bytes`. (`painted_frames` remains the existing test-only RGB fixture buffer.)

- [ ] **Step 1: Write failing Python protocol tests**

```python
def test_paint2_telemetry_is_fixed_width_and_round_trips():
    value = PaintTelemetry(True, 1234, 1.5, 2.5, 3.5, 4.5, 5.5, 4096)
    packet = value.pack()
    assert len(packet) == 48
    assert struct.unpack('<4sB3xQ5f4xQ', packet) == (
        b'PNT2', 1, 1234, 1.5, 2.5, 3.5, 4.5, 5.5, 4096)
```

The fields are `planner`, `mutated_pixels`, `transport_ms`, `planner_ms`,
`dirty_ms`, `worker_cpu_ms`, `full_raster_ms`, and `worker_rss_bytes`.

- [ ] **Step 2: Verify Python RED**

Run `python -m unittest -v test_live.LiveTests.test_paint2_telemetry_is_fixed_width_and_round_trips` in the production image. Expected: `PaintTelemetry` import failure.

- [ ] **Step 3: Add the Python telemetry value and Linux RSS reader**

```python
@dataclass(frozen=True)
class PaintTelemetry:
    planner: bool
    mutated_pixels: int
    transport_ms: float
    planner_ms: float
    dirty_ms: float
    worker_cpu_ms: float
    full_raster_ms: float
    worker_rss_bytes: int

    def pack(self):
        return struct.pack('<4sB3xQ5f4xQ', b'PNT2', int(self.planner),
            self.mutated_pixels, self.transport_ms, self.planner_ms,
            self.dirty_ms, self.worker_cpu_ms, self.full_raster_ms,
            self.worker_rss_bytes)

def current_rss_bytes():
    with open('/proc/self/statm', encoding='ascii') as status:
        resident_pages = int(status.read().split()[1])
    return resident_pages * os.sysconf('SC_PAGE_SIZE')
```

`PAINT1` remains byte-for-byte unchanged. The worker sends `PAINT2` and the
48-byte header only when `LOADED_PAINT_MODE=incremental`.

- [ ] **Step 4: Write failing Rust decode and fake-worker tests**

Add tests that decode the Python fixture above, reject the wrong magic and
non-finite timings, and connect to a fake `PAINT2` worker that sends one header
plus one RGB frame. Assert the Rust result reports one planner frame and 1,234
mutated pixels.

- [ ] **Step 5: Verify Rust RED**

Run `cargo test loaded::tests::paint2 -- --nocapture` in the production builder. Expected: missing `PaintTelemetry` and protocol variant.

- [ ] **Step 6: Implement Rust decoding and aggregation**

Use explicit little-endian slices rather than transmuting packed data:

```rust
#[derive(Clone, Copy, Default)]
struct PaintTelemetry {
    planner: bool, mutated_pixels: u64, transport_ms: f64, planner_ms: f64,
    dirty_ms: f64, worker_cpu_ms: f64, full_raster_ms: f64,
    worker_rss_bytes: u64,
}
```

Store `protocol: Protocol` on `Worker`, accept only `PAINT1` or `PAINT2`, and
read the telemetry header before RGB for `PAINT2`. Sum counters across the
segment, take maximum RSS, and derive:

```rust
paint_fps = paint_updates as f64 / duration_seconds;
planner_fps = planner_updates as f64 / duration_seconds;
mutated_pixel_ratio = mutated_pixels as f64
    / (paint_updates * width as u64 * height as u64).max(1) as f64;
```

- [ ] **Step 7: Expose non-waking public metrics**

Add the new fields to the serialized `ProcessingStats`, `/api/live/status`, and
the existing status JSON without calling `touch()`. Legacy mode reports zero for
the new worker-only fields and retains its existing `paint_fps` calculation.

- [ ] **Step 8: Run focused and complete suites, then commit**

Run the Python suite, `cargo test` for the engine, and native tests through the
Railway Dockerfile. Commit:

```bash
git add artwork/stroke-memory/live.py artwork/stroke-memory/test_live.py \
  livestream-morphing-rs/src/loaded.rs livestream-morphing-rs/src/renderer.rs \
  livestream-morphing-rs/src/pipeline.rs livestream-morphing-rs/src/api.rs
git commit -m "feat: report distinct paint and planner telemetry"
```

---

### Task 3: Exact persistent dirty-tile compositor

**Files:**
- Create: `artwork/stroke-memory/native/src/tiles.rs`
- Modify: `artwork/stroke-memory/native/src/lib.rs`
- Modify: `artwork/stroke-memory/live.py`
- Modify: `artwork/stroke-memory/test_live.py`

**Interfaces:**
- Produces `TilePainter::paint(values: &[f32]) -> Option<TileResult<'_>>`.
- Produces C ABI `paint_frame_incremental(Painter*, marks, count, PaintFrameStats*) -> *const u8`.
- Produces Python `Raster.paint_incremental(marks) -> tuple[np.ndarray, int, float]` returning pixels, changed-pixel count, and full-raster milliseconds.

- [ ] **Step 1: Write failing native parity tests**

Create table-driven sequences for static packets, subpixel translation,
recoloring, addition, removal, clipped brushes, overlapping reversed order, odd
dimensions, and forced full invalidation. For every transaction compare all RGB
bytes from `TilePainter` with a fresh full `Painter` rendering the same complete
ordered packet. Also assert static repeat has zero mutated pixels.

- [ ] **Step 2: Verify native RED**

Run `cargo test --manifest-path artwork/stroke-memory/native/Cargo.toml tiles::tests -- --nocapture`. Expected: module `tiles` missing.

- [ ] **Step 3: Implement tile geometry and conservative invalidation**

Use fixed 64×64 tiles and half-open bounds:

```rust
pub const TILE: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect { pub x0: usize, pub y0: usize, pub x1: usize, pub y1: usize }

pub struct MarkState { pub values: [f32; 12], pub bounds: Rect }
pub struct TileResult<'a> {
    pub rgb: &'a [u8],
    pub mutated_pixels: u64,
    pub dirty_micros: u64,
    pub full_raster_micros: u64,
    pub full_fallback: bool,
}
pub struct TileGrid {
    width: usize, height: usize, columns: usize,
    previous: HashMap<u32, MarkState>,
    members: Vec<Vec<u32>>,
    dirty: Vec<bool>,
}
```

Invalidate the union of old and new bounds for changed marks, old bounds for
removed marks, and new bounds for additions. A duplicate/non-finite ID, invalid
geometry, order change among overlapping marks, dimension change, or ID
compaction marks every tile dirty.

- [ ] **Step 4: Implement ordered region replay**

Extract a `draw_region(canvas, canvas_width, Rect, mark, stamp)` primitive from
the existing band renderer. For each dirty tile: restore `[81,83,74,255]`, then
replay current marks that overlap it in original packet order. Copy only dirty
tile RGB into the persistent packed RGB output and compare old/new bytes while
copying to count mutated pixels. Clean tiles are untouched.

- [ ] **Step 5: Add the opt-in ABI and Python binding**

```rust
#[repr(C)]
pub struct PaintFrameStats {
    pub mutated_pixels: u64,
    pub dirty_micros: u64,
    pub full_raster_micros: u64,
    pub full_fallback: u8,
}
```

`paint_frame` remains unchanged. `paint_frame_incremental` validates all pointers
and counts, updates `PaintFrameStats`, and returns null on invariant failure so
Python can clear state and retry the same packet through `paint_frame`.

- [ ] **Step 6: Add Python fallback tests and implementation**

Write a fake native binding test that returns null once and verify
`Raster.paint_incremental` clears and retries a full frame exactly once. Add the
binding, convert microseconds to milliseconds, and return a copied view only in
tests; production retains the shared native buffer.

- [ ] **Step 7: Run parity, Python, and sanitizer-friendly debug tests**

Run all native and Python tests. Run native tests once with one thread and once
with default threads. Expected: exact RGB equality and no unbounded member-list
growth after 1,000 bounded synthetic transactions.

- [ ] **Step 8: Commit**

```bash
git add artwork/stroke-memory/native/src/tiles.rs \
  artwork/stroke-memory/native/src/lib.rs artwork/stroke-memory/live.py \
  artwork/stroke-memory/test_live.py
git commit -m "perf: rebuild only changed paint tiles"
```

---

### Task 4: Every-frame transport with bounded planner keyframes

**Files:**
- Modify: `artwork/stroke-memory/render.py`
- Modify: `artwork/stroke-memory/adaptive.py`
- Modify: `artwork/stroke-memory/live.py`
- Modify: `artwork/stroke-memory/test_motion.py`
- Modify: `artwork/stroke-memory/test_adaptive.py`
- Modify: `artwork/stroke-memory/test_live.py`

**Interfaces:**
- Produces `Painting.transport_only(forward, backward, previous_gray, current_gray) -> np.ndarray`.
- Produces `AdaptiveBrushwork.transport_only(forward, backward, previous_gray, current_gray) -> np.ndarray`.
- Changes `LoadedPainting.paint(rgb, hour) -> tuple[np.ndarray, PaintTelemetry]` in incremental mode only.

- [ ] **Step 1: Write failing transport-only tests**

For base and adaptive populations, seed three marks, apply a trusted translation,
and assert IDs/order/pigments are retained while coordinates move. Apply an
occluded flow and assert the mark is retained unchanged until the next planner
keyframe. Assert transport-only creates no new marks and does not advance birth
or retirement counters.

- [ ] **Step 2: Verify Python RED**

Run the three focused test modules. Expected: `transport_only` missing.

- [ ] **Step 3: Extract transport-only state changes**

Move coordinate transport shared by `Painting.update` and
`AdaptiveBrushwork.begin` into the new methods. They accept the already computed
480×270 grayscale pair, zero displacements below 0.22 source pixels, move only
trusted marks, and return a boolean row mask identifying changed marks. Existing
planner methods call the helper before pigment refresh, failure aging, retirement,
and birth so legacy output remains exact.

- [ ] **Step 4: Write the failing cadence transaction test**

Feed six translated frames to incremental `LoadedPainting` with
`LOADED_PLANNER_STRIDE=3`. Assert six telemetry records, two planner records,
six native incremental calls, distinct moving-mark coordinates on each frame,
and no held-frame substitution.

- [ ] **Step 5: Implement planner cadence and incremental transactions**

In incremental mode, compute flow every frame. On `updates % planner_stride == 0`
run the unchanged full base/adaptive planner and refinement. Otherwise call both
`transport_only` methods, concatenate ground/base/detail populations in the same
deposition order, and call `Raster.paint_incremental`. Clamp
`LOADED_PLANNER_STRIDE` to 2–6 with default 3. Fill `PaintTelemetry` from measured
transport/planner/native stages and `time.process_time()`.

- [ ] **Step 6: Verify legacy exactness and incremental behavior**

Run the paired benchmark in legacy mode against `HEAD`; expected exact pixels and
brush arrays. Run incremental twice; expected deterministic pixels and brush
arrays, 30000/1001 paint transactions, and planner FPS matching the configured
stride.

- [ ] **Step 7: Commit**

```bash
git add artwork/stroke-memory/render.py artwork/stroke-memory/adaptive.py \
  artwork/stroke-memory/live.py artwork/stroke-memory/test_motion.py \
  artwork/stroke-memory/test_adaptive.py artwork/stroke-memory/test_live.py
git commit -m "feat: advance loaded paint on every source frame"
```

---

### Task 5: Rust 30-FPS scheduling and overload fallback

**Files:**
- Modify: `livestream-morphing-rs/src/loaded.rs`
- Modify: `livestream-morphing-rs/src/renderer.rs`
- Modify: `livestream-morphing-rs/src/pipeline.rs`

**Interfaces:**
- Consumes: `PAINT2` telemetry from Task 2.
- Produces: one request/response paint transaction per decoded frame while incremental mode is healthy.
- Produces: explicit `Incremental`, `LegacyAdaptive`, and `Recovering` scheduling states.

- [ ] **Step 1: Write failing 180-transaction and boundary tests**

Use a fake `PAINT2` worker and a 180-frame segment. Assert 180 requests, 180
distinct returned canvases, `paint_fps == source_fps`, and the expected planner
count from telemetry. Repeat across uneven five-frame/four-frame contiguous
segments and assert transaction numbering has no gap or duplicate.

- [ ] **Step 2: Write failing overload-state tests**

Starting incremental, feed three segment costs above 95% of duration and assert
a transition to `LegacyAdaptive` with stride 2.5. Feed six windows below 75% and
assert `Recovering`, then six more and assert return to incremental stride 1.
One segment above 125% transitions immediately. Reset retains the learned safety
state but drops worker/canvas data.

- [ ] **Step 3: Verify Rust RED**

Run the focused loaded-renderer tests. Expected failures show the current
stride-two-or-greater clamp and held canvases.

- [ ] **Step 4: Implement the scheduler**

Parse `LOADED_PAINT_MODE` once. Incremental mode starts at effective stride 1 and
requests every decoded frame. Legacy mode preserves current `paint_due()` and
adaptation unchanged. The overload state changes only at segment boundaries;
it never drops an already decoded frame. Aggregate actual response counts rather
than deriving paint FPS from configured stride.

- [ ] **Step 5: Verify timing, output, and failure reset**

Run all Rust tests, including stalled worker, invalid handshake/header, source
gap, and dimension change. Expected: worker failure resets state; HLS frame count
and timestamps remain 180/30000/1001 in both modes.

- [ ] **Step 6: Commit**

```bash
git add livestream-morphing-rs/src/loaded.rs \
  livestream-morphing-rs/src/renderer.rs livestream-morphing-rs/src/pipeline.rs
git commit -m "feat: schedule incremental paint at source cadence"
```

---

### Task 6: Bounded source-preparation overlap

**Files:**
- Modify: `artwork/stroke-memory/live.py`
- Modify: `artwork/stroke-memory/test_live.py`
- Modify: `livestream-morphing-rs/src/loaded.rs`

**Interfaces:**
- Produces Python `PreparedFrame` containing RGB, grayscale, forward/backward flow, and measured preparation time.
- Produces Python `PaintWorker` with `Queue(maxsize=1)` between preparation and ordered commit.
- Produces Rust one-request-ahead PAINT2 transport with responses consumed strictly in request order.

- [ ] **Step 1: Write failing bounded-queue and ordering tests**

Feed numbered synthetic frames through a fake slow commit function. Assert the
preparer cannot become more than one frame ahead, responses remain numbered
0–5, shutdown joins both threads, and a preparation/commit exception closes the
socket instead of returning a stale canvas.

- [ ] **Step 2: Write the failing Rust one-request-ahead test**

Use a fake PAINT2 worker that records all received first pixels and delays each
response. Render five frames and assert request 1 arrives before response 0 is
read, output canvases remain 0–4, and the final outstanding response is drained
before encoder finish.

- [ ] **Step 3: Verify RED in both suites**

Run the focused Python and Rust tests. Expected: `PreparedFrame`, `PaintWorker`,
and the one-request-ahead state are absent.

- [ ] **Step 4: Implement Python preparation and ordered commit**

```python
@dataclass(frozen=True)
class PreparedFrame:
    rgb: np.ndarray
    gray: np.ndarray
    forward: np.ndarray
    backward: np.ndarray
    prepare_ms: float

class PaintWorker:
    def __init__(self, painting):
        self.to_prepare = queue.Queue(maxsize=1)
        self.to_commit = queue.Queue(maxsize=1)
        self.painting = painting
```

The reader thread copies at most one incoming RGB frame into `to_prepare`. The
preparer computes grayscale/resize/flow in input order and places one
`PreparedFrame` in `to_commit`. The commit loop alone mutates painting state,
packs telemetry, and writes responses. A sentinel closes both stages and every
thread is joined in `finally`.

- [ ] **Step 5: Implement Rust one-request-ahead scheduling**

For PAINT2 only, submit the current decoded frame before consuming the previous
response. Keep exactly one `Outstanding { output_number }`. After submission,
receive and emit the prior result; at end-of-segment receive and emit the final
result. PAINT1 retains the current request/emit/receive behavior. Do not buffer
more than one unencoded painted frame.

- [ ] **Step 6: Verify overlap and full output invariants**

Run both focused suites and the 180-frame segment test. Expected: 180 ordered
paint transactions and output frames, bounded queues, no timestamp difference,
and no worker/thread left alive after reset.

- [ ] **Step 7: Commit**

```bash
git add artwork/stroke-memory/live.py artwork/stroke-memory/test_live.py \
  livestream-morphing-rs/src/loaded.rs
git commit -m "perf: overlap bounded paint preparation and commit"
```

---

### Task 7: Profile-guided native hot-loop work

**Files:**
- Modify only files identified by the retained complete-worker profile among:
  `artwork/stroke-memory/live.py`, `artwork/stroke-memory/render.py`,
  `artwork/stroke-memory/adaptive.py`, `artwork/stroke-memory/native/src/tiles.rs`,
  `artwork/stroke-memory/native/src/lib.rs`
- Modify corresponding test files before each production edit.
- Modify: `artwork/stroke-memory/LIVE.md`

**Interfaces:**
- Consumes: paired benchmark and per-stage telemetry.
- Produces: mean complete-worker wall time at most 33.24 ms and CPU at most 114 ms/update on the recorded fixture, or a documented rejected candidate with production unchanged.

- [ ] **Step 1: Run alternating paired profiles**

Run baseline/candidate/baseline order for at least 240 measured frames after 60
warmup frames. Record mean/median/p95 wall, process CPU, transport, planner,
dirty-tile, full-raster fallback, refinement, changed-pixel ratio, and RSS.

- [ ] **Step 2: Select one measured bottleneck**

Choose the largest stage contributing at least 10% of complete-worker mean. If
no stage meets that threshold, reject low-level optimization and retain the
current candidate. Do not optimize based on an isolated kernel result.

- [ ] **Step 3: Write and verify one failing parity test**

The test must compare the proposed fast path with the current scalar/reference
path over odd sizes, tails, zero work, saturated channels, clipped geometry, and
the recorded packet that triggered the bottleneck. Run it and verify a mismatch
or missing fast path before implementation.

- [ ] **Step 4: Implement in this order and stop at the first passing gate**

First reuse buffers/data layout; second use portable loop changes; third use
runtime-selected AVX2/NEON intrinsics. Hand-written assembly is attempted only
if compiler inspection shows the retained hot loop is not vectorized and the
intrinsic version still leaves the complete worker above 33.24 ms. Keep scalar
and intrinsic fallbacks callable in tests.

- [ ] **Step 5: Re-run the complete-worker gate**

Retain the change only if exact parity passes and paired complete-worker mean or
p95 improves by at least 3% without CPU or RSS regression. Revert the isolated
candidate otherwise and record the rejection in `LIVE.md`.

- [ ] **Step 6: Repeat Steps 1–5 while the acceptance gate is unmet and a stage remains above 10%**

Each retained change receives its own failing test, green verification, paired
measurement, and commit. Stop when the 33.24 ms/114 ms gates pass or every
remaining measured stage falls below the selection threshold.

- [ ] **Step 7: Commit each retained pass**

For a dirty-tile/native pass:

```bash
git add artwork/stroke-memory/native/src/tiles.rs \
  artwork/stroke-memory/native/src/lib.rs artwork/stroke-memory/LIVE.md
git commit -m "perf: reduce dirty-tile paint cost"
```

For a Python transport/planner pass:

```bash
git add artwork/stroke-memory/live.py artwork/stroke-memory/render.py \
  artwork/stroke-memory/adaptive.py artwork/stroke-memory/test_live.py \
  artwork/stroke-memory/test_motion.py artwork/stroke-memory/test_adaptive.py \
  artwork/stroke-memory/LIVE.md
git commit -m "perf: reduce transport and planner paint cost"
```

Use only the applicable explicit command; never stage an unrelated path.

---

### Task 8: Container, segment, and browser acceptance

**Files:**
- Modify: `artwork/stroke-memory/LIVE.md`
- Modify: `README.md`
- Modify tests only if acceptance reveals a reproducible defect; write the failing regression first.

**Interfaces:**
- Consumes: locally accepted incremental implementation.
- Produces: a tested production image and documented Railway baseline/canary evidence.

- [ ] **Step 1: Run complete local verification**

Build `Dockerfile.railway` with the documented ARM override. Require all engine,
native, and Python tests with no skips. Render the recorded 18.015-second fixture
through legacy and incremental modes and verify both outputs decode at 1920×1080,
30000/1001, with the expected frame counts.

- [ ] **Step 2: Collect an instrumented Railway baseline**

Deploy telemetry with rendering still in legacy mode to the existing service.
Keep a client active for at least 20 distinct ready segments. Save segment wall,
worker CPU, Rust CPU, worker/Rust RSS, bytes, buffer depth, and errors. Confirm
visual playback, then retain the deployment ID as the one-command rollback.

- [ ] **Step 3: Deploy the opt-in canary**

Set only:

```text
LOADED_PAINT_MODE=incremental
LOADED_PLANNER_STRIDE=3
LOADED_PAINT_STRIDE=1
```

Deploy the exact clean worktree with `Dockerfile.railway`; do not alter service,
replica, region, source, output width, or encoder settings.

- [ ] **Step 4: Collect 20-segment production evidence**

Require `paint_fps=29.970`, 180 paint transactions per six-second segment, mean
segment time at most 4.851 seconds, p95 at most 5.40 seconds, buffer readiness,
combined CPU no greater than baseline, peak/mean RSS no greater than baseline,
and generated video no greater than 0.415 GB/hour. Treat missing resource fields
as a failed gate, not zero.

- [ ] **Step 5: Verify matched media and browser playback**

Decode one matched raw/paint pair and assert 1920×1080, 180 frames, and
30000/1001. In the production browser verify the live badge, advancing video,
no media error, Loaded-bristle appearance, London color treatment, and no
stutter during a moving vehicle or pedestrian sequence.

- [ ] **Step 6: Accept or roll back**

If every gate passes, retain the canary and document deployment IDs, observation
window, distributions, resource measurements, matched media, and browser result.
If any gate fails, remove `LOADED_PAINT_MODE`, restore the baseline deployment,
verify live playback, and document the failed measurement without weakening the
gate.

- [ ] **Step 7: Commit acceptance documentation**

```bash
git add artwork/stroke-memory/LIVE.md README.md
git commit -m "docs: record 30 fps paint acceptance"
```
