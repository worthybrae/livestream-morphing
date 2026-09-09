# Monochrome Fullscreen Livestream Design

## Context

The production portfolio route at `/art/livestream-art` currently embeds the livestream inside the same narrow media block used by other art pieces. The Rust stream processor also preserves source RGB values because its quantization effect operates on each color channel independently. This differs from the original Python implementation, which converted every frame to grayscale before smoothing, quantization, and edge enhancement.

The approved direction is the “cinematic cut”: the stream owns the opening viewport, then scrolls completely away before the existing project text appears. The page must remain visually consistent with the rest of `worthyrae.com` and retain its existing navigation and transition language.

## Goals

- Make the live artwork the full opening experience of the portfolio route.
- Preserve the current immediate prerecorded fallback and automatic transition to live playback.
- Restore the original monochrome visual language: black, gray, white, and visible linework.
- Preserve the portfolio’s existing navigation, typography, black background, project metadata, and melt transition.
- Keep the existing low-cost, wake-on-view Railway architecture and current public API surface.

## Non-goals

- Redesigning other art or project detail pages.
- Adding interactive stream controls or exposing the admin pipeline.
- Moving the stream to AWS or Cloudflare.
- Changing the stream’s wake/sleep policy, resolution, bitrate, or cost controls.
- Making the livestream persist as a floating or inline player after the hero scrolls away.

## Stream Processing

The default Rust processing pipeline will add an explicit grayscale stage immediately after distortion and before quantization. The stage will compute luminance for each RGB pixel and write that same value back to all three channels. Quantization will therefore create neutral stepped tones instead of independently quantized colors.

The existing edge stage will remain after quantization so it operates on the simplified tonal fields and produces visible contour lines. The existing time-based edge-darkness behavior may still choose darker or lighter line emphasis, but it must not introduce hue. Canvas texture remains the final stage.

The default order becomes:

1. Distortion
2. Grayscale conversion
3. Quantization
4. Edge detection and line overlay
5. Canvas texture

This is intentionally narrower than recreating every OpenCV operation from the historical Python implementation. It fixes the confirmed color mismatch while preserving the performant Rust pipeline and its current painterly character.

## Portfolio Layout

Only the `livestream-art` detail route receives the new composition. Other art pieces continue to use the existing `DetailOverlay` layout unchanged.

The livestream page consists of two consecutive regions:

1. A hero whose height is the opening viewport. The existing site navigation remains present over the top of the experience. The video fills the available viewport with `object-fit: cover`, square corners, and a black background. The compact live-state label remains over the video in the site’s monospace style.
2. The existing project-detail content on solid black. The title, description, summary, tools, and repository link retain the current typography, spacing language, and melt/fade behavior.

Normal document scrolling creates the transition. The hero is not sticky and does not animate into another size. As the user scrolls, it moves naturally off the top of the page, and the text section follows. This keeps the behavior direct, accessible, and consistent with the selected cinematic-cut direction.

## Playback Behavior

The existing two-video player architecture remains unchanged:

- The prerecorded Abbey Road video is visible immediately on route entry.
- Loading the page wakes the Railway stream and polls readiness.
- Once live playback can begin, the live element crossfades over the fallback.
- Stream stalls or fatal playback errors return the viewer to the fallback while retrying.
- Leaving the route unmounts the player and preserves the existing serverless inactivity behavior.

The layout component will make the player fill its parent rather than enforcing its own `aspect-ratio`. This allows the same player logic to support the fullscreen hero without duplicating stream state.

## Responsive Behavior

- Desktop and mobile both use a viewport-height hero.
- The video remains cover-cropped rather than letterboxed.
- Safe viewport units are preferred where supported so mobile browser chrome does not create a visible gap.
- Status copy remains readable against the image with the existing dark backing.
- The text section retains the portfolio’s current responsive content width and padding.

## Testing and Verification

Stream tests will prove that grayscale conversion sets all three channels to the same luminance and that the default pipeline places it before quantization and edges. Existing stream tests must continue to pass.

Portfolio tests will prove that:

- `livestream-art` uses the dedicated fullscreen detail composition.
- Other art pages retain the existing composition.
- The live player can fill a parent without forcing `aspect-video`.
- Existing fallback, readiness, crossfade, retry, and cleanup behavior is preserved.

Before deployment, both repositories will run their relevant test suites and production builds. After deployment, the production route will be checked in a desktop browser for immediate fallback playback, transition to a monochrome live feed, full-viewport opening layout, normal scroll into text, and unchanged navigation.

## Deployment

The stream change will deploy to the existing Railway project using the already authenticated local Railway session. The portfolio change will deploy through the existing portfolio deployment path. No new credentials, AWS resources, Cloudflare configuration, or third-party services are required.

