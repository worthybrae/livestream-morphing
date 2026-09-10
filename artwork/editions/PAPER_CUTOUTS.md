# Paper / cutouts

A mix of bold flat cutouts and subtle depth, developed from the paper studies.
Edition 32 keeps cobalt, vermilion, rose, and warm cream as solid shapes above a navy ground. Shallow cast shadows and a fine illuminated edge provide depth without broad interior shading. Fixed, restrained grain gives the surface a little paper tooth.

The material uses one registered tonal stack and two translated luminance samples per pixel. It avoids the independent layer sampling used by editions 26/27. The exact shared psychedelic warp remains unchanged, applied once after the material: amplitude 0.012, frequency 8, eight-second cycle, source frame time. No history or trails.

Local study only. Render with render.py and paper-focus.json, selecting paper-cutouts; compare with the retained originals at the same source resolution, frame cadence, and CRF. Local timings do not establish production monthly cost.

## Close variations

- Edition 33, lifted edges: the same palette and tonal shapes with 6×4.5px shadows at 1080p and slightly stronger edge depth.
- Edition 34, terracotta: the original shallow depth in petrol, terracotta, peach and unbleached cream.
- Edition 35, sundial: a 24-second lighting orbit moves shadows and edge illumination around the same cutouts. Lighting direction is computed once per frame; it adds no extra per-pixel samples.

All three reuse the same two-sample construction, source-timed animation, and final signature warp. They do not use persistent trails or change production settings.

## Clean cuts

Edition 36 retains lifted-edge depth and colors but classifies smoothed luminance into discrete paper layers before sampling shadows. The prior fixed luminance transition band produced broad color mixtures wherever source gradients were weak. Discrete masks remove that dependence; translated shadows use bilinear mask samples, and the unchanged final warp provides spatial interpolation. The original lifted-edge film remains for comparison. Check moving edges for any increased stepping or shimmer when evaluating this harder treatment.

## Clean-cut palettes

Editions 37–40 are color-only variations of clean cuts: Acid orchid (violet/citron), Blue hour (indigo/sea glass), Ember (burgundy/apricot), and Rose garden (green/blush). They share edition 36's discrete masks, thresholds, shadow geometry, grain and exact final warp. Palette selection happens once per frame. No time-dependent color cycling is introduced in this comparison.

## Environmental Rose garden

Edition 41 derives five palette colors from London hour, cloud cover and rain. Smooth clock-based anchors keep Rose garden's daytime identity, move into blue-green night and warm evening. These are artistic clock anchors, not seasonal solar calculations. Cloud gently cools/desaturates the palette; rain darkens it slightly. The five colors are calculated once per frame; masks, shadows and the signature warp are unchanged.

`weather.py --edition 41` supplies a time/forecast snapshot with Europe/London DST handling and the existing MET Norway disk cache (at least 15 minutes between network checks, conditional requests, stale-data rejection and fallback). It is a local sampling command, not an enabled production scheduler. Live adoption still needs a shared refresh loop and gradual interpolation between forecast updates; no per-viewer requests are needed.

The day-cycle film is explicitly accelerated (one hour per second). The London snapshot uses current sampled parameters on recorded footage; it is not the weather at recording time. Forecast attribution: MET Norway, CC BY 4.0, adapted into palette signals. https://api.met.no/ and https://creativecommons.org/licenses/by/4.0/

## Painted relief studies

Editions 42 and 43 add directional pigment fields and simulated ridge lighting inside Rose garden's discrete paper masks. Smooth, deterministic anisotropic noise produces elongated strokes; stroke breadth increases toward the foreground, and source luminance supplies restrained tonal variation. These are procedural brush coordinates, not semantic object recognition or motion-tracked strokes. All paint construction precedes the exact signature warp, so texture deforms with the surface. The two editions differ only in texture strength (0.85 vs 1.4). Static Rose garden colors isolate this material experiment from environmental palette changes.

Compare the moving films for material quality, texture shimmer, and compression cost. This is a stylized painted-relief experiment, not a claim of photorealistic oil painting. No production deployment.

## Layered brushwork

Edition 44 replaces the continuous diagonal texture with two coats of individually rasterized paint marks. Seeded positions and irregular footprints overlap; each mark has its own pigment load, internal ridges and lighting. Broad source gradients guide direction where present; quieter areas use varied seeded angles. Every deposit is clipped to its source paper layer. These are source-responsive procedural marks, not tracked physical strokes. All construction still precedes the exact final warp. No accumulated history.

This is a material-quality experiment with additional rasterization cost; measure before considering live adoption.

## Palette knife still studies

Editions 45/46 construct the scene from three scales of overlapping opaque knife deposits. Tapered angular faces, clipped corners, pigment variation and raised lips replace the paper-mask-plus-texture model. Smaller strokes are reserved for areas with source contrast. Both use Rose garden-related pigments and the unchanged final psychedelic warp. 45 has broader marks; 46 is finer. Initial evaluation uses a single frame sampled three seconds into the same reference footage, not a new full video render.

Fine knife (46) selected for a full 719-frame motion study, `garden-fine-knife`, using the unchanged renderer. Local command time 54.6s for 24s footage at 1080p/29.97, CRF23. This establishes a motion and performance baseline; not live-ready on the measured setup.

## Building paint / cached knife

Edition 47 tests the selected Fine knife direction with a precomputed footprint bank (three coats, four shape variants, 24 axial orientations). Coverage and ridge/pigment lighting are reused across frames. Direction interpolates between adjacent stamps; weak gradients blend toward a stable seeded direction instead of abruptly switching at a threshold. Finest detail fades in across a contrast range, and five source samples stabilize stroke color.

The lower coat remains present. Upper deposits are revealed at staggered phases over a nine-second cycle, with a smooth return; this suggests repainting without a camera-frame feedback buffer. Source colors always use the current frame. It is not unbounded paint accumulation or retained video history. Worker order remains irrelevant. The exact final psychedelic warp is unchanged. Compare against edition 46: the finite footprint bank changes individual marks slightly, so the optimization is not pixel-identical.

## Solid knife

Edition 48 restores paint opacity while retaining edition 47's smooth detail/orientation and cached footprints. Adjacent orientations accumulate coverage and lighting in a bounded local scratch buffer, then deposit a single opaque stroke. This removes the transparency introduced by compositing each orientation separately. Upper layers reveal spatially from the middle outward rather than fading the whole face. Ridge lighting is stronger. Original 46 and softer 47 remain as references; final warp is unchanged.

## Appearance-preserving Fine knife optimization

Convex scanline bounds skip pixels outside the knife face, with a two-pixel guard and intersection with the original bounding box. Per-pixel paint calculations and draw order remain unchanged. Full 719-frame output at 1080p/29.97, CRF23 was byte-identical to the original Fine knife MP4. Native wall time improved from 52.96s to 46.60s in local runs (full command 54.6s to 48.2s). Still slower than real time; not a production cost guarantee. A separate pigment-lattice cache experiment preserved bytes but showed no speed improvement and was discarded.


## SIMD Fine knife

The original 45/46 knife renderer now evaluates four pixels at once using `wide` 0.7's native vector operations. Pigment hashes, smooth noise interpolation, coverage, and ridge lighting keep the scalar equations and operation order. Empty batches in the conservative scanline guard exit before texture work. There is no resolution reduction, dropped source frame, cached orientation bank, approximate reciprocal, temporal blend, or change to the final signature warp.

Frame workers receive their next painting job before the previous result is encoded. Output still drains in source order with bounded jobs; stateful effects retain their serial path. The local renderer accepts `--workers 1..6` (default 3) and records that choice in its metrics. The public stream configuration is unchanged.

Validation includes scalar-versus-SIMD byte comparisons over varied pixel colors, stroke angles, widths, and partial batches, plus the existing serial/parallel frame-order tests. Benchmark timings below are local ARM64 Docker measurements, not Railway measurements or a monthly-price estimate.

The optional five/six-worker settings are local benchmarking controls; public ingestion retains its existing four-worker cap and three-worker default. Raising a benchmark worker count is not included in the same-worker optimization savings.


| Local run | Painting workers | Frames | Native render time | Processing fps |
|---|---:|---:|---:|---:|
| Previous scanline optimization | 3 | 719 | 46.60 s | 15.43 |
| SIMD + empty-batch rejection + scheduling | 3 | 719 | 28.64 s | 25.10 |
| Same optimized renderer, more concurrency | 5 | 719 | 22.22 s | 32.36 |
| Longer footage, optimized renderer | 5 | 1,438 | 47.18 s | 30.48 |

At three workers, native wall time fell 38.5%. The five-worker long run processed 47.98 seconds of footage slightly faster than real time, but its margin is only about 1.7%; this is not sufficient evidence for a robust production service. Peak RSS in that run was 498,640 KiB. Five workers use more concurrency and memory; that gain is separate from the optimization itself. All outputs retain 1920×1080, 30000/1001 fps and CRF23.

The final 719-frame five-worker MP4 is byte-identical to the original Fine knife reference. 122 Rust tests pass, including scalar pixel parity and ordered serial/parallel output at three, five, and six workers. Experiments with automatic batching, vectorized byte conversion, and a vectorized underpainting did not improve measured performance and were not retained. No production deployment or change to live worker configuration.

Artifacts: `/Users/worthy/Test/living-ink-paper-filaments-2026-09-10/fine-knife-throughput/` and `fine-knife-throughput-long/`; `comparison.json` records source/binary/video hashes and measurements. Reproduce with `render.py OUTPUT --source SOURCE --catalog artwork/editions/paper-focus.json --slugs garden-fine-knife --crf 23 --workers 5`.
