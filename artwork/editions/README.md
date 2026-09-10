# Living Ink: moving studies

A collection of material treatments for Worthy’s Abbey Road artwork. The original psychedelic distortion is the collection’s fixed signature. The studies change the way the street is marked, shaded, and colored underneath that movement.

The native `ink_editions` effect applies Living Ink’s contours with its internal warp disabled, builds the selected material, then applies the signature exactly once. Amplitude is 0.012, frequency is 8, period is 8 seconds. These are fixed constants, not edition controls. The RGB implementation uses the original monochrome sampler’s exact coordinates and interpolation weights; a byte-equivalence test protects the geometry.

No trails accumulate in these editions. Paper grain, screens, and hatching are deterministic in space. They bend with the artwork instead of independently flickering every frame. Source-frame time drives the signature. Independent worker instances produce the same ordered encoded output as a serial run.

## Collection

The first thirteen candidates explored pale and dark grounds. A second pass added engraved dots, woodcut, woven topography, and mineral cut-paper. The shortlist emphasizes materially different images, rather than only palette changes:

- **Graphite weather**: open paper, fine contours, crossed shadow hatching.
- **Blue hour**: cyanotype blue with a light tonal wash.
- **Street pressure**: topographic bands derived from brightness, anchored by the original contours.
- **Red registration**: vermilion lines over a blue printed screen.
- **Phosphor garden**: mineral-green light with a small spatial halo and no afterimage.
- **Black river**: broad dark masses opened by pale carved marks.
- **A street, woven**: a fine loom whose threads follow the scene’s brightness.
- **Mineral morning**: slate, lichen, and paper-colored cut shapes.

The remaining studies preserve quieter or more experimental alternatives: Silver street, Copper current, Porcelain road, Night glass, Dust in motion, Solar silver, Cinnabar crossing, Iris after dark, and Printed air. Keeping these available lets the artist judge the edges of the collection.

## Reproduction

Run from the stream-engine repository. Docker image `livestream-morphing-sdd-builder` contains Rust 1.86 and the FFmpeg development libraries. The repository’s Railway Dockerfile documents the matching native dependencies.

```sh
docker run --rm -v "$PWD/livestream-morphing-rs:/work" -w /work livestream-morphing-sdd-builder cargo build --release
python3 artwork/editions/render.py /absolute/output --source /absolute/source.ts --crf 23
python3 artwork/editions/verify.py /absolute/output --source /absolute/source.ts --crf 23
python3 artwork/editions/showreel.py /absolute/output
python3 artwork/editions/make_gallery.py /absolute/output
```

Each edition exports a 1920 × 1080 MP4, a native parameter JSON, a representative PNG, and measured local render statistics. The MP4 is a stream-copy remux of the native H.264 output. Source cadence is preserved; Abbey Road’s source runs at 30000/1001 fps.

`capture.py` can capture a bounded, continuous set of already-downloaded raw segments from the paired live API. Captures are local source material, not an additional live rendering service. The 90-second collection uses fifteen consecutive segments, 2,700 source frames. No frame interpolation or frame duplication is used.

Long local exports have a separate 18,000-frame limit. Public ingestion retains its original 720-frame decoder bound and its tighter 13-second segment validation. Both paths keep only a bounded number of decoded frames in flight. `ENCODE_CRF` defaults to 30, preserving the production encoder setting; local masters use 23.

These films are recorded studies, not seamless loops. Their camera action returns to the beginning on replay. The gallery plays one selected film and preserves the playback position when selecting another. It pauses when the page is hidden.

The public live artwork, stream settings, website, and hosting configuration are not changed by this collection.

## Viewing

Serve the collection with `python3 artwork/editions/serve.py /absolute/output --port 3089`, then open `http://127.0.0.1:3089/`. This local-only server supports byte ranges so switching materials can seek to the same recorded moment. Opening `index.html` directly also works in browsers that allow local media. The shortlist appears first. Select another material while a film plays to compare the same camera moment with the same phase of the signature movement.

Start with **Graphite weather**, **Black river**, and **A street, woven** for the largest differences in mark-making. **Blue hour** and **Phosphor garden** are closer companions to the original live drawing. **Street pressure** is the most abstract: changes in brightness become additional contours, so it deliberately exposes more detail and visual noise.

The 40-second `showreel.mp4` cuts through the eight shortlisted materials. Each cut advances five seconds along the same source footage, so the camera action and distortion phase continue forward. `showreel.json` lists the order. The showreel is re-encoded from the masters; individual downloads retain the direct native encode.

`manifest.json` records the source and renderer hashes, exact presets, measured output dimensions, frame counts, and local render statistics. The 17 finished studies are experiments for curation, not a requirement to publish all 17 as a single exhibition.
