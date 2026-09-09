# Living Ink Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Replace muddy filtering with a sparse, time-aware Rust contour painting within the existing stream budget.
**Architecture:** One Rust renderer serves live and offline output. Validate renderer quality and resource cost locally before integrating studio and deploying the portfolio.
**Tech Stack:** Rust, ffmpeg-next, Axum, React, HLS, Docker.
**Spec:** docs/superpowers/specs/2026-09-09-living-ink-design.md

## Global Constraints
- No new paid infrastructure.
- Rust is the sole renderer.
- Target approximately $5/month for the stream, including idle memory; this is a target to verify, not a guaranteed cap.
- Local preview must not activate the production service.

## Task 1: Native renderer and regression proof
Files: src/effects/living_ink.rs (new); src/effects/mod.rs; src/registry.rs; src/pipeline.rs under livestream-morphing-rs.
Interface: LivingInk implements Effect; FrameCtx adds delta_seconds: f32; Pipeline::process_frame_timed(frame, frame_number, delta_seconds) preserves the existing convenience method.
- [x] Add tests asserting uniform input outputs uniform charcoal, moving boundaries leave fading strokes, static contours do not brighten, and repeated set_dimensions preserves history.
- [x] Run `cargo test living_ink` in the existing Docker builder and observe failure before implementation.
- [x] Implement preallocated float buffers, luminance smoothing, thinned two-scale contours and exponential trail decay. Clamp parameters to registry ranges; reset on init.
- [x] Run all Rust tests and commit renderer with its proof.

## Task 2: Source timing, lean processing and local replay
Files: src/codec.rs, src/pipeline.rs, src/hls.rs, src/main.rs, src/offline.rs (new).
Interface: decoded frame batches carry source FPS; encode uses that FPS; HLS stores frames/FPS duration. Offline command accepts a captured input file, output TS path, and width and emits JSON timing metrics.
- [x] Preserve source cadence, decode directly to output width, eliminate the second batch, share processing between normal/prefetched paths.
- [x] Set a one-minute idle grace and reset temporal state at session boundaries.
- [x] Capture one source segment. Replay identical input at 640 and 960 widths; compare old/new output using native Rust.
- [x] Measure frame p50/p95, total segment time, bitrate, peak RSS and estimate monthly cost before choosing production resolution.

## Task 3: Isolated studio and telemetry
Files: src/api.rs, src/server.rs, src/pipeline.rs; studio/src/App.tsx and dedicated preview module.
- [x] Expose safe, non-waking public engine telemetry.
- [x] Add a local draft preview that renders captured input through the same Rust effect; only one bounded render may run at a time.
- [x] Display equation, validated sliders, measured timing, and versioned preset export; never apply slider changes to the public pipeline.
- [x] Test public routes cannot invoke previews or mutate presets.

## Task 4: Portfolio, verification and release
Files: personal/src/components/LiveStream, personal/src/pages/Home.tsx, related overlay/navigation code.
- [x] Extend hero behind navigation and keep normal scroll behavior.
- [x] Show real engine telemetry and stop background demand when the page is hidden.
- [x] Run portfolio tests/build and studio build; visually inspect live-rendered comparison and local page.
- [ ] Deploy only if benchmark and cost projection pass; verify live playback and retain rollback deployment identifiers.

Implementation note: preview uses a captured clip rendered on explicit request, with versioned export for production. This avoids a paid preview service and keeps production publishing tied to a reviewed deployment. See the benchmark report for the 360p release decision.
