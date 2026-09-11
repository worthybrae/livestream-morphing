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
