# Oil painting iteration session

Started 2026-09-10 22:52:35 UTC. Planned finish 2026-09-11 00:52:35 UTC.

Baseline: engine e4cf90c, Railway deployment d9026a07-2fa9-439e-aada-0c61e7f603c8, edition 65.

The first reconstruction study separates pigment selection from brush texture. A coarse ground is covered by three scales of stable, irregularly ordered marks. Later coats repair color differences. Bilateral pigment loading avoids blending a small colored subject into adjacent pavement. Stroke extent responds to source-color differences, with compact dabs for small forms. It does not use semantic object detection.

The first four-layer attempt restored too much photographic detail and was expensive. Three layers are more painterly. Global bright detail dots have been removed. The final psychedelic remapping remains exactly the same; an explicit packed/scalar test protects its pixels.

Performance changes: conservative scanline clipping, invariant reciprocals outside pixel loops, direct reads for complete SIMD batches, smaller fine brushes, fewer fine extent probes, and removal of an unused dense seed layer. The RGB warp now handles four pixels together with the original operation order.

All local output measurements are machine- and workload-specific. Production segment duration is the acceptance check, not isolated per-frame timings from parallel workers.

## Temporal diagnostic

`temporal_quality.py` removes the known separable warp and compensates source motion with optical flow. It measures changes in areas whose source brightness remains stable. This is a flicker diagnostic, not an aesthetic score or a guarantee of motion quality.

On the same first six seconds of the close Times Square recording, the early three-layer study reduced the mean compensated difference from 5.72 to 4.82 RGB levels, and the fraction of large changes from 9.82% to 5.41%. These are not measurements of the final revision.

The inspiration for coarse-to-fine, error-guided refinement is Aaron Hertzmann's *Painterly Rendering with Curved Brush Strokes of Multiple Sizes* (SIGGRAPH 1998), https://mrl.cs.nyu.edu/publications/painterly98/ . The implementation here is an independent bounded brush renderer, with frame-independent identities and material lighting tailored to the live stream.

## Production experiments

The eight-lane/x86-v3 experiment (1e943ac) passed image tests but regressed production to roughly 15 seconds per six-second segment. It was rolled back with Railway's deployment rollback API, restoring the previous image and variables. Subsequent work uses four lanes and x86-v2 again.

The pigment buffer now uses packed RGB words, with exact-image comparison against a frozen RGB24 reference. Twelve generated bristle deposits have five filtered mip levels. Coarse coats can render at half resolution; the final accents remain at 1920×1080. Accent groups separate light and dark residual pigment and use weighted spatial moments for their position and shape. There is no semantic person detector and no temporal accumulation.

A one-worker, 30-frame comparison on the production host, while the existing stream was running, measured median effects times of 146.4 ms (65), 166.7 ms (66), 175.1 ms (71), and 137.1 ms (72). These are relative diagnostics, not real-time throughput. Edition 72 was promoted with six render workers; initial full production segments took 3.9–4.0 seconds for six seconds of 1080p30 source.

Later candidates 73/74 make the coarse coat more opaque and blend unoriented gradient tensors so both horizontal and vertical architecture can guide the brush. A sparse spatial average stabilizes these directions without adding a full-frame coarse blur. These candidates must still be judged against the live 72 baseline before promotion.

## Motion and form boundaries

The opaque passage studies had more motion-compensated change than the initial grouped-brush version, so they were not promoted. The diagnostic is imperfect: it favors photographic reconstruction and is not a direct score for paint quality.

The later studies separate source feature evidence from painting error, preventing the fine layer from correcting away the artist's own brush texture. Orientation-neutral bristle deposits are centrosymmetric in coverage and antisymmetric in surface derivatives; a regression test verifies that reversing a tangent changes at most one RGB level. Ambiguous direction tensors shorten the brush toward a compact dab.

Edition 80 explores 52px/22px placement grids and a full-resolution finishing grid. A bounded geometric progression measures color support along both axes of each brush. It stops on the first boundary rather than skipping across a narrow subject and resuming in another similarly colored region. A dedicated test preserves a narrow vertical subject while allowing the full brush length on open ground. Nearby source pigments can mix within the loaded bristles; night grading and the signature warp remain unchanged.

The first edition-80 preview exposed rectangular ground tiles in detailed areas. Those were rejected before deployment. The revised ground is a bilinearly mixed pigment wash at half resolution, covered by the actual brush deposits; it has no visible tile boundaries. Packed interpolation has a channel-isolation regression test. The final local renderer suite passes 149 tests.

## Final selection and verification

**Edition 79, quiet loaded paint, is the selected live treatment.** Edition 80 remains an unselected experiment: its wash removed the rectangles, but the overall result lost too much definition. Do not confuse edition 80's two-axis boundary probes or continuous wash with the deployed selection.

Edition 79 uses 29px and 13px coarse placement grids, with 8px full-resolution accent sites. Stable irregular brush deposits carry two nearby source pigments, bristle relief, and varied paint loading. Source-feature evidence selects compact light/dark accents without reconstructing every texture difference. It retains the quieter base of edition 72, the exact 0.012-amplitude, frequency-8, eight-second psychedelic warp, and New York day/night grading. It has no semantic object classifier, temporal paint accumulation, or live weather data connection.

Engine code 7e4c5ad was deployed as `fdca6281-3c0b-4251-8529-ee3bfb411795`, verified SUCCESS, with `LIVE_PAINT_EDITION=79` and `RENDER_WORKERS=6`. No additional service, GPU, or paid API was introduced. The final observation window (00:41:03–00:47:33 UTC) sampled 38 completed segments: median 3.7745 seconds, range 3.283–4.897 seconds, for six seconds of 1920×1080 source at 30 fps. Every observation was ready; buffer depth was at least seven and settled at ten. This demonstrates measured throughput headroom, not a monthly billing guarantee.

A matching raw/paint segment pair captured at 00:44:11 UTC decoded without errors. Both contained 180 frames at 1920×1080 and 30 fps. The source and output use different timestamp origins; the comparison player synchronizes by matching segment identity and within-segment offset. The final mobile page showed the synchronized comparison, New York AM/PM clock, and no engine overlay.

The portfolio deployment `0bd9af23-10ff-417e-b40f-0a4c6c8c28c2` (code 49cb7ff) also succeeded. Its recorded fallback is now an eight-second 1080p30 sample of edition 79, 14,605,508 bytes, and its introduction describes the live oil treatment. The earlier Abbey Road contour desk remains explicitly identified as a recorded study. Verified 149 renderer tests and 109 website tests; the website production build passed. The published media served a valid byte-range response and the refreshed page loaded the new copy.

Local side-by-side studies are at `http://127.0.0.1:3092/oil-final-comparison/#quiet-loaded-paint`: baseline 65, intermediate 72, and selected 79 use the same daytime source. The final source-motion-compensated diagnostic for 79 was 5.683 mean RGB levels and 8.813% large changes, versus baseline 5.72 and 9.82%. That is a modest diagnostic improvement, not evidence of a large flicker reduction. Material-orientation stability is separately regression-tested.

## Superseded by user selection

After reviewing the studies, the user requested Abbey Road with the earlier fine knife design, explicitly not the white-line treatment. Production now uses `LIVE_STYLE=fine_knife` (edition 46), the AbbeyRoadHD1 source, 1920px output, and six frame workers. Deployment `9c42e400-1fd3-4b71-9bf3-c1afb89e99b7` succeeded. The portfolio restores the London clock and the matching garden-fine-knife recorded sample. Edition 79 and the comparison studies remain available locally but are no longer selected live. Verified live Abbey Road imagery and 29.97 fps, with a recent six-second segment taking 4.101 seconds.

The user then clarified that dynamic color must also return. `fine_knife_london` selects the previously approved day/night fine knife edition 53, with the real Europe/London hour updated once per segment. This restores source-derived colors, nighttime desaturation/darkening and highlight compression; it does not enable the 24-second preview cycle or a live weather API. The fixed-palette edition 46 remains available but is no longer the requested live selection. Renderer tests: 149 passed.

## Fine knife brush and repaint refinement

Code af86c5a refines edition 53 only. Wave displacement eases to zero over the outer 3.6% of each axis, avoiding clamped-edge extrusion while preserving the interior wave. The brush mask mixes rounded filbert and flat tips, with lighter bristle relief and variable dry coverage; the original conservative raster bounds remain valid. Reference: Winsor & Newton's filbert head description, https://www.winsornewton.com/products/artists-oil-synthetic-hog-brush-artists-oil-synthetic-hog-brush-filbert .

With LIVE_REPAINT=true and fine_knife_london, workers render unwarped targets. A small ordered stage retains the canvas and refreshes changed regions through staggered soft fronts, before applying the wave. This approximates brief paint passes; it does not simulate a physical brush or preserve an indefinite paint history. Stationary regions stay unchanged. Updates complete in roughly 0.15–0.3 seconds. The 24px-at-1080p refresh regions are deliberately small; check for patch-like motion on new footage before increasing their size or duration.

151 tests pass, including no out-of-bounds edge displacement, repaint completion without residual trails, and serial/parallel equality with repaint enabled. Same-source 239-frame local renders took 14.8 seconds without repaint and 14.0 seconds with repaint; these local ARM timings are not production throughput estimates. The local comparison is http://127.0.0.1:3092/brush-repainting/#repainting .

Square highlight regression: the user identified visible 24px refresh cells. LIVE_REPAINT was temporarily disabled. Code be78fe7 replaces cell-clipped writes with overlapping, softly bounded elliptical filbert masks. Detection cells only trigger marks; a shared coverage buffer combines the marks, and painting never forces a cell rectangle at completion. Added a regression asserting a mark crosses its detection boundary and has partial edge coverage. 152 tests pass. Compared no repaint and rounded repaint on a newly captured night segment with bright lamps; artifacts were absent in the inspected still. Production timing must be checked because overlap adds work.

## Two clocks: painting and motion (September 11)

Edition 53 with `LIVE_REPAINT=true`, `LIVE_CALM=true` and `LIVE_PAINT_STRIDE=2` paints alternate source frames while preserving all output timestamps. A 6.006-second Abbey Road segment produces 90 unwarped targets and 180 encoded frames at 29.97 fps. The ordered canvas refresh and anchored eight-second wave run on every output frame. Paint history continues across contiguous segments, resets on a source gap, and never feeds distorted pixels back into the canvas. The real London hour and the recent increase in nighttime source saturation remain in place.

Calm mode replaces threshold-based brush rotation/detail appearance with continuous responses. A three-output-frame pigment median rejects isolated one-frame spikes; an asymmetric, rate-limited response dampens sustained changes. A pulse held for both output frames at 15 fps can pass the median, so the rate limit also matters. This is brief temporal filtering, not motion-compensated object tracking; some softness around fast movement is an explicit tradeoff. Soft overlapping brush masks still shape the refresh. Q8 pigment and Q15 response arithmetic retain small changes without per-channel floating-point work; four-wide brush coverage matches the scalar sweep exactly in tests.

Replay settings: `artwork/editions/motion-cadence.json`. Local comparison: http://127.0.0.1:3092/motion-cadence/traffic/#paint-15-wave-30 . The six-second, 1080p recorded sample contains a moving car and bright headlights. Before batching, the previous 30-fps behavior took 15.784 seconds locally; the completed two-clock version took 13.425 seconds (14.9% less wall time). Summed frame-worker render time fell from 27.559 to 13.699 seconds (50.3%). Both used six workers, CRF 23, and two encoding threads. These are local ARM measurements, not production throughput or a monthly-cost guarantee.

A separate quiet 18-second night capture had continuous source timestamps across ten observed segments. The motion-compensated flicker diagnostic changed only modestly; the moving-headlight diagnostic did not show a broad improvement from cadence alone. Do not describe these as proof that all flicker is fixed. Tests cover single-frame flash rejection, bounded pigment updates, eventual settling, exact SIMD coverage, odd segment tails, serial/parallel equality, continuing paint state, and distinct wave frames during held paintings. 157 renderer tests pass.

Rollback: set `LIVE_PAINT_STRIDE=1` and `LIVE_CALM=false` to restore the previous cadence and temporal behavior while retaining the optimized compositor. Additional effect stacks automatically retain full painting cadence; held targets are limited to the single fine-knife repaint effect.

The first production trial (5349bd7 / deployment 7283879a-8201-4e34-a323-478ac659af4f) confirmed 14.985 painted targets and 29.970 output frames per second, but some segments took 6.1–6.5 seconds. This exceeded the six-second cadence, so it was not accepted as the final performance result. The serial refresh stage dominated.

The follow-up computes the soft refresh coverage at 480 × 270 for a 1080p frame and reconstructs it bilinearly. Source analysis, brush geometry, pigments, canvas history and the wave still run at 1920 × 1080. Local total time on the headlight clip dropped to 10.528 seconds, versus 15.784 seconds before this work (33.3% less). Compared with full-resolution refresh coverage, decoded output differed by 0.898 RGB levels on average; only 0.00031% of channel values differed by more than 15. That includes H.264 encoding differences. 158 tests pass, including smooth mask interpolation and odd frame dimensions.

The reduced-mask production trial (30ce8f3 / 993bad33-a123-42ac-9e62-7b16fd9b14a4) regressed to 11.7–12.8 seconds per six-second segment. It is rejected. Local speedups did not predict this server result. Added separate repaint and warp timings, exact integer mask reconstruction, and four-component pigment blending with scalar-parity tests.

The next version implements a true held canvas: both target generation and ordered paint updates run at 15 fps, with elapsed paint time doubled; only the anchored wave runs at 30 fps. The previous implementation unnecessarily repainted held targets on intervening frames. A three-target median now runs at painting cadence. The sequential oracle checks that the exact painted canvas holds while only wave phase changes, including contiguous segment boundaries. 160 tests pass. Production acceptance still requires measured segment times below the source duration.

### Accepted live cadence

Engine 83d9c3e / deployment f9401770-cb0b-4ebb-923f-7a6ffdd48107 is verified SUCCESS, with fine_knife_london, repaint and calm enabled, stride 2 and full 1920px output. Eight distinct observed segments took 4.398–4.662 seconds per 6.006 seconds of source (median 4.562 seconds). The slowest observation retained 22.4% of the source-duration budget; the buffer filled to ten. Repainting and distortion each took about 1.5 seconds per segment. These measurements support current throughput, not a promise for all scenes or a monthly bill.

A matched raw/paint pair from the final deployment decoded without errors. Each has 180 frames at 1920 × 1080 and 30000/1001 fps, with 90 painted targets. Browser inspection confirms the playing Abbey Road comparison, London AM/PM clock, nighttime source color, anchored wave and fine-knife surface. The completed canvas is held between paint updates; no warped frame enters its history.

Portfolio 08b77e6 / deployment b2927bd1-a2e4-4d82-becb-075609aaa642 is SUCCESS. Its explanation now distinguishes paint updates from held-frame wave motion. Two targeted component tests and the production build pass. The local moving-headlight comparison opens on the final 15/30 version and stops at the recording's end. Latest local replay: 9.285 seconds total, 2.700 seconds repaint, 2.038 seconds warp; local and server hardware differ.
