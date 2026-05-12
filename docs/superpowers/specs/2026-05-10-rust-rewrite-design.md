# Rust Rewrite: Livestream Morphing

**Date:** 2026-05-10
**Status:** Draft
**Approach:** Option B — Rust core + FFmpeg libs

## Problem

The current Python + C++ + FFmpeg stack consumes excessive resources (maxed out CPU on a 32GB machine) for processing a single webcam stream. This makes it too expensive to deploy sustainably. The goal is a lean binary that can run on free/cheap-tier cloud hosting (Railway) with on-demand activation — $0 when nobody's watching.

## Architecture Overview

A single Rust binary replaces the entire Python/C++/FFmpeg-subprocess stack. FFmpeg is linked as a library (via `ffmpeg-next` crate) for H.264 decode/encode. Everything else — HTTP serving, image effects, HLS playlist management, stream fetching — is pure Rust.

```
Frontend (static site, always-on, user's existing website)
    │
    │  GET /api/stream  →  wakes backend
    ▼
┌──────────────────────────────────────────────────────┐
│  Rust Binary (single process, ~60-100MB RAM)         │
│                                                      │
│  axum HTTP server                                    │
│    ├── GET /api/stream        → M3U8 playlist        │
│    ├── GET /api/segments/:id  → .ts segment bytes    │
│    └── GET /health            → health check         │
│                                                      │
│  Pipeline Manager (tokio tasks)                      │
│    ├── Stream Source    → fetch Abbey Road HLS        │
│    ├── Codec (decode)   → ffmpeg-next → raw pixels   │
│    ├── Effects          → pure Rust pixel processing  │
│    ├── Codec (encode)   → ffmpeg-next → H.264 .ts    │
│    └── HLS Buffer       → ring buffer (10 segments)  │
│                                                      │
│  Auto-shutdown after 5 min idle                      │
└──────────────────────────────────────────────────────┘
```

### On-Demand Lifecycle

1. **SLEEPING** — Railway machine is stopped. $0 cost.
2. **WAKING** — Frontend sends `GET /api/stream`. Railway cold-starts the container (~1-2s). Returns empty M3U8 playlist.
3. **BUFFERING** — Pipeline starts fetching and processing Abbey Road segments. First 3 segments take ~10-15s.
4. **STREAMING** — Continuous processing. M3U8 playlist updates with new segments. Clients play video.
5. **SLEEPING** — No client requests for 5 minutes. Pipeline stops, buffers cleared, container sleeps.

## In-Memory Pipeline

The critical architectural change: **zero disk I/O** in the processing path.

### Current Pipeline (wasteful)

```
Download .ts to disk → PyAV reads from disk → copy to numpy → copy to C++ Mat
→ C++ processes → copy back to numpy → write JPEG to disk ×30
→ FFmpeg subprocess reads JPEGs from disk → write .ts to disk
```

- 5+ memory copies per frame
- 60+ disk operations per segment
- JPEG compression/decompression quality loss

### Rust Pipeline (in-memory)

```
HTTP GET → Vec<u8> in memory → ffmpeg-next decodes → raw pixel buffer
→ Rust effects (in-place mutation) → ffmpeg-next encodes → .ts bytes in ring buffer
```

- 0-1 copies per frame (in-place mutation where possible)
- Zero disk I/O
- No intermediate format conversion loss

### Per-Segment Timing (estimated, shared-cpu-1x)

| Step | Operation | Time |
|------|-----------|------|
| 1 | HTTP fetch .ts segment (~3-5MB) | ~200ms |
| 2 | ffmpeg-next decode → ~180 raw frames (30fps × 6s) | ~300ms |
| 3 | Effects processing (4 passes × 180 frames) | ~4.5s |
| 4 | ffmpeg-next encode → H.264 MPEG-TS | ~500ms |
| 5 | Store in ring buffer, update M3U8 | ~0ms |
| **Total** | | **~5.5s per 6s segment** |

Note: 180 frames at ~25ms/frame dominates. If this is too tight on a shared CPU, process every 2nd frame and duplicate (cuts effects time to ~2.25s, total to ~3.25s). The artistic style hides the frame doubling.

## Effects Pipeline

Four passes per frame, targeting a "live painting with texture" aesthetic. Operates on frames downsampled to 960×540 (half resolution), upsampled back to 1920×1080 with nearest-neighbor interpolation (preserves hard painterly edges).

### Pass 1: Psychedelic Sine-Wave Distortion (~10ms/frame)

Preserved from the current system. Applies coordinate remapping using sine waves for a melting/dreamy effect.

- Pre-compute distortion maps per row (SIMD-friendly)
- Bilinear interpolation for remapped coordinates
- Parameters: amplitude (0.01-0.05), frequency (8-20), cycle length (180 frames)
- Time-varying: the distortion animates continuously

### Pass 2: Color Quantization (~5ms/frame)

Replaces bilateral filter + posterization + morphology (3 passes → 1 pass). Snaps each color channel to N discrete levels.

- O(n) per pixel — just division and rounding
- 8-12 color levels for painterly flatness
- Optional small box blur on quantized result to soften region boundaries
- Operates on RGB channels independently

### Pass 3: Sobel Edge Overlay (~8ms/frame)

Same approach as current system. Detects color boundaries and draws dark outlines.

- Sobel gradient on grayscale of quantized image
- Threshold to binary edges
- Blend as dark lines over color image
- Tunable: edge thickness, darkness, threshold sensitivity

### Pass 4: Canvas Texture Blend (~2ms/frame)

New. Gives the output a physical "painted on canvas" feel.

- Static canvas/paper texture loaded once at startup via `include_bytes!`
- Tiled to match frame resolution
- Multiply-blend at 10-20% opacity
- Nearly free since texture is pre-loaded in memory
- Tunable: texture type (canvas, watercolor paper, linen), blend strength

### Time-of-Day Color

Preserved from the current system. London time determines the color palette:

- Grey level interpolated between lightest (noon) and darkest (midnight)
- Edge color adapts: dark outlines during day, light outlines at night
- Applied as a color adjustment after the effects pipeline

### Per-Frame Total: ~25ms

Compared to ~115ms in the current C++ pipeline (and ~5,500ms in the original pure-Python version).

## Project Structure

```
livestream-morphing-rs/
├── Cargo.toml
├── Dockerfile
├── textures/
│   └── canvas.png                # baked into binary via include_bytes!
└── src/
    ├── main.rs                   # entry point, CLI args, startup
    ├── server.rs                 # axum HTTP server, routes, idle tracking
    ├── pipeline.rs               # orchestrator: fetch → decode → process → encode
    ├── effects.rs                # image processing (distortion, quantize, edges, texture)
    ├── codec.rs                  # ffmpeg-next decode/encode wrappers
    ├── hls.rs                    # M3U8 playlist generation, segment ring buffer
    ├── stream_source.rs          # Abbey Road HLS fetcher (HTTP + M3U8 parsing)
    └── time_color.rs             # London time → color palette mapping
```

### Module Responsibilities

**main.rs** — Parse CLI args (port, idle timeout, log level). Initialize tracing. Start tokio runtime. Launch server and pipeline.

**server.rs** — Axum router with three routes: `/api/stream` (M3U8 playlist), `/api/segments/{id}.ts` (segment bytes), `/health` (health check). Tracks the timestamp of the last client request. Signals the pipeline to start when first client connects. CORS middleware for frontend access.

**pipeline.rs** — Owns the main processing loop. Coordinates fetch → decode → effects → encode. Manages the segment ring buffer (VecDeque of last 10 encoded segments). Uses `tokio::task::spawn_blocking` for CPU-bound work (effects + codec). Watches for idle shutdown signal.

**effects.rs** — Pure Rust pixel math. Four public functions: `apply_distortion()`, `quantize()`, `detect_edges()`, `blend_texture()`. All operate on `&mut [u8]` pixel slices in-place. No external image processing library — just math on byte arrays. Canvas texture stored as a static `&[u8]` via `include_bytes!`.

**codec.rs** — Thin wrapper around `ffmpeg-next`. Two functions: `decode_segment(bytes: &[u8]) -> Vec<Frame>` and `encode_segment(frames: &[Frame]) -> Vec<u8>`. Handles YUV↔RGB pixel format conversion. Configures H.264 encoder: CRF 25, ultrafast preset, MPEG-TS container.

**hls.rs** — Generates M3U8 playlists from the ring buffer. Assigns monotonically increasing media sequence numbers. Returns playlist as a `String` and segment bytes as `Bytes`.

**stream_source.rs** — Fetches the Abbey Road EarthCam HLS stream. Parses the M3U8 playlist to find the latest segment URI. Downloads `.ts` segments with retry logic (3 attempts). Includes the required EarthCam HTTP headers (Origin, Referer, User-Agent).

**time_color.rs** — Calculates the current London time using `chrono-tz`. Maps hour/minute to a grey level (lightest at noon, darkest at midnight). Returns edge color and background color for the effects pipeline.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `axum` | HTTP server |
| `tokio` (rt-multi-thread) | Async runtime |
| `ffmpeg-next` | H.264 decode/encode via FFmpeg C libs |
| `image` | Load canvas texture, pixel format helpers |
| `reqwest` | HTTP client for fetching stream segments |
| `chrono` + `chrono-tz` | London timezone calculations |
| `tracing` + `tracing-subscriber` | Structured logging |

System dependency: FFmpeg runtime libraries (`libavcodec`, `libavformat`, `libavutil`, `libswscale`).

## Deployment

### Dockerfile (multi-stage)

```dockerfile
# Build stage
FROM rust:1.83-bookworm AS builder
RUN apt-get update && apt-get install -y \
    libavcodec-dev libavformat-dev libavutil-dev libswscale-dev \
    pkg-config
WORKDIR /app
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    libavcodec60 libavformat60 libavutil58 libswscale7 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/livestream-morphing-rs /usr/local/bin/morphing
EXPOSE 8080
CMD ["morphing"]
```

### Railway Configuration

- **Build:** Dockerfile-based (Railway auto-detects)
- **Sleep:** Enable "Sleep after inactivity" in dashboard (5 min timeout)
- **Region:** Auto-selected (or choose US-East for closer to EarthCam servers)
- **Resources:** Minimal — the binary runs on the smallest available instance
- **Cost:** $0-3/mo within the $5 free tier credit

### Resource Budget

| Metric | Value |
|--------|-------|
| Binary size | ~10-15MB (release, stripped) |
| Container image | ~80-120MB |
| RAM (streaming) | ~60-100MB |
| RAM (idle) | 0MB (machine stopped) |
| CPU per segment | ~2-3s burst, then idle |
| Cold start | ~1-2s |
| Monthly cost | $0-3 (Railway free tier) |

## What's Not Included

- **Admin UI / live parameter tuning** — not in scope for the Rust rewrite. Can be added later as additional routes.
- **Multi-stream support** — hardcoded to Abbey Road. Single pipeline.
- **S3/R2 upload** — no cloud storage. Segments served directly from in-memory ring buffer.
- **Raw stream passthrough** — only the processed stream is served.
- **Neural style transfer** — the `style-transfer/` experiments are not ported.

## Migration Strategy

The Rust binary is a new project (`livestream-morphing-rs/`) alongside the existing Python backend. Both can run independently. The frontend switches between them by changing the API URL. The Python backend remains as a reference and fallback until the Rust version is validated.
