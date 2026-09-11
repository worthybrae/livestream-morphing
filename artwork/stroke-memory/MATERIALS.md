# Moving brushwork: paint materials

Three local material studies build on the approved adaptive-effort detail pass.
The shared source is the same 24.024-second Abbey Road recording at 1920 × 1080.
Source pigment uses the existing palette at a fixed 3 PM London setting.
Paint updates remain at 14.985 Hz, new-stroke decisions at 4.995 Hz, and the
established edge-anchored psychedelic wave/output at 29.97 fps.

## Comparisons

- **Loaded bristle**: fuller deposits, irregular raised edges, interrupted
  grooves and smoother patches of loaded paint. Unequal ends and changing
  pressure break up the uniform stamp shape.
- **Scumbled pigment**: irregular patches of thinner pigment reveal previous
  painted coats. Short rough marks and fuller small accents give a quieter,
  dry surface. The exposed layer is underpainting, never a raw-video overlay.
- **Swept oils**: longer curved gestures with dragged tips, pigment-load
  variation and stronger directional brushwork. It is the most exaggerated
  of these three directions; broad marks can simplify nearby forms.
- **Adaptive effort — reference**: the previously approved detail study,
  copied unchanged for comparison at the same point in the recording.

The first material samples had overly regular parallel grooves. The revised
masks use seeded, smoothly interpolated irregularity to break the grooves
into groups and vary pigment coverage. Ground strokes retain their original
quiet masks. Accents narrower than four pixels across retain the original
mask exactly; larger marks gradually gain the material treatment. This keeps
texture from consuming the smallest features and avoids its computation on
those tiny masks.

## Implementation and limitations

`native/src/material.rs` generates brush-local masks from persistent geometry
and seeds. Neither frame time nor screen position affects their texture.
The native renderer caches the masks under stroke IDs, so their grooves and
broken deposits travel with the marks. A fixed light direction shades the
relief. The existing subpixel compositor, binary stroke protocol and ordering
are unchanged; `plain` remains the default and uses the original mask code.

These are procedural approximations of paint, not a physical pigment/fluid
simulation. A mask is regenerated when a new stroke is born. The adaptive
planner sees the actual material raster between brush sizes, so it can choose
different finishing marks for each material. Its approximate within-batch
footprint pricing and 20,000-mark refinement cap remain unchanged.

The new pipe-level integration test verifies that material selection changes
the painting, small isolated accents retain the original pixels, repeated
frames remain identical, and integer motion transports the texture without
changing it. The full suite contains fifteen Python tests and the existing
native serial/parallel/append compositing test. These checks do not establish
that the artwork looks convincingly hand-painted; the recordings are for
visual selection.

```sh
cargo build --release --manifest-path artwork/stroke-memory/native/Cargo.toml
python3 artwork/stroke-memory/render.py /absolute/street.ts /absolute/loaded-bristle.mp4 --seconds 24.024 --detail adaptive --material loaded
python3 artwork/stroke-memory/render.py /absolute/street.ts /absolute/scumbled-pigment.mp4 --seconds 24.024 --detail adaptive --material scumble
python3 artwork/stroke-memory/render.py /absolute/street.ts /absolute/swept-oils.mp4 --seconds 24.024 --detail adaptive --material swept
```

All rendering is local. The production engine, website and hosting spend are
unchanged. Cached masks avoid rebuilding texture every frame, but these
prototypes still rasterize the full canvas and are not real-time production
implementations. Render metrics beside each video include analysis, mask
generation, refinement, raster transfer and encoding in total wall time.

The sequential full-clip renders measured:

| Material | Wall time for 24.024 s | Median initial raster round trip | Median refinement update |
| --- | ---: | ---: | ---: |
| Loaded bristle | 55.44 s | 46.04 ms | 54.83 ms |
| Scumbled pigment | 58.75 s | 50.06 ms | 62.19 ms |
| Swept oils | 61.48 s | 53.91 ms | 55.67 ms |

Refinement runs about five times per second and its timing includes appended
raster passes. All three reached a maximum total population of 33,858 marks.
The refinement cap prevented proposals on 82, 76 and 96 scale passes,
respectively. Machine contention varied markedly during short samples, so
these figures should not be interpreted as a speedup over earlier recordings
or a hosting-cost forecast. Each full video has 720 frames and decodes fully.

Preview: http://127.0.0.1:3092/stroke-memory/materials/#loaded-bristle
