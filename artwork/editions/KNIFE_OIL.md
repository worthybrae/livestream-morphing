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

## Softer night lights (edition 52)

A smooth highlight shoulder compresses bright source colors before depositing paint. At full night, an RGB peak of 255 becomes about 200 before texture lighting. Peaks up to 150 stay unchanged. Scaling all channels together preserves the light's source hue. The blue clock tint is reduced on highlights to preserve warm lamps. This is brightness-based, not lamp/object detection; other bright surfaces are compressed too.

The adjustment fades out between 4:30 and 8 AM and in between 6 and 9 PM using the same illustrative London clock convention. At midday it is byte-identical to edition 51. A 256-entry table is computed once per frame. No new blur or bloom pass is added. The lookup is used both for underpaint and stroke colors; knife texture and final psychedelic warp remain unchanged.

The `knife-night.json` before/after pair uses 18 seconds of raw night footage captured from the existing stream; source provenance is saved alongside it. Both treatments use identical fixed clock/weather values. Validation: 127 Rust tests passed, including a monotonic highlight curve, unchanged shadows and daytime output, bounded night strength, and parallel rendering equivalence. Local previews only; production is unchanged.

## Charcoal night / colorful day (edition 53)

Uses the same smooth night strength as the soft-light treatment. Night reduces source saturation to 22% and exposure to 62%, while reducing the old blue/violet clock tint to a trace. Day uses 140% source saturation and 104% exposure, retaining restrained clock/weather color. Night highlight compression remains enabled. The paint texture is laid over these graded pigments, so it stays visible in the darker image. This changes saturation and brightness rather than inverting RGB values.

`knife-atmosphere.json` supplies actual night footage, actual day footage, and an explicitly illustrative 24-hour cycle on one daytime clip. New edition only; previous studies and production are unchanged.

## Painted flow (edition 54)

Longer, narrower marks with rounded, curved, bristled tips, clipped inside the original convex footprint. The existing pigment field supplies the breakup; no extra noise field is generated. Raised pigment channels remain. Fine detail opacity now ramps continuously over a contrast interval instead of appearing at 0.065. Stroke orientation uses bounded continuous alignment around deterministic anchors, removing the old weak-gradient angle switch. These measures reduce structural popping while retaining independent frames, parallel rendering, and the exact final psychedelic warp. There is no temporal image blending; source movement and color still update each frame.

This is a new material rather than a change to existing previews. It inherits edition 53's dark gray night / richer day grading and softened lights. All 129 Rust tests passed, including continuity at old gradient thresholds and encoded serial/parallel equivalence. Judge the before/after in motion; continuity tests are not a claim that all temporal artifacts have been removed.
