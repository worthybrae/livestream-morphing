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

## London clock tint (edition 51)

A stronger tint follows smooth clock anchors: blue night, rose dawn, warm daytime, amber evening, violet twilight. Camera RGB and oil-relief texture remain the base. Clouds soften the warm shift and rain adds coolness. The tint is calculated once per frame; the stroke shader and psychedelic warp are unchanged. The midnight boundary is continuous. These clock anchors are artistic and do not claim to track seasonal sunrise/sunset.

`knife-daylight.json` contains three fixed-hour comparisons plus a 24-hour cycle compressed into the 24-second recorded clip. Weather is explicitly illustrative. Production still uses edition 46, and this preview does not add a live clock/weather fetcher to it.
