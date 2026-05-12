# Rust Rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Python+C++/FFmpeg stack with a single lean Rust binary that processes the Abbey Road livestream with painterly effects and serves it as HLS, targeting Railway free-tier deployment.

**Architecture:** Single Rust binary using axum for HTTP, ffmpeg-next for H.264 codec, and pure Rust for image effects. On-demand activation — pipeline starts when clients connect, sleeps after 5 min idle. All processing happens in-memory with a ring buffer of encoded segments.

**Tech Stack:** Rust, axum, tokio, ffmpeg-next, reqwest, chrono/chrono-tz, tracing

**Spec:** `docs/superpowers/specs/2026-05-10-rust-rewrite-design.md`

---

## File Structure

```
livestream-morphing-rs/
├── Cargo.toml
├── Dockerfile
├── src/
│   ├── main.rs              # Entry point, CLI args, startup
│   ├── server.rs            # Axum HTTP server, routes, idle tracking
│   ├── pipeline.rs          # Orchestrator: fetch → decode → process → encode
│   ├── effects.rs           # Image processing (distortion, quantize, edges, texture)
│   ├── codec.rs             # ffmpeg-next decode/encode wrappers
│   ├── hls.rs               # M3U8 playlist generation, segment ring buffer
│   ├── stream_source.rs     # Abbey Road HLS fetcher
│   └── time_color.rs        # London time → color palette
└── tests/
    └── integration.rs       # End-to-end pipeline test
```

---

### Task 1: Project Scaffolding

**Files:**
- Create: `livestream-morphing-rs/Cargo.toml`
- Create: `livestream-morphing-rs/src/main.rs`
- Create: `livestream-morphing-rs/src/effects.rs`
- Create: `livestream-morphing-rs/src/time_color.rs`
- Create: `livestream-morphing-rs/src/hls.rs`
- Create: `livestream-morphing-rs/src/stream_source.rs`
- Create: `livestream-morphing-rs/src/codec.rs`
- Create: `livestream-morphing-rs/src/pipeline.rs`
- Create: `livestream-morphing-rs/src/server.rs`

- [ ] **Step 1: Create project directory**

```bash
mkdir -p livestream-morphing-rs/src
mkdir -p livestream-morphing-rs/tests
```

- [ ] **Step 2: Write Cargo.toml**

```toml
[package]
name = "livestream-morphing-rs"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
ffmpeg-next = "7"
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls"] }
chrono = "0.4"
chrono-tz = "0.10"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tempfile = "3"
tower-http = { version = "0.6", features = ["cors"] }

[profile.release]
opt-level = 3
lto = true
strip = true
```

- [ ] **Step 3: Write stub module files**

`src/main.rs`:
```rust
mod codec;
mod effects;
mod hls;
mod pipeline;
mod server;
mod stream_source;
mod time_color;

fn main() {
    println!("livestream-morphing-rs");
}
```

`src/effects.rs`:
```rust
// Image processing effects pipeline
```

`src/time_color.rs`:
```rust
// London time → color palette
```

`src/hls.rs`:
```rust
// HLS buffer and playlist generation
```

`src/stream_source.rs`:
```rust
// Abbey Road stream fetcher
```

`src/codec.rs`:
```rust
// ffmpeg-next decode/encode
```

`src/pipeline.rs`:
```rust
// Pipeline orchestrator
```

`src/server.rs`:
```rust
// Axum HTTP server
```

- [ ] **Step 4: Verify project compiles**

```bash
cd livestream-morphing-rs && cargo check
```

Expected: compiles with warnings about unused modules.

- [ ] **Step 5: Commit**

```bash
git add livestream-morphing-rs/
git commit -m "feat: scaffold Rust rewrite project with dependencies"
```

---

### Task 2: time_color Module

**Files:**
- Modify: `livestream-morphing-rs/src/time_color.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_is_lightest() {
        assert_eq!(get_grey_level(12, 0), 175);
    }

    #[test]
    fn midnight_is_darkest() {
        assert_eq!(get_grey_level(0, 0), 25);
    }

    #[test]
    fn six_am_is_midpoint() {
        let level = get_grey_level(6, 0);
        assert!(level > 90 && level < 110, "6am should be mid-grey, got {level}");
    }

    #[test]
    fn six_pm_mirrors_six_am() {
        let am = get_grey_level(6, 0);
        let pm = get_grey_level(18, 0);
        assert_eq!(am, pm);
    }

    #[test]
    fn light_background_gets_dark_edges() {
        let (edge, bg) = get_colors(12, 0);
        assert!(bg.0 > 127);
        assert_eq!(edge, (0, 0, 0));
    }

    #[test]
    fn dark_background_gets_light_edges() {
        let (edge, bg) = get_colors(0, 0);
        assert!(bg.0 <= 127);
        assert_eq!(edge, (255, 255, 255));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd livestream-morphing-rs && cargo test time_color
```

Expected: FAIL — functions not defined.

- [ ] **Step 3: Implement time_color**

```rust
use chrono::Utc;
use chrono_tz::Europe::London;

pub type Color = (u8, u8, u8);

/// Maps hour+minute to a grey level (25=darkest at midnight, 175=lightest at noon).
pub fn get_grey_level(hour: u32, minute: u32) -> u8 {
    let total_minutes = hour * 60 + minute;
    let distance_from_noon = if total_minutes > 720 {
        1440 - total_minutes
    } else {
        total_minutes
    };
    let lightest: u8 = 175;
    let darkest: u8 = 25;
    let range = (lightest - darkest) as f32;
    darkest + (range * distance_from_noon as f32 / 720.0) as u8
}

/// Returns (edge_color, background_color) based on time of day.
pub fn get_colors(hour: u32, minute: u32) -> (Color, Color) {
    let grey = get_grey_level(hour, minute);
    let bg = (grey, grey, grey);
    let edge = if grey > 127 { (0, 0, 0) } else { (255, 255, 255) };
    (edge, bg)
}

/// Gets colors for the current London time.
pub fn get_colors_now() -> (Color, Color) {
    let london_now = Utc::now().with_timezone(&London);
    get_colors(london_now.hour(), london_now.minute())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_is_lightest() {
        assert_eq!(get_grey_level(12, 0), 175);
    }

    #[test]
    fn midnight_is_darkest() {
        assert_eq!(get_grey_level(0, 0), 25);
    }

    #[test]
    fn six_am_is_midpoint() {
        let level = get_grey_level(6, 0);
        assert!(level > 90 && level < 110, "6am should be mid-grey, got {level}");
    }

    #[test]
    fn six_pm_mirrors_six_am() {
        assert_eq!(get_grey_level(6, 0), get_grey_level(18, 0));
    }

    #[test]
    fn light_background_gets_dark_edges() {
        let (edge, bg) = get_colors(12, 0);
        assert!(bg.0 > 127);
        assert_eq!(edge, (0, 0, 0));
    }

    #[test]
    fn dark_background_gets_light_edges() {
        let (edge, bg) = get_colors(0, 0);
        assert!(bg.0 <= 127);
        assert_eq!(edge, (255, 255, 255));
    }
}
```

Add `use chrono::Timelike;` at the top for `.hour()` and `.minute()`.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd livestream-morphing-rs && cargo test time_color
```

Expected: all 6 tests PASS.

- [ ] **Step 5: Commit**

```bash
cd livestream-morphing-rs && git add src/time_color.rs && git commit -m "feat: add time_color module with London time color mapping"
```

---

### Task 3: Effects Module — Core Functions

**Files:**
- Modify: `livestream-morphing-rs/src/effects.rs`

- [ ] **Step 1: Write RawFrame struct and quantization test**

```rust
use std::f32::consts::PI;

/// RGB24 pixel buffer.
#[derive(Clone)]
pub struct RawFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl RawFrame {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            data: vec![0u8; (width * height * 3) as usize],
            width,
            height,
        }
    }

    pub fn filled(width: u32, height: u32, r: u8, g: u8, b: u8) -> Self {
        let mut frame = Self::new(width, height);
        for pixel in frame.data.chunks_exact_mut(3) {
            pixel[0] = r;
            pixel[1] = g;
            pixel[2] = b;
        }
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_snaps_to_levels() {
        // With 2 levels, values snap to 0 or 255
        let mut frame = RawFrame::new(2, 1);
        frame.data = vec![50, 50, 50, 200, 200, 200];
        quantize(&mut frame, 2);
        assert_eq!(frame.data, vec![0, 0, 0, 255, 255, 255]);
    }

    #[test]
    fn quantize_with_more_levels() {
        let mut frame = RawFrame::new(1, 1);
        frame.data = vec![100, 100, 100];
        quantize(&mut frame, 4);
        // 4 levels: 0, 85, 170, 255. 100 is closest to 85.
        assert_eq!(frame.data, vec![85, 85, 85]);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd livestream-morphing-rs && cargo test effects
```

Expected: FAIL — `quantize` not defined.

- [ ] **Step 3: Implement quantize**

```rust
/// Snap each color channel to N discrete levels.
pub fn quantize(frame: &mut RawFrame, levels: u8) {
    let step = 255.0 / (levels - 1) as f32;
    for byte in frame.data.iter_mut() {
        let val = *byte as f32;
        *byte = ((val / step).round() * step).clamp(0.0, 255.0) as u8;
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd livestream-morphing-rs && cargo test effects::tests::quantize
```

Expected: PASS.

- [ ] **Step 5: Write distortion tests**

Add to the `tests` module:

```rust
    #[test]
    fn distortion_zero_amplitude_is_identity() {
        let src = RawFrame::filled(4, 4, 128, 64, 32);
        let mut dst = RawFrame::new(4, 4);
        apply_distortion(&src, &mut dst, 0, 0.0, 12.0, 180);
        assert_eq!(src.data, dst.data);
    }

    #[test]
    fn distortion_nonzero_amplitude_changes_pixels() {
        let mut src = RawFrame::new(8, 8);
        // Create a gradient so distortion has something to shift
        for y in 0..8u32 {
            for x in 0..8u32 {
                let idx = ((y * 8 + x) * 3) as usize;
                src.data[idx] = (x * 32) as u8;
                src.data[idx + 1] = (y * 32) as u8;
                src.data[idx + 2] = 0;
            }
        }
        let mut dst = RawFrame::new(8, 8);
        apply_distortion(&src, &mut dst, 10, 0.05, 12.0, 180);
        assert_ne!(src.data, dst.data);
    }
```

- [ ] **Step 6: Implement apply_distortion**

```rust
/// Sine-wave coordinate remapping for psychedelic melting effect.
/// Writes from `src` into `dst` with bilinear interpolation.
pub fn apply_distortion(
    src: &RawFrame,
    dst: &mut RawFrame,
    frame_number: u32,
    amplitude: f32,
    frequency: f32,
    cycle_length: u32,
) {
    let w = src.width;
    let h = src.height;
    let wf = w as f32;
    let hf = h as f32;
    let time = (frame_number % cycle_length) as f32 * (2.0 * PI / cycle_length as f32);

    for y in 0..h {
        let y_offset = (time + y as f32 * frequency / hf).sin() * hf * amplitude;
        for x in 0..w {
            let x_offset = (time + x as f32 * frequency / wf).sin() * wf * amplitude;

            let src_x = (x as f32 + x_offset).clamp(0.0, wf - 1.0);
            let src_y = (y as f32 + y_offset).clamp(0.0, hf - 1.0);

            // Bilinear interpolation
            let x0 = src_x.floor() as u32;
            let y0 = src_y.floor() as u32;
            let x1 = (x0 + 1).min(w - 1);
            let y1 = (y0 + 1).min(h - 1);
            let fx = src_x.fract();
            let fy = src_y.fract();

            let dst_idx = ((y * w + x) * 3) as usize;
            for c in 0..3 {
                let p00 = src.data[((y0 * w + x0) * 3) as usize + c] as f32;
                let p10 = src.data[((y0 * w + x1) * 3) as usize + c] as f32;
                let p01 = src.data[((y1 * w + x0) * 3) as usize + c] as f32;
                let p11 = src.data[((y1 * w + x1) * 3) as usize + c] as f32;
                let val = p00 * (1.0 - fx) * (1.0 - fy)
                    + p10 * fx * (1.0 - fy)
                    + p01 * (1.0 - fx) * fy
                    + p11 * fx * fy;
                dst.data[dst_idx + c] = val.clamp(0.0, 255.0) as u8;
            }
        }
    }
}
```

- [ ] **Step 7: Run distortion tests**

```bash
cd livestream-morphing-rs && cargo test effects::tests::distortion
```

Expected: PASS.

- [ ] **Step 8: Write edge detection tests**

Add to the `tests` module:

```rust
    #[test]
    fn edges_detected_at_sharp_boundary() {
        // Left half white, right half black — vertical edge in the middle
        let mut frame = RawFrame::new(8, 4);
        for y in 0..4u32 {
            for x in 0..8u32 {
                let idx = ((y * 8 + x) * 3) as usize;
                let val = if x < 4 { 255 } else { 0 };
                frame.data[idx] = val;
                frame.data[idx + 1] = val;
                frame.data[idx + 2] = val;
            }
        }
        let original = frame.data.clone();
        let mut gray = vec![0u8; 32];
        let mut edges = vec![0u8; 32];
        detect_and_overlay_edges(&mut frame, &mut gray, &mut edges, 8, 4, 30, 80);
        // Pixels near column 3-4 boundary should be darkened
        let mid_pixel = frame.data[((1 * 8 + 4) * 3) as usize]; // row 1, col 4
        assert!(mid_pixel < original[((1 * 8 + 4) * 3) as usize],
            "Edge pixel should be darkened");
    }

    #[test]
    fn no_edges_on_uniform_frame() {
        let mut frame = RawFrame::filled(8, 8, 128, 128, 128);
        let original = frame.data.clone();
        let mut gray = vec![0u8; 64];
        let mut edges = vec![0u8; 64];
        detect_and_overlay_edges(&mut frame, &mut gray, &mut edges, 8, 8, 30, 80);
        assert_eq!(frame.data, original, "Uniform frame should have no edges");
    }
```

- [ ] **Step 9: Implement detect_and_overlay_edges**

```rust
/// Sobel edge detection + dark overlay on the frame.
/// `gray` and `edges` are pre-allocated scratch buffers (width * height each).
pub fn detect_and_overlay_edges(
    frame: &mut RawFrame,
    gray: &mut [u8],
    edges: &mut [u8],
    width: u32,
    height: u32,
    threshold: u8,
    darkness: u8,
) {
    let w = width as usize;
    let h = height as usize;

    // RGB → grayscale (BT.601 weights)
    for i in 0..(w * h) {
        let r = frame.data[i * 3] as u16;
        let g = frame.data[i * 3 + 1] as u16;
        let b = frame.data[i * 3 + 2] as u16;
        gray[i] = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
    }

    // Clear edges
    edges.iter_mut().for_each(|e| *e = 0);

    // Sobel (skip border pixels)
    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let g = |dy: i32, dx: i32| -> i16 {
                gray[((y as i32 + dy) as usize) * w + (x as i32 + dx) as usize] as i16
            };
            let gx = -g(-1, -1) + g(-1, 1) - 2 * g(0, -1) + 2 * g(0, 1) - g(1, -1) + g(1, 1);
            let gy = -g(-1, -1) - 2 * g(-1, 0) - g(-1, 1) + g(1, -1) + 2 * g(1, 0) + g(1, 1);
            let mag = ((gx.unsigned_abs() + gy.unsigned_abs()) / 2).min(255) as u8;
            edges[y * w + x] = if mag > threshold { 255 } else { 0 };
        }
    }

    // Overlay dark edges
    for i in 0..(w * h) {
        if edges[i] > 0 {
            frame.data[i * 3] = frame.data[i * 3].saturating_sub(darkness);
            frame.data[i * 3 + 1] = frame.data[i * 3 + 1].saturating_sub(darkness);
            frame.data[i * 3 + 2] = frame.data[i * 3 + 2].saturating_sub(darkness);
        }
    }
}
```

- [ ] **Step 10: Run edge detection tests**

```bash
cd livestream-morphing-rs && cargo test effects::tests::edges
```

Expected: PASS.

- [ ] **Step 11: Write texture blend test**

```rust
    #[test]
    fn texture_blend_darkens_pixels() {
        let mut frame = RawFrame::filled(4, 4, 200, 200, 200);
        // Texture with value 128 (mid-grey) at 50% strength should darken
        let texture = vec![128u8; 16];
        blend_texture(&mut frame, &texture, 0.5);
        // 200 * (1.0 - 0.5 + 0.5 * 128/255) ≈ 200 * 0.75 = 150
        assert!(frame.data[0] < 200, "Should be darkened, got {}", frame.data[0]);
        assert!(frame.data[0] > 100, "Shouldn't be too dark, got {}", frame.data[0]);
    }

    #[test]
    fn texture_blend_zero_strength_is_identity() {
        let mut frame = RawFrame::filled(2, 2, 100, 100, 100);
        let texture = vec![0u8; 4];
        let original = frame.data.clone();
        blend_texture(&mut frame, &texture, 0.0);
        assert_eq!(frame.data, original);
    }
```

- [ ] **Step 12: Implement blend_texture and generate_canvas_texture**

```rust
/// Multiply-blend a grayscale texture over the frame.
pub fn blend_texture(frame: &mut RawFrame, texture: &[u8], strength: f32) {
    let pixel_count = (frame.width * frame.height) as usize;
    for i in 0..pixel_count {
        let tex = texture[i % texture.len()] as f32 / 255.0;
        let factor = 1.0 - strength + strength * tex;
        for c in 0..3 {
            let idx = i * 3 + c;
            frame.data[idx] = (frame.data[idx] as f32 * factor).clamp(0.0, 255.0) as u8;
        }
    }
}

/// Generate a deterministic canvas-weave texture (grayscale).
pub fn generate_canvas_texture(width: u32, height: u32) -> Vec<u8> {
    let mut texture = vec![0u8; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            // Hash-based noise
            let h = x.wrapping_mul(374761393)
                .wrapping_add(y.wrapping_mul(668265263))
                .wrapping_mul(1274126177);
            let noise = ((h >> 24) & 0x1F) as u8; // 0-31 range
            // Canvas weave pattern
            let weave: u8 = if (x % 4 < 2) ^ (y % 4 < 2) { 10 } else { 0 };
            texture[(y * width + x) as usize] = 200u8.wrapping_add(noise).wrapping_add(weave);
        }
    }
    texture
}
```

- [ ] **Step 13: Run texture tests**

```bash
cd livestream-morphing-rs && cargo test effects::tests::texture
```

Expected: PASS.

- [ ] **Step 14: Implement downsample/upsample helpers**

```rust
/// Downsample a frame by 2x using nearest-neighbor sampling.
pub fn downsample_2x(src: &RawFrame) -> RawFrame {
    let dw = src.width / 2;
    let dh = src.height / 2;
    let mut dst = RawFrame::new(dw, dh);
    for y in 0..dh {
        for x in 0..dw {
            let si = ((y * 2 * src.width + x * 2) * 3) as usize;
            let di = ((y * dw + x) * 3) as usize;
            dst.data[di..di + 3].copy_from_slice(&src.data[si..si + 3]);
        }
    }
    dst
}

/// Upsample a frame by 2x using nearest-neighbor (preserves hard painterly edges).
pub fn upsample_2x(src: &RawFrame, dst_w: u32, dst_h: u32) -> RawFrame {
    let mut dst = RawFrame::new(dst_w, dst_h);
    for y in 0..dst_h {
        for x in 0..dst_w {
            let sx = (x / 2).min(src.width - 1);
            let sy = (y / 2).min(src.height - 1);
            let si = ((sy * src.width + sx) * 3) as usize;
            let di = ((y * dst_w + x) * 3) as usize;
            dst.data[di..di + 3].copy_from_slice(&src.data[si..si + 3]);
        }
    }
    dst
}
```

- [ ] **Step 15: Commit**

```bash
cd livestream-morphing-rs && git add src/effects.rs && git commit -m "feat: add effects module with quantization, distortion, edge detection, texture"
```

---

### Task 4: FrameProcessor Compositor

**Files:**
- Modify: `livestream-morphing-rs/src/effects.rs`

- [ ] **Step 1: Write FrameProcessor test**

Add to the `tests` module in `effects.rs`:

```rust
    #[test]
    fn frame_processor_changes_frame() {
        let mut processor = FrameProcessor::new(8, 8);
        // Create a gradient frame
        let mut frame = RawFrame::new(8, 8);
        for y in 0..8u32 {
            for x in 0..8u32 {
                let idx = ((y * 8 + x) * 3) as usize;
                frame.data[idx] = (x * 32) as u8;
                frame.data[idx + 1] = (y * 32) as u8;
                frame.data[idx + 2] = 128;
            }
        }
        let original = frame.data.clone();
        processor.process_frame(&mut frame, 0);
        assert_ne!(frame.data, original, "Processing should change the frame");
    }
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd livestream-morphing-rs && cargo test effects::tests::frame_processor
```

Expected: FAIL — `FrameProcessor` not defined.

- [ ] **Step 3: Implement FrameProcessor**

```rust
/// Pre-allocated processor that applies all 4 effect passes.
pub struct FrameProcessor {
    width: u32,
    height: u32,
    scratch: RawFrame,
    grayscale: Vec<u8>,
    edges: Vec<u8>,
    texture: Vec<u8>,
    // Tunable parameters
    pub quantize_levels: u8,
    pub distortion_amplitude: f32,
    pub distortion_frequency: f32,
    pub distortion_cycle: u32,
    pub edge_threshold: u8,
    pub edge_darkness: u8,
    pub texture_strength: f32,
}

impl FrameProcessor {
    pub fn new(width: u32, height: u32) -> Self {
        let pixel_count = (width * height) as usize;
        Self {
            width,
            height,
            scratch: RawFrame::new(width, height),
            grayscale: vec![0u8; pixel_count],
            edges: vec![0u8; pixel_count],
            texture: generate_canvas_texture(width, height),
            quantize_levels: 10,
            distortion_amplitude: 0.02,
            distortion_frequency: 12.0,
            distortion_cycle: 180,
            edge_threshold: 30,
            edge_darkness: 80,
            texture_strength: 0.15,
        }
    }

    /// Apply all 4 effect passes to a frame in-place.
    pub fn process_frame(&mut self, frame: &mut RawFrame, frame_number: u32) {
        // Pass 1: Psychedelic distortion (src → scratch, then swap)
        apply_distortion(
            frame,
            &mut self.scratch,
            frame_number,
            self.distortion_amplitude,
            self.distortion_frequency,
            self.distortion_cycle,
        );
        std::mem::swap(&mut frame.data, &mut self.scratch.data);

        // Pass 2: Color quantization
        quantize(frame, self.quantize_levels);

        // Pass 3: Edge detection + overlay
        detect_and_overlay_edges(
            frame,
            &mut self.grayscale,
            &mut self.edges,
            self.width,
            self.height,
            self.edge_threshold,
            self.edge_darkness,
        );

        // Pass 4: Canvas texture blend
        blend_texture(frame, &self.texture, self.texture_strength);
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd livestream-morphing-rs && cargo test effects
```

Expected: all effects tests PASS.

- [ ] **Step 5: Commit**

```bash
cd livestream-morphing-rs && git add src/effects.rs && git commit -m "feat: add FrameProcessor compositor combining all effect passes"
```

---

### Task 5: HLS Buffer Module

**Files:**
- Modify: `livestream-morphing-rs/src/hls.rs`

- [ ] **Step 1: Write HLS buffer tests**

```rust
use std::collections::VecDeque;

pub struct Segment {
    pub id: String,
    pub data: Vec<u8>,
    pub duration: f32,
}

pub struct HlsBuffer {
    segments: VecDeque<Segment>,
    max_segments: usize,
    media_sequence: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_retrieve_segment() {
        let mut buf = HlsBuffer::new(10);
        buf.push_segment("001".into(), vec![1, 2, 3]);
        assert_eq!(buf.get_segment("001"), Some([1u8, 2, 3].as_slice()));
        assert_eq!(buf.segment_count(), 1);
    }

    #[test]
    fn evicts_oldest_when_full() {
        let mut buf = HlsBuffer::new(3);
        buf.push_segment("1".into(), vec![1]);
        buf.push_segment("2".into(), vec![2]);
        buf.push_segment("3".into(), vec![3]);
        buf.push_segment("4".into(), vec![4]);
        assert_eq!(buf.get_segment("1"), None);
        assert_eq!(buf.get_segment("4"), Some([4u8].as_slice()));
        assert_eq!(buf.segment_count(), 3);
    }

    #[test]
    fn media_sequence_increments_on_eviction() {
        let mut buf = HlsBuffer::new(2);
        buf.push_segment("1".into(), vec![]);
        buf.push_segment("2".into(), vec![]);
        assert_eq!(buf.media_sequence(), 0);
        buf.push_segment("3".into(), vec![]);
        assert_eq!(buf.media_sequence(), 1);
        buf.push_segment("4".into(), vec![]);
        assert_eq!(buf.media_sequence(), 2);
    }

    #[test]
    fn playlist_format() {
        let mut buf = HlsBuffer::new(10);
        buf.push_segment("100".into(), vec![]);
        buf.push_segment("101".into(), vec![]);
        let playlist = buf.generate_playlist();
        assert!(playlist.contains("#EXTM3U"));
        assert!(playlist.contains("#EXT-X-MEDIA-SEQUENCE:0"));
        assert!(playlist.contains("/api/segments/100.ts"));
        assert!(playlist.contains("/api/segments/101.ts"));
    }

    #[test]
    fn empty_playlist() {
        let buf = HlsBuffer::new(10);
        let playlist = buf.generate_playlist();
        assert!(playlist.contains("#EXTM3U"));
        assert!(!playlist.contains("#EXTINF"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd livestream-morphing-rs && cargo test hls
```

Expected: FAIL — methods not implemented.

- [ ] **Step 3: Implement HlsBuffer**

```rust
use std::collections::VecDeque;

pub struct Segment {
    pub id: String,
    pub data: Vec<u8>,
    pub duration: f32,
}

pub struct HlsBuffer {
    segments: VecDeque<Segment>,
    max_segments: usize,
    sequence: u64,
}

impl HlsBuffer {
    pub fn new(max_segments: usize) -> Self {
        Self {
            segments: VecDeque::new(),
            max_segments,
            sequence: 0,
        }
    }

    pub fn push_segment(&mut self, id: String, data: Vec<u8>) {
        if self.segments.len() >= self.max_segments {
            self.segments.pop_front();
            self.sequence += 1;
        }
        self.segments.push_back(Segment {
            id,
            data,
            duration: 6.0,
        });
    }

    pub fn get_segment(&self, id: &str) -> Option<&[u8]> {
        self.segments
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.data.as_slice())
    }

    pub fn generate_playlist(&self) -> String {
        let mut m3u8 = format!(
            "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:6\n#EXT-X-MEDIA-SEQUENCE:{}\n",
            self.sequence
        );
        for seg in &self.segments {
            m3u8.push_str(&format!(
                "#EXTINF:{:.1},\n/api/segments/{}.ts\n",
                seg.duration, seg.id
            ));
        }
        m3u8
    }

    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    pub fn media_sequence(&self) -> u64 {
        self.sequence
    }

    pub fn clear(&mut self) {
        self.segments.clear();
        self.sequence = 0;
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd livestream-morphing-rs && cargo test hls
```

Expected: all 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
cd livestream-morphing-rs && git add src/hls.rs && git commit -m "feat: add HLS ring buffer with M3U8 playlist generation"
```

---

### Task 6: Stream Source Module

**Files:**
- Modify: `livestream-morphing-rs/src/stream_source.rs`

- [ ] **Step 1: Write M3U8 parsing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_segment_id_from_m3u8() {
        let m3u8 = "\
#EXTM3U
#EXT-X-VERSION:3
#EXT-X-TARGETDURATION:6
#EXT-X-MEDIA-SEQUENCE:54321
#EXTINF:6.006,
media_w1715000000_99887.ts
";
        let id = extract_segment_id(m3u8);
        assert_eq!(id, Some("99887".to_string()));
    }

    #[test]
    fn parse_returns_none_for_empty_playlist() {
        let m3u8 = "#EXTM3U\n#EXT-X-VERSION:3\n";
        assert_eq!(extract_segment_id(m3u8), None);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd livestream-morphing-rs && cargo test stream_source
```

Expected: FAIL — `extract_segment_id` not defined.

- [ ] **Step 3: Implement stream_source**

```rust
use reqwest::Client;

const STREAM_BASE_URL: &str =
    "https://videos-3.earthcam.com/fecnetwork/AbbeyRoadHD1.flv/chunklist_w";

fn earthcam_headers() -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Origin", "https://www.abbeyroad.com".parse().unwrap());
    headers.insert("Referer", "https://www.abbeyroad.com/".parse().unwrap());
    headers.insert(
        "User-Agent",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"
            .parse()
            .unwrap(),
    );
    headers
}

/// Extract the segment ID from the first .ts URI in an M3U8 playlist.
/// URI format: `media_w{timestamp}_{segment_id}.ts`
pub fn extract_segment_id(m3u8_text: &str) -> Option<String> {
    m3u8_text
        .lines()
        .find(|line| line.ends_with(".ts"))
        .and_then(|line| {
            let name = line.trim();
            // Extract segment ID: everything between last '_' and '.ts'
            let without_ext = name.strip_suffix(".ts")?;
            let id = without_ext.rsplit('_').next()?;
            Some(id.to_string())
        })
}

pub struct StreamSource {
    client: Client,
    recent_ids: Vec<String>,
}

impl StreamSource {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            recent_ids: Vec::new(),
        }
    }

    /// Fetch the latest segment ID from the Abbey Road stream.
    /// Returns `None` if the segment was already seen or fetch fails.
    pub async fn fetch_latest_segment_id(&mut self) -> Option<String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let url = format!("{STREAM_BASE_URL}{timestamp}.m3u8");

        let resp = self
            .client
            .get(&url)
            .headers(earthcam_headers())
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .ok()?;

        let text = resp.text().await.ok()?;
        let id = extract_segment_id(&text)?;

        if self.recent_ids.contains(&id) {
            return None;
        }

        self.recent_ids.push(id.clone());
        if self.recent_ids.len() > 20 {
            self.recent_ids.remove(0);
        }

        Some(id)
    }

    /// Download a .ts segment by ID. Retries up to 3 times.
    pub async fn download_segment(&self, segment_id: &str) -> Option<Vec<u8>> {
        let base_url = STREAM_BASE_URL.replace("/chunklist_w", "/media_w");

        for attempt in 0..3 {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let url = format!("{base_url}{timestamp}_{segment_id}.ts");

            match self
                .client
                .get(&url)
                .headers(earthcam_headers())
                .timeout(std::time::Duration::from_secs(30))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(bytes) = resp.bytes().await {
                        tracing::info!(
                            segment_id,
                            size_mb = bytes.len() as f64 / 1_048_576.0,
                            "Downloaded segment"
                        );
                        return Some(bytes.to_vec());
                    }
                }
                Ok(resp) => {
                    tracing::warn!(segment_id, attempt, status = %resp.status(), "Download failed");
                }
                Err(e) => {
                    tracing::warn!(segment_id, attempt, error = %e, "Download error");
                }
            }

            if attempt < 2 {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }

        tracing::error!(segment_id, "Failed to download after 3 attempts");
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_segment_id_from_m3u8() {
        let m3u8 = "\
#EXTM3U
#EXT-X-VERSION:3
#EXT-X-TARGETDURATION:6
#EXT-X-MEDIA-SEQUENCE:54321
#EXTINF:6.006,
media_w1715000000_99887.ts
";
        let id = extract_segment_id(m3u8);
        assert_eq!(id, Some("99887".to_string()));
    }

    #[test]
    fn parse_returns_none_for_empty_playlist() {
        let m3u8 = "#EXTM3U\n#EXT-X-VERSION:3\n";
        assert_eq!(extract_segment_id(m3u8), None);
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd livestream-morphing-rs && cargo test stream_source
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd livestream-morphing-rs && git add src/stream_source.rs && git commit -m "feat: add stream source with Abbey Road M3U8 parsing and segment fetching"
```

---

### Task 7: Codec Module

**Files:**
- Modify: `livestream-morphing-rs/src/codec.rs`

Note: This module uses ffmpeg-next which links against system FFmpeg. Ensure FFmpeg dev libraries are installed: `apt-get install libavcodec-dev libavformat-dev libavutil-dev libswscale-dev pkg-config` (Debian/Ubuntu) or `brew install ffmpeg` (macOS).

- [ ] **Step 1: Implement decode_segment**

```rust
extern crate ffmpeg_next as ffmpeg;

use ffmpeg::format::{input, Pixel};
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context as ScalingContext, flag::Flags};
use ffmpeg::util::frame::video::Video;
use std::io::Write;
use tempfile::NamedTempFile;

use crate::effects::RawFrame;

/// Initialize FFmpeg. Call once at program start.
pub fn init() {
    ffmpeg::init().expect("Failed to initialize FFmpeg");
}

/// Decode a .ts segment from raw bytes into RGB24 frames.
pub fn decode_segment(ts_bytes: &[u8]) -> Result<Vec<RawFrame>, Box<dyn std::error::Error + Send + Sync>> {
    // Write to temp file (ffmpeg-next requires a file path for input)
    let mut tmp = NamedTempFile::new()?;
    tmp.write_all(ts_bytes)?;
    tmp.flush()?;

    let mut ictx = input(tmp.path())?;
    let stream = ictx
        .streams()
        .best(Type::Video)
        .ok_or("No video stream found")?;
    let stream_index = stream.index();

    let decoder_ctx = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
    let mut decoder = decoder_ctx.decoder().video()?;

    let mut scaler = ScalingContext::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::RGB24,
        decoder.width(),
        decoder.height(),
        Flags::BILINEAR,
    )?;

    let width = decoder.width();
    let height = decoder.height();
    let mut frames = Vec::new();

    let mut receive_frames = |decoder: &mut ffmpeg::decoder::Video| -> Result<(), ffmpeg::Error> {
        let mut decoded = Video::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            let mut rgb = Video::empty();
            scaler.run(&decoded, &mut rgb)?;

            // Copy RGB data, accounting for stride alignment
            let stride = rgb.stride(0);
            let row_bytes = (width * 3) as usize;
            let mut data = Vec::with_capacity((width * height * 3) as usize);
            for y in 0..height as usize {
                let offset = y * stride;
                data.extend_from_slice(&rgb.data(0)[offset..offset + row_bytes]);
            }

            frames.push(RawFrame { data, width, height });
        }
        Ok(())
    };

    for (stream, packet) in ictx.packets() {
        if stream.index() == stream_index {
            decoder.send_packet(&packet)?;
            receive_frames(&mut decoder)?;
        }
    }
    decoder.send_eof()?;
    receive_frames(&mut decoder)?;

    tracing::info!(frame_count = frames.len(), width, height, "Decoded segment");
    Ok(frames)
}
```

- [ ] **Step 2: Implement encode_segment**

```rust
/// Encode RGB24 frames into an H.264 MPEG-TS segment.
pub fn encode_segment(
    frames: &[RawFrame],
    fps: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    if frames.is_empty() {
        return Err("No frames to encode".into());
    }

    let width = frames[0].width;
    let height = frames[0].height;

    let tmp = NamedTempFile::new()?;
    let tmp_path = tmp.path().to_owned();

    // Create output context
    let mut octx = ffmpeg::format::output_as(&tmp_path, "mpegts")?;

    // Find H.264 encoder
    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::H264)
        .ok_or("H264 encoder not found — install libx264")?;

    let mut ost = octx.add_stream(codec)?;

    // Configure encoder
    let mut encoder = ffmpeg::codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()?;

    encoder.set_width(width);
    encoder.set_height(height);
    encoder.set_format(Pixel::YUV420P);
    encoder.set_frame_rate(Some(ffmpeg::Rational(fps as i32, 1)));
    encoder.set_time_base(ffmpeg::Rational(1, fps as i32));

    if octx
        .format()
        .flags()
        .contains(ffmpeg::format::Flags::GLOBAL_HEADER)
    {
        encoder.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
    }

    let mut x264_opts = ffmpeg::Dictionary::new();
    x264_opts.set("preset", "ultrafast");
    x264_opts.set("crf", "25");

    let mut encoder = encoder.open_with(x264_opts)?;
    ost.set_parameters(&encoder);

    octx.write_header()?;

    // Scaler: RGB24 → YUV420P
    let mut scaler = ScalingContext::get(
        Pixel::RGB24,
        width,
        height,
        Pixel::YUV420P,
        width,
        height,
        Flags::BILINEAR,
    )?;

    let row_bytes = (width * 3) as usize;

    for (i, frame) in frames.iter().enumerate() {
        let mut rgb = Video::new(Pixel::RGB24, width, height);
        // Copy data accounting for stride
        let stride = rgb.stride(0);
        for y in 0..height as usize {
            let src_offset = y * row_bytes;
            let dst_offset = y * stride;
            rgb.data_mut(0)[dst_offset..dst_offset + row_bytes]
                .copy_from_slice(&frame.data[src_offset..src_offset + row_bytes]);
        }

        let mut yuv = Video::empty();
        scaler.run(&rgb, &mut yuv)?;
        yuv.set_pts(Some(i as i64));

        encoder.send_frame(&yuv)?;

        let mut encoded_packet = ffmpeg::Packet::empty();
        while encoder.receive_packet(&mut encoded_packet).is_ok() {
            encoded_packet.set_stream(0);
            encoded_packet.write_interleaved(&mut octx)?;
        }
    }

    // Flush encoder
    encoder.send_eof()?;
    let mut encoded_packet = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut encoded_packet).is_ok() {
        encoded_packet.set_stream(0);
        encoded_packet.write_interleaved(&mut octx)?;
    }

    octx.write_trailer()?;
    drop(octx); // Close output before reading

    let output_bytes = std::fs::read(&tmp_path)?;
    tracing::info!(
        frame_count = frames.len(),
        size_kb = output_bytes.len() / 1024,
        "Encoded segment"
    );
    Ok(output_bytes)
}
```

- [ ] **Step 3: Write round-trip test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_encode_decode() {
        init();

        // Create 5 simple test frames (solid colors)
        let frames: Vec<RawFrame> = (0..5)
            .map(|i| {
                let val = (i * 50) as u8;
                RawFrame::filled(64, 48, val, val, val)
            })
            .collect();

        // Encode
        let ts_bytes = encode_segment(&frames, 30).expect("encode failed");
        assert!(!ts_bytes.is_empty(), "Encoded bytes should not be empty");

        // Decode
        let decoded = decode_segment(&ts_bytes).expect("decode failed");
        assert_eq!(decoded.len(), 5, "Should get back 5 frames");
        assert_eq!(decoded[0].width, 64);
        assert_eq!(decoded[0].height, 48);
    }
}
```

- [ ] **Step 4: Run test**

```bash
cd livestream-morphing-rs && cargo test codec::tests::roundtrip -- --nocapture
```

Expected: PASS. Note: requires FFmpeg + libx264 installed on the system.

- [ ] **Step 5: Commit**

```bash
cd livestream-morphing-rs && git add src/codec.rs && git commit -m "feat: add codec module with ffmpeg-next H.264 decode/encode"
```

---

### Task 8: Pipeline Orchestrator

**Files:**
- Modify: `livestream-morphing-rs/src/pipeline.rs`

- [ ] **Step 1: Implement the pipeline module**

```rust
use std::sync::Arc;
use tokio::sync::{watch, RwLock};

use crate::codec;
use crate::effects::{downsample_2x, upsample_2x, FrameProcessor};
use crate::hls::HlsBuffer;
use crate::stream_source::StreamSource;

/// Shared state between the pipeline and HTTP server.
pub struct AppState {
    pub hls_buffer: RwLock<HlsBuffer>,
    pub pipeline_active: watch::Sender<bool>,
    pub last_client_request: std::sync::atomic::AtomicU64,
}

impl AppState {
    pub fn new() -> (Arc<Self>, watch::Receiver<bool>) {
        let (tx, rx) = watch::channel(false);
        let state = Arc::new(Self {
            hls_buffer: RwLock::new(HlsBuffer::new(10)),
            pipeline_active: tx,
            last_client_request: std::sync::atomic::AtomicU64::new(0),
        });
        (state, rx)
    }

    /// Record a client request and activate the pipeline if needed.
    pub fn touch(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_client_request
            .store(now, std::sync::atomic::Ordering::Relaxed);
        // Signal pipeline to start
        let _ = self.pipeline_active.send(true);
    }

    /// Seconds since last client request.
    pub fn idle_seconds(&self) -> u64 {
        let last = self
            .last_client_request
            .load(std::sync::atomic::Ordering::Relaxed);
        if last == 0 {
            return u64::MAX;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now.saturating_sub(last)
    }
}

/// Main pipeline loop. Runs until the shutdown receiver signals.
pub async fn run(state: Arc<AppState>, mut active: watch::Receiver<bool>) {
    tracing::info!("Pipeline waiting for first client...");

    // Wait for first client to activate
    loop {
        if *active.borrow_and_update() {
            break;
        }
        if active.changed().await.is_err() {
            return; // Sender dropped
        }
    }

    tracing::info!("Pipeline activated!");
    let mut source = StreamSource::new();
    let idle_timeout = std::time::Duration::from_secs(300); // 5 minutes

    loop {
        // Check idle timeout
        if state.idle_seconds() > idle_timeout.as_secs() {
            tracing::info!("No clients for 5 minutes, pipeline going idle");
            let _ = state.pipeline_active.send(false);
            state.hls_buffer.write().await.clear();

            // Wait for reactivation
            loop {
                if *active.borrow_and_update() {
                    break;
                }
                if active.changed().await.is_err() {
                    return;
                }
            }
            tracing::info!("Pipeline reactivated!");
            source = StreamSource::new();
        }

        // Fetch latest segment
        let segment_id = match source.fetch_latest_segment_id().await {
            Some(id) => id,
            None => {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        // Download
        let ts_bytes = match source.download_segment(&segment_id).await {
            Some(bytes) => bytes,
            None => {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        // Process in blocking thread (CPU-bound)
        let seg_id = segment_id.clone();
        let processed = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
            let mut frames = codec::decode_segment(&ts_bytes)?;

            if frames.is_empty() {
                return Err("No frames decoded".into());
            }

            let orig_w = frames[0].width;
            let orig_h = frames[0].height;

            // Downsample to half res for processing
            let mut half_frames: Vec<_> = frames.iter().map(|f| downsample_2x(f)).collect();
            let half_w = half_frames[0].width;
            let half_h = half_frames[0].height;

            // Process each frame
            let mut processor = FrameProcessor::new(half_w, half_h);

            // Adjust edge darkness based on London time
            let (edge_color, _bg) = crate::time_color::get_colors_now();
            // Dark edges during day (light bg), light edges at night (dark bg)
            processor.edge_darkness = if edge_color == (0, 0, 0) { 100 } else { 40 };

            for (i, frame) in half_frames.iter_mut().enumerate() {
                processor.process_frame(frame, i as u32);
            }

            // Upsample back to original size
            let full_frames: Vec<_> = half_frames
                .iter()
                .map(|f| upsample_2x(f, orig_w, orig_h))
                .collect();

            // Encode
            let encoded = codec::encode_segment(&full_frames, 30)?;
            Ok(encoded)
        })
        .await;

        match processed {
            Ok(Ok(encoded)) => {
                tracing::info!(segment_id, size_kb = encoded.len() / 1024, "Segment processed");
                state.hls_buffer.write().await.push_segment(segment_id, encoded);
            }
            Ok(Err(e)) => {
                tracing::error!(segment_id = seg_id, error = %e, "Processing failed");
            }
            Err(e) => {
                tracing::error!(segment_id = seg_id, error = %e, "Task panicked");
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cd livestream-morphing-rs && cargo check
```

Expected: compiles (some warnings about unused imports in other modules).

- [ ] **Step 3: Commit**

```bash
cd livestream-morphing-rs && git add src/pipeline.rs && git commit -m "feat: add pipeline orchestrator with on-demand activation and idle timeout"
```

---

### Task 9: HTTP Server

**Files:**
- Modify: `livestream-morphing-rs/src/server.rs`

- [ ] **Step 1: Implement the axum server**

```rust
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::pipeline::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/stream", get(stream_playlist))
        .route("/api/segments/{segment_id}.ts", get(get_segment))
        .route("/health", get(health))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn stream_playlist(State(state): State<Arc<AppState>>) -> Response {
    // Record client activity and activate pipeline
    state.touch();

    let buf = state.hls_buffer.read().await;
    let playlist = buf.generate_playlist();

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/x-mpegURL"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        playlist,
    )
        .into_response()
}

async fn get_segment(
    State(state): State<Arc<AppState>>,
    Path(segment_id): Path<String>,
) -> Response {
    state.touch();

    // Strip .ts extension if present
    let id = segment_id.strip_suffix(".ts").unwrap_or(&segment_id);

    let buf = state.hls_buffer.read().await;
    match buf.get_segment(id) {
        Some(data) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "video/mp2t"),
                (header::CACHE_CONTROL, "max-age=3600"),
            ],
            data.to_vec(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn health() -> &'static str {
    "ok"
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cd livestream-morphing-rs && cargo check
```

Expected: compiles.

- [ ] **Step 3: Commit**

```bash
cd livestream-morphing-rs && git add src/server.rs && git commit -m "feat: add axum HTTP server with HLS streaming routes and CORS"
```

---

### Task 10: Main Entry Point

**Files:**
- Modify: `livestream-morphing-rs/src/main.rs`

- [ ] **Step 1: Wire everything together in main.rs**

```rust
mod codec;
mod effects;
mod hls;
mod pipeline;
mod server;
mod stream_source;
mod time_color;

use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "livestream_morphing_rs=info".into()),
        )
        .init();

    // Initialize FFmpeg
    codec::init();

    // Create shared state
    let (state, active_rx) = pipeline::AppState::new();

    // Start pipeline in background
    let pipeline_state = state.clone();
    tokio::spawn(async move {
        pipeline::run(pipeline_state, active_rx).await;
    });

    // Start HTTP server
    let app = server::router(state);
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!(%addr, "Server starting");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

- [ ] **Step 2: Verify the full project compiles**

```bash
cd livestream-morphing-rs && cargo build
```

Expected: compiles with no errors. Warnings about unused code in stream_source are OK.

- [ ] **Step 3: Run all tests**

```bash
cd livestream-morphing-rs && cargo test
```

Expected: all unit tests pass. The codec round-trip test requires FFmpeg installed.

- [ ] **Step 4: Commit**

```bash
cd livestream-morphing-rs && git add src/main.rs && git commit -m "feat: wire up main entry point with server, pipeline, and logging"
```

---

### Task 11: Dockerfile & Deployment

**Files:**
- Create: `livestream-morphing-rs/Dockerfile`
- Create: `livestream-morphing-rs/.dockerignore`

- [ ] **Step 1: Write Dockerfile**

```dockerfile
# Build stage
FROM rust:1.83-bookworm AS builder

RUN apt-get update && apt-get install -y \
    libavcodec-dev \
    libavformat-dev \
    libavutil-dev \
    libswscale-dev \
    libavfilter-dev \
    libavdevice-dev \
    pkg-config \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
# Create dummy main to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

COPY src/ src/
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libavcodec60 \
    libavformat60 \
    libavutil58 \
    libswscale7 \
    libx264-164 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/livestream-morphing-rs /usr/local/bin/morphing

EXPOSE 8080
ENV PORT=8080
CMD ["morphing"]
```

- [ ] **Step 2: Write .dockerignore**

```
target/
.git/
docs/
tests/
*.md
```

- [ ] **Step 3: Test Docker build**

```bash
cd livestream-morphing-rs && docker build -t morphing .
```

Expected: builds successfully. Final image should be ~100-150MB.

- [ ] **Step 4: Test Docker run locally**

```bash
docker run -p 8080:8080 morphing
```

Expected: server starts, logs "Server starting" and "Pipeline waiting for first client."

Verify with:
```bash
curl http://localhost:8080/health
# Expected: "ok"

curl http://localhost:8080/api/stream
# Expected: empty M3U8 playlist (pipeline hasn't processed segments yet)
```

- [ ] **Step 5: Commit**

```bash
cd livestream-morphing-rs && git add Dockerfile .dockerignore && git commit -m "feat: add multi-stage Dockerfile for Railway deployment"
```

- [ ] **Step 6: Deploy to Railway**

1. Push the repository to GitHub.
2. In Railway dashboard: New Project → Deploy from GitHub Repo.
3. Railway auto-detects the Dockerfile.
4. Set "Root Directory" to `livestream-morphing-rs` if deploying from the monorepo.
5. In Settings → Networking: add a public domain.
6. In Settings → Deploy: enable "Sleep after inactivity" (5 minutes).
7. Verify: `curl https://your-app.railway.app/health` returns `"ok"`.

---

## Post-Implementation Notes

- **Canvas texture:** The procedural texture is a placeholder. Replace `generate_canvas_texture()` with a real canvas texture PNG loaded via `include_bytes!("../textures/canvas.png")` and decoded with the `image` crate for a more authentic look.
- **Frame skipping:** If processing can't keep up on shared CPU, set `process_every_nth = 2` in the pipeline to halve effects work (duplicate frames).
- **Effect tuning:** All effect parameters are fields on `FrameProcessor`. Expose them via environment variables or a `/api/config` endpoint for live tuning.
- **Monitoring:** Add `/api/status` endpoint returning `{ segment_count, avg_processing_time, pipeline_active }` for debugging.
