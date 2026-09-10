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
