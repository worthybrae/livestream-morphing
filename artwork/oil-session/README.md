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
