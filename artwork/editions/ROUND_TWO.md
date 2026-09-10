# Mineral, moving threads, and quieter reflections

The second round develops the user's choices from the first collection. Original mineral, woven, isobars, and halftone remain available for comparison. The public live treatment is not changed by these experiments.

## Mineral

Breathing pigments interpolates a hand-picked stone, clay, and moss palette over 48 seconds. Only five pigment colors are calculated each frame. Color movement is separate from the unchanged signature geometry: amplitude 0.012, frequency 8, period 8 seconds, applied once after the material.

The atmosphere version blends the pigments using London hour, forecast cloud cover, and forecast precipitation. Dawn warms the lighter colors, overcast conditions mute the palette, rain shifts it toward cool slate, and night leaves a more luminous blue-gray drawing. These are artistic mappings, not measurements of the actual colors of the sky.

The day/night curve currently uses a fixed artistic clock schedule. It does not calculate seasonal sunrise or solar elevation. The accelerated day study compresses 24 hours into about 24 seconds so the behavior can be judged. It is explicitly a simulation. A separate “today's palette” uses a dated forecast sample and current London clock time on the recorded footage, not historical conditions from when it was filmed.

## Woven, chrome, and halftone

Traveling threads use one moving horizontal screen shaped by image brightness. The threads travel independently of the final signature wave and gently change their relief over 16 seconds. There is no memory or accumulated trail.

Cold chrome uses smoothed brightness gradients to construct broad reflection bands, with fewer fine contours than isobars. It is a stylized reflection effect, not physically traced glass or a 3D scene.

Open-screen halftone uses 12-pixel cells instead of 7-pixel cells at 1080p. Squared-distance coverage avoids a square root per pixel; the dots are antialiased and the final wave remains the same. The larger marks are also intended to compress more easily.

## Weather sampling

`weather.py` is a local, optional sampler. It never changes the public pipeline. It reads MET Norway's nearby forecast once, respects cache expiration and conditional requests, and derives a small parameter JSON. Clock conversion uses `Europe/London`, including daylight saving time. When the API is unavailable it uses a still-relevant cached forecast, or neutral weather inputs with an explicit unavailable status. Old forecasts more than two hours from the requested time are rejected.

```sh
python3 artwork/editions/weather.py /absolute/weather-now.json --cache /absolute/weather-cache.json
```

A live integration should run this as one shared backend sampler, with gradual parameter transitions; it should not fetch weather per frame or per visitor. That live integration has not been enabled. A forecast is not a street-level rain sensor, and the precipitation signal describes a forecast hour rather than a measured wet road.

Weather data: [MET Norway](https://api.met.no/doc/TermsOfService), [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Forecast fields are adapted into color inputs. The application does not imply MET Norway endorsement. Coordinates are approximate to the Abbey Road crossing.

## Reproduction and measurements

Use the existing renderer and gallery scripts with `--catalog artwork/editions/round-two.json`. Catalog entries may include `params`; those values seed each native preset. The `frames` field records the separately probed frame count for the longer mineral examples.

The core comparison source has 719 frames; the longer mineral pair has 1,438 frames. Both are stream-copy cuts from the original recording, without an extra lossy source encode. All outputs remain 1920 × 1080 at 30000/1001 fps.

The palette cycle is complete in the longer mineral film, but the camera recording itself is not a seamless loop. The viewer preserves position when switching studies; shorter studies clamp to their end if the chosen position is beyond their duration.

Measure native rendering and encoded bitrate separately. A cheap color operation can still change video compression cost, while dense dot screens can be bandwidth-heavy. Local timing results are comparisons on this machine, not promises about a monthly hosting bill.
