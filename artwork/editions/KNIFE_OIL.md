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

## Mixed brushes (editions 55–56)

The previous pass was visually too subtle. These studies use three clearly different stroke families: broad loaded marks, very long narrow strokes, and short rounded dabs. Family and size variation are fixed by the stroke seed, so they do not randomize across frames. Edition 55 uses coat spacings 60/26/8 at 1080p; edition 56 increases these to 80/32/10 for larger gestures. Existing gradual orientation/detail response, source colors, day/night grading, softened lights, and final psychedelic warp are retained. Internal groove spacing also varies by stroke identity.

The larger spacing reduces stroke counts; geometry and overdraw still determine actual speed. Compare the rendered benchmarks rather than assuming cheaper or faster production. These are local previews.

## Brushed passages (edition 57)

Replaces the straight knife silhouette with a curved brush path, variable pressure, rounded ends and broken bristle coverage. Nearby strokes use a smooth regional direction field with restrained individual variation. Two extra source samples per stroke supply pigment dragged along its length. Anisotropic pigment noise runs along the bristles and contributes to edge breakup and paint load. Stroke identity, curve and regional field are deterministic; source gradients still produce gradual responses. Existing day/night grading and final psychedelic warp remain.

Conservative scanline bounds expand to include the curved footprint. Sampled boundary checks across the full seed range verified containment, and the 130-test suite passed including serial/parallel equivalence for edition 57. Compare against edition 55 using the same source. Local preview only; no production change.

## Reference-inspired impasto scales (editions 58–59)

Responds to the user's painting reference by contrasting broad loaded paint, short dabs and fine dragged marks. Width and length use independent stable seeds within three brush families. Ed58 coat spacings are80/30/8; ed59 uses150/58/12 for a more abstract surface. Medium coats become lighter in low-detail regions, and fine accents use a fixed55% (ed58) or34% (ed59) site density, allowing broad paint fields to survive overpainting. The detail response remains continuous. Curved paths, pigment drag, regional directions, source colors, night/day grading and psychedelic warp are inherited. Raised shoulders are strengthened slightly.

Local comparisons use the same daytime clip. The source colors are retained; the reference's blue palette is not copied. Ed59 intentionally trades more scene detail for visible paint scale. Production unchanged.

## Fine knife revisited (edition 60)

Returns to the original Fine knife geometry (36/18/8), full detail-site density, gradient-driven angles and original pigment/ridge texture. Retains source RGB and the accepted dark-gray-night/richer-day grading. Only20% of coarse stroke identities are eligible for a curved accent, and this fades to zero by gradient0.05. Eligible strokes lengthen up to45% with a gentle0.18-width bend. There is no regional direction field, large paint patch, or pigment dragging. The exact final psychedelic warp remains.

The comparison includes edition46 as it appears in production, rendered on the same clip. Edition60 is a local preview; production is unchanged.131 tests passed, including serial/parallel equivalence for the new edition.

## Three brush directions (editions 61–63)

Recorded comparison against edition 60, all at noon with cloud 0.3. Loaded brush (61) uses a pressure belly, loaded heel, tapered dry end and longitudinal bristle gaps. Paint over paint (62) uses broader underpainting and selective translucent overpainting; layers are spatial, rebuilt per frame, with no temporal trails. Quiet fields (63) concentrates finer marks at luminance detail and keeps broad passages quieter.

The three new modes anchor stroke angles, sizes and curves to spatial identities rather than changing gradients. Source pigment and detail opacity still respond to video. All retain source RGB, the existing day/night/weather grading and the unchanged final psychedelic warp. Preview weather is fixed, not a live weather integration. These are local studies, not production changes.

The New York comparison uses a separately captured Times Square sample from the supplied EarthCam playlist (camera 50683), 1920 × 1080 at 30 fps. The catalog records the America/New_York local hour at capture; weather inputs remain illustrative. Signed playlist credentials are not saved in the repository.

## New York live refinement (edition 64)

Fine knife revisited retains its pigment, small geometry and original psychedelic wave. Continuous bounded gradient alignment removes the previous angle threshold. Fine accents use 6px sites rather than 8px; opacity ramps above gradient 0.065 so quiet regions avoid unnecessary detail work. The live pipeline snapshots America/New_York time each segment. Weather remains at the existing default, not a live weather feed.

LIVE_STYLE=fine_knife_nyc selects this edition. STREAM_URL uses the verified unsigned EarthCam 50683 chunklist prefix. EarthCam Origin and Referer headers replace Abbey Road headers. Five frame workers and two encode threads remain; a six-worker/one-encoder comparison was slower locally.

Production verification of the generic x86 build measured roughly 7.8–8.2s per 6s segment. The Railway-only Docker build now targets x86-64-v2 (SSE4.2-class CPUs, supported by the observed production CPU) so wide SIMD operations use hardware rounding and integer arithmetic. Other-architecture builders must override RUSTFLAGS. Validate production timing after deploy; local ARM timing is not a substitute.

## Small forms (edition 65)

Uses the Fine knife coarse/middle passes, with a separate radius-one luminance layer for details. On a four-pixel grid, small contrast islands receive compact source-colored dabs instead of long knife strokes. Broad smoothing no longer erases the detail signal before these marks are chosen. This is size/contrast-based reconstruction, not semantic person/building segmentation. Day/night grading and the final wave remain unchanged.

The target-specific Railway build runs the Rust tests before building its release binary.

The first dot-only pass also dotted long rails and building edges. A local Hessian compactness check now separates compact contrast islands from elongated edges: the latter receive restrained short thin marks. The analysis is lightweight and deterministic per frame. Broad paint remains unchanged.
