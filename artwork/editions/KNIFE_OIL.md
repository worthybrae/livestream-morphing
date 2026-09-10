# Fine knife: source pigment and oil relief

Local studies added 10 September 2026. Production remains edition 46.

- Edition 49 uses the camera's RGB colors with the reference knife surface.
- Edition 50 adds blade shoulders, dark paint creases, and internal scraped channels.
- The geometry, three stroke sizes, deterministic placement, and final 0.012-amplitude / frequency-8 / eight-second psychedelic warp are unchanged.

Color is sampled from five nearby source pixels per stroke. The undercoat also uses source RGB. A frame-local RGB copy prevents later coats from sampling already-painted pixels. Cloud and rain parameters supply a restrained warm/cool bias of at most three RGB levels; there is no replacement palette. The weather adjustment is constant per frame and texture is anchored in blade coordinates. Source motion still changes stroke direction and detail selection, as in the reference.

Relief reuses the existing SIMD pigment noise, adding arithmetic for grooves and edge lighting. No extra full-frame noise, blur, or lighting pass. Weather inputs are parameters: the preview includes illustrative weather and a cached forecast snapshot, not continuous production weather polling. A live weather feed is not enabled by this study.

## Reproduce

Run `artwork/editions/render.py` with `--catalog artwork/editions/knife-oil.json --workers 5 --encode-threads 2 --crf 23` and the same source clip as the reference. Local artifacts are under `/Users/worthy/Test/living-ink-paper-filaments-2026-09-10/knife-oil`; the snapshot's precise inputs and provenance are in `study-catalog.json` and `weather-status.json` there.

Validation: 125 Rust tests passed, including source hue retention for identical luminance geometry, bounded weather differences, deterministic moving editions, and byte-identical serial/parallel encoded output. The 719-frame edition-46 video remains byte-identical to the previous reference (SHA256 `b53c83818301ddeba60d968ad743b23fa464713047b106d1bf9824e3a7fa7054`). Timings in the artifact metrics are local, not a production cost guarantee.
