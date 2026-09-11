# Persistent brushwork: local feasibility study

Approved direction: individual brushstrokes should retain their identity and
follow source movement. Broad, quiet passages support smaller descriptive marks.
The source remains Abbey Road, and the established wave and London palette stay.

This standalone experiment does not change the live renderer or website. It is
an independent adaptation inspired by Peter Litwinowicz's *Processing Images and
Video for an Impressionist Effect* (SIGGRAPH 1997), especially forward transport
and replenishment of strokes. It is not a reproduction of that paper's complete
algorithm or a physical oil-paint simulation.

Paper: https://doi.org/10.1145/258734.258893

## Implementation

- OpenCV DIS forward/backward motion at 480 × 270, at 14.985 paint updates/sec.
- Round-trip and source-appearance checks reject inconsistent displacement.
  Unchanged areas stay anchored when they offer no reliable motion evidence.
- Persistent records hold identity, position, direction, length, width, pigment,
  opacity, and coat. Motion changes position; it does not randomly rotate marks.
- Three coats use boundary-limited footprints. Fine marks depend on local source
  contrast; there is no semantic person/building detector. Color and structural
  analysis use a 960 × 540 image. The stroke raster is 1920 × 1080.
- New strokes fill space according to a distance map and a bounded birth budget.
  Inconsistent/occluded strokes retire over several paint updates. Broad painted
  ground strokes cover newly exposed areas during replenishment.
- A small Rust program caches the irregular filbert masks, including dry tips
  and bristle relief, and composites their pigment with subpixel positioning.
  Four bands rasterize independently without changing stroke order or pixels.
- The completed canvas is held on alternate output frames. The original
  amplitude .012, frequency 8, eight-second sinusoidal wave is applied once,
  with the same edge taper. OpenCV's interpolation uses quantized weights, so
  this preview is not byte-identical to the production wave sampler.
- This recording uses the live fine-knife color formulas at a fixed London
  hour of 15. No weather service or ongoing environment query is added.

The fixed-position control shares the brush system, appearance checks, and
source frames but disables transport. It helps distinguish changed brush
geometry from the benefit (or harm) of transporting marks.

## Reproduce

Requires Rust, FFmpeg, Python, NumPy and OpenCV. Tested with NumPy 2.0.2 and
OpenCV 5.0.0 in `/tmp/stroke-memory-venv`; see `requirements.txt` for exact pins.

```sh
cargo test --manifest-path artwork/stroke-memory/native/Cargo.toml
cargo build --release --manifest-path artwork/stroke-memory/native/Cargo.toml
python3 -m unittest discover -s artwork/stroke-memory -p 'test_*.py' -v
python3 artwork/stroke-memory/render.py /absolute/street.ts /absolute/moving-brushwork.mp4 --seconds 24.024
python3 artwork/stroke-memory/render.py /absolute/street.ts /absolute/fixed-brushwork.mp4 --seconds 24.024 --mode anchored
python3 artwork/stroke-memory/render.py /absolute/street.ts /absolute/moving-fine-detail.mp4 --seconds 24.024 --detail selective
```

Output siblings include JSON timings, native raster timings and selected
unwarped PNGs. Motion analysis, native raster round trips, total wall time,
population and births/deaths are reported separately. The current implementation
reuses masks but rasterizes the entire painting at each paint update; it does
not yet cache unchanged painted regions. Python analysis, process transfers,
and the independent CPU raster make its timings unsuitable for projecting a
production bill. No deployment, replica, GPU, paid API, or service was added.

## Tests and review criteria

Tests exercise known translation, real DIS estimates, failed round trips,
offscreen marks, a stationary foreground crossing a moving background, and
bounded population on a static image. Native output is identical between serial
compositing and the four-band implementation, including overlaps at band seams.

View the bus departure and the pedestrians after it. Look for dragging of the
lamppost, empty holes, remnants of departed subjects, boiling foliage, loss of
small figures and repeated oval stamps. Technical tests cannot establish that
the result resembles convincing hand-painted oil artwork. This is a study for
visual selection, not a claim that the artistic problem is solved.

## September 11 result

Four contiguous source segments captured at 14:50 UTC produced a 720-frame,
24.024-second comparison. The moving-brushwork prototype took 49.51 seconds
locally, with median bidirectional flow time 7.82 ms per painted update and
native raster round trips 53.18 ms. Its maximum population was 13,912 marks.
The fixed-position control took 50.12 seconds. The existing live renderer's
local Docker replay of the same source took 32.98 seconds internally, 34.8
seconds including the artwork-export command. These use different runtimes
and parallelization; they do not establish a production cost ratio.

The experiment is not ready for a low-cost live deployment. It establishes a
reviewable image/motion direction with reusable mark geometry and a cheap flow
prepass. Population management and appearance checks fixed the obvious holes
and static-foreground dragging found in early renders, but painterly quality,
occlusion transitions, texture repetition and subtle drift still need visual
judgment. Five Python motion/state tests and one native compositing-parity test
pass. The production engine and portfolio files are unchanged.

Preview: http://127.0.0.1:3092/stroke-memory/#moving-brushwork

Local artifacts: `/Users/worthy/Test/living-ink-paper-filaments-2026-09-10/stroke-memory`.

## Selective detail iteration

The approved Moving brushwork remains available as the comparison. The new
`--detail selective` option adds a fourth coat of small finishing marks while
leaving the first three coats' spacing, seeded geometry and analysis unchanged.
It uses the full-resolution source for pigment and fine structure. Local color
contrast and orientation coherence select useful features, so quiet areas do
not receive a blanket detail texture. This is feature-based, not semantic
recognition of people or buildings.

Finishing marks have varied, short footprints, tangent directions and stricter
color-boundary limits. They retain the same cached natural masks, motion
transport, pigment settling and fade behavior. Their occupancy map uses final
image resolution so a small feature can receive a mark without losing nearby
features to motion-grid rounding. The original grid's duplicate-birth guard
is retained. Source color is graded only at sampled pigments for this coat;
grading the entire 1080p analysis image was unnecessarily expensive.

The 24.024-second, 720-frame detail recording took 72.75 seconds locally, with
7.94 ms median flow analysis and 58.21 ms native raster round trips per paint
update. Maximum population was 28,618 marks, versus 13,912 in the original
recording. The higher-resolution analysis still adds substantial cost: this
remains a local quality study, not a production performance improvement.
Both versions retain 14.985 paint updates and 29.97 wave/output frames per
second. The palette and wave settings are unchanged.

Seven Python tests pass. The two new checks verify that narrow light/dark
features receive finishing pigment without altering quiet regions or base
coats, and that those fine marks retain identical identity/geometry/pigment
over repeated static frames. The native raster code is unchanged.

Preview: http://127.0.0.1:3092/stroke-memory/#moving-fine-detail

## Human focus: varying effort by subject

The selective-detail pass treated contrast as importance. That also enriched
foliage, while small people could still disappear into coarse underpaint.
`--detail people` instead allocates the finishing coat to detected people.
The original three coats remain the background; no full-resolution video
patches are composited into the painting.

An optional, pinned 3.8 MB NanoDet model finds people in overlapping crops
approximately twice per second. Anonymous attention boxes follow the existing
motion field between checks. Matched detections smooth box jitter; a missed
detection has a 1.2-second grace period followed by an 0.8-second fade, rather
than instantly changing the painting's level of detail. See `PERSON_MODEL.md`
for the upstream source, Apache-2.0 license, checksum and limitations.

Within those soft attention areas, denser overlapping loaded marks paint the
whole figure, including low-contrast clothing. Full source colors and tighter
boundary limits separate small shapes. The first attempt used narrow marks
and looked scratchy, so the final study uses fuller overlapping deposits.
Source boundaries shorten marks at heads, limbs and clothing changes; the
system does not infer pose, invent facial features, or explicitly segment
those body parts. The same pigment grading and 15/30 paint/wave cadence remain.

```sh
python3 artwork/stroke-memory/fetch_person_model.py
python3 artwork/stroke-memory/render.py /absolute/street.ts /absolute/moving-human-focus.mp4 --seconds 24.024 --detail people
```

The full 720-frame recording rendered in 78.36 seconds locally. The detector
ran 48 times, with a 99.78 ms median for the complete crop batch; the raster
round trip was 59.82 ms median. Peak population was 35,028 brush marks. These
costs are included in the total, not a cached/precomputed-detection estimate.
The previous selective-detail recording took 72.75 seconds. Neither path is
yet real-time or a measured production hosting cost.

Eleven Python tests cover the existing motion behavior plus detector output
ordering across OpenCV versions, empty detections, attention transport,
temporary misses/expiry, stable tracking IDs and concentrating fine pigment
on a low-contrast figure instead of a busy background. Full video decode and
HTTP byte-range playback are also checked. Native raster code is unchanged.

Detection can miss obscured people or mistake other forms for a person. This
is a reviewable subject-aware painting study, not a solved semantic rendering
system. The live engine and portfolio remain unchanged.

Preview: http://127.0.0.1:3092/stroke-memory/#moving-human-focus
