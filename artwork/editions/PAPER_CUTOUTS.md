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
