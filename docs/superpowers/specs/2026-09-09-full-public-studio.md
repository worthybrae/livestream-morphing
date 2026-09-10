# Full public drawing studio

The public page now exposes every LivingInk parameter, grouped into contours, flow, memory, and charcoal/light. Arbitrary slider changes redraw a recorded sample, including while paused. Starting looks remain shortcuts, and visitors can save/restore in browser storage, download a parameter JSON, compare the camera image, select preview resolution, and open the original recorded-study fallback.

## Cost and architecture

The same LivingInk and Distortion Rust source files compile into `ink-wasm`. The shared frame/trait types were extracted without semantic changes. The only effect API addition supplies explicit source time, so skipped preview frames do not change flow speed. Native calls preserve their existing frame-clock calculation, covered by an equality test. The deployed live stream is unchanged.

WASM processes pixels in a dedicated worker. JavaScript transfers decoded video frames and displays the result; it does not reimplement the drawing effect. One frame is allowed in flight. Visibility pauses playback; paused edits and the final paused source frame queue a redraw when needed. Loop/discontinuity and parameter changes clear temporal history. Browser playback rejection remains retryable. Engine errors retain access to recorded studies.

The shipped WASM is 149105 bytes; the recorded source MP4 is approximately 590 KiB. Both load only after Start. There are no new server render jobs, admin routes, services, or infrastructure. Normal static delivery still applies. `ink-wasm/publish-assets.mjs` copies the compiled artifact and derives all 14 slider definitions from it; a portfolio test verifies metadata against the actual binary.

The preview starts at 1920×1080 with 720p/540p options. It starts paused. Local browser tuning measurements were roughly 125–145 ms/frame at 1080p (single-digit animated preview FPS), depending on load and settings. This is not a 1080p/30 guarantee for client CPUs. Report measured preview timing; offer lower sizes or the prerecorded full-HD studies for smooth playback. The existing live stream remains 1080p at 29.97 fps. No production cost estimate changes are inferred from local preview timing.

## Explanation

A five-stage interactive diagram explains luminance, smoothing, contour extraction, temporal memory, and flow. Each stage has a conceptual illustration, plain-language text, and a matching equation. Animation is opt-in, pauses offscreen/hidden, and respects reduced motion. A note explains that charcoal/highlight composition precedes the final warp. Illustrations are labeled conceptual rather than renderer diagnostics.

## Verification

111 native Rust tests pass. 15 WASM crate tests pass, including the same effect tests plus bridge bounds and pixel output. Portfolio tests include the actual compiled WASM's metadata/output, arbitrary sliders and save/restore validation, deferred source loading, diagram selection/animation toggles, worker redraws after offscreen edits, final-frame pause behavior, and retryable playback failures. The reviewer identified the last three lifecycle cases; fixes and focused tests are included.

Browser QA confirmed all 14 labeled sliders, native WASM drawing at 1920×1080, an arbitrary threshold change to 33 while paused, playback controls, and the animated flow diagram. The unchanged live pipeline stays deployed independently.

## Deployment

Portfolio commit `0be38c0`, Railway deployment `e9d06948-dcb7-4cd7-8b55-381eda846763`: SUCCESS. Shared Rust/browser source commit `873f07f`. Final portfolio checks: 102 tests passed and production build passed. Native: 111 tests; WASM crate: 15 tests. Production WASM and source URLs returned 200 with correct application/wasm and video/mp4 content types and byte lengths (149105 and 604202). Browser confirmed all 14 sliders, initialized 1920×1080 canvas, no engine error, and an arbitrary fine-threshold value of 33. A measured production-page preview render was 106 ms on this device. Reset restored the starting look.
