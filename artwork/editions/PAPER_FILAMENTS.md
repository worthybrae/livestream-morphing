# Paper and filaments: second studies

This collection develops the two materials selected by the artist. The originals remain beside four new studies. These are local recorded previews; no public stream is changed.

## Filaments

Contour script replaces the original uniform diagonal drift with a smooth orientation field and stronger guidance from image brightness gradients. Strokes vary in length, taper at both ends, and fade into dark areas rather than filling the entire frame evenly. Ivory and blue replace the broader spectrum.

Silk tide uses longer copper and teal strokes with a traveling highlight. The field turns slowly, and the highlight moves along each freshly reconstructed stroke. This is not a particle-history effect: every frame is independent and no trails accumulate.

Both use one antialiased capsule for each small path segment. This replaces the original overlapping disc stamps and removes square roots from the rasterizer's pixel loop. Actual end-to-end timing and bitrate are recorded in the collection's MEASUREMENTS.md; algorithmic savings do not guarantee a particular hosting cost.

## Paper

Deeper stage retains the original cobalt, red, and rose identity, using fewer brightness layers and stronger spatial smoothing to reduce small fragments. Soft cast shadows and small lit edges give the paper more depth. Fractional layer offsets use bilinear sampling, avoiding the original whole-pixel stepping. Spatial paper texture is restrained. Fixed translation coordinates are precomputed per row and column, preserving bilinear sampling while avoiding repeated coordinate clamps and conversions for each pixel.

Nocturne keeps the same construction in petrol blue, muted jade, copper, and warm paper. It is an alternate palette for the same material, not an additional rendering technique.

## Signature and validation

All materials finish with the exact same psychedelic warp: amplitude 0.012, frequency 8, period 8 seconds, applied once using source-frame time. The new studies do not change the original materials' calculations.

The collection uses one 719-frame source recording for every film: 1920 × 1080 at 30000/1001 fps, roughly 24 seconds. The recording is not a seamless loop. Reproduce with the existing render/gallery/verify scripts and `--catalog artwork/editions/paper-filaments.json`. Native determinism and serial/parallel equivalence tests cover the new material IDs.
