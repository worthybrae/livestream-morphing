# Living Ink: reconstructed surfaces

This round changes the scene's structure, following feedback that the palette studies were too restrained. Four materials reconstruct the source directly from its spatially smoothed brightness. They bypass the contour detector entirely. The identical final psychedelic distortion is applied once: amplitude 0.012, frequency 8, period 8 seconds, using source-frame time. No material accumulates trails.

- **Living current** traces short luminous filaments along a field derived from brightness gradients. Inertia makes each stroke curve smoothly; a slow change in the field turns the strokes. Every frame is reconstructed independently, without accumulated particle trails.
- **Cathedral** builds irregular glass cells. Gold bevels separate the panes, source brightness lights their interiors, and a slow spectrum moves through the glass. Cell geometry is cached per renderer and resolution; the palette updates once per cell per frame. This is stylized glass, not physically traced refraction.
- **Ribbon relief** turns brightness into elevation. Overlapping opaque ribbons are drawn from back to front, with lit edges, shaded bodies, and dark gaps. The material breathes slightly while the shared final wave stays identical.
- **Paper theatre** separates the street into torn, colored layers. Small independent offsets and cast shadows give the cutouts depth. Its cobalt, vermilion, rose, ochre, and paper palette is deliberately more assertive than mineral.

Reproduce with `render.py`, `make_gallery.py`, and `verify.py`, using `--catalog artwork/editions/forms.json`. The local collection uses the same 719-frame recorded source as the preceding short studies: 1920 × 1080 at 30000/1001 fps. These videos are roughly 24 seconds long; the camera footage is not a seamless loop.

New materials are local experiments. No production pipeline, weather job, or streaming service is changed. Read the collection's MEASUREMENTS.md for native processing speed and encoded bitrate. Local costs do not establish a monthly hosting price.
