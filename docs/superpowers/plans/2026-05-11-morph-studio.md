# Morph Studio Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a web-based creative studio for real-time visual effect experimentation on the Abbey Road livestream.

**Architecture:** Rust backend with an `Effect` trait + `inventory`-based registration macro, dynamic pipeline engine, REST API. React + TypeScript + Vite + Tailwind frontend with drag-and-drop pipeline editor, parameter sliders, HLS live preview, and preset management.

**Tech Stack:** Rust (Axum, serde, uuid, inventory, toml), React 18 (TypeScript, Vite, Tailwind, hls.js, @dnd-kit)

**Spec:** `docs/superpowers/specs/2026-05-11-morph-studio-design.md`

---

## File Map

### Rust Backend — New Files

| File | Responsibility |
|------|---------------|
| `src/registry.rs` | `Effect` trait, `ParamDef`, `ParamValues`, `FrameCtx`, `EffectFactory`, `register_effect!` macro |
| `src/effects/mod.rs` | `RawFrame`, `downsample_2x`, `upsample_2x`, re-exports all effects |
| `src/effects/quantize.rs` | Quantize effect (refactored from effects.rs) |
| `src/effects/distortion.rs` | Distortion effect (refactored from effects.rs) |
| `src/effects/edges.rs` | Edge detection effect (refactored from effects.rs) |
| `src/effects/canvas_texture.rs` | Canvas texture effect (refactored from effects.rs) |
| `src/effects/blur.rs` | New Blur effect |
| `src/effects/color_shift.rs` | New Color Shift effect |
| `src/api.rs` | REST endpoints for effects, pipeline, presets |
| `presets/default.toml` | Default preset matching current pipeline |

### Rust Backend — Modified Files

| File | Changes |
|------|---------|
| `Cargo.toml` | Add serde, serde_json, uuid, inventory, toml dependencies |
| `src/main.rs` | Add `mod registry`, `mod api`, change `mod effects` to directory module |
| `src/pipeline.rs` | Replace `FrameProcessor` usage with dynamic `Pipeline`, add `Pipeline` to `AppState` |
| `src/server.rs` | Add API routes, update CORS for PUT/PATCH/POST/DELETE, serve static files |
| `src/effects.rs` | **Deleted** — replaced by `src/effects/` directory |

### React Frontend — New Files

| File | Responsibility |
|------|---------------|
| `studio/package.json` | Dependencies |
| `studio/vite.config.ts` | Vite config with API proxy |
| `studio/tailwind.config.ts` | Tailwind config |
| `studio/tsconfig.json` | TypeScript config |
| `studio/index.html` | HTML entry point |
| `studio/src/main.tsx` | React entry |
| `studio/src/App.tsx` | Layout shell |
| `studio/src/types/index.ts` | Shared TypeScript types |
| `studio/src/hooks/useEffects.ts` | GET /api/effects |
| `studio/src/hooks/usePipeline.ts` | Pipeline CRUD + debounced param updates |
| `studio/src/hooks/usePresets.ts` | Preset CRUD |
| `studio/src/components/Slider.tsx` | Reusable slider with label + value |
| `studio/src/components/ParamPanel.tsx` | Parameter sliders for selected effect |
| `studio/src/components/EffectLibrary.tsx` | Browsable list of registered effects |
| `studio/src/components/PipelineEditor.tsx` | Drag-and-drop pipeline ordering |
| `studio/src/components/VideoPlayer.tsx` | HLS.js live player |
| `studio/src/components/PresetBar.tsx` | Preset save/load UI |

---

### Task 1: Add Cargo Dependencies

**Files:**
- Modify: `livestream-morphing-rs/Cargo.toml`

- [ ] **Step 1: Add new dependencies**

In `livestream-morphing-rs/Cargo.toml`, add to `[dependencies]`:

```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
inventory = "0.3"
toml = "0.8"
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /Users/worthy/TestCode/livestream-morphing/livestream-morphing-rs && cargo check`
Expected: Compiles with no errors (new deps unused for now)

- [ ] **Step 3: Commit**

```bash
cd /Users/worthy/TestCode/livestream-morphing
git add livestream-morphing-rs/Cargo.toml
git commit -m "chore: add serde, uuid, inventory, toml dependencies"
```

---

### Task 2: Create Effect Registry Core

**Files:**
- Create: `livestream-morphing-rs/src/registry.rs`

- [ ] **Step 1: Write tests for ParamDef and ParamValues**

Create `src/registry.rs` with the types and tests:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parameter definition — describes a single tunable knob on an effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    pub id: &'static str,
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub step: f32,
}

impl ParamDef {
    pub const fn new(
        id: &'static str,
        name: &'static str,
        min: f32,
        max: f32,
        default: f32,
        step: f32,
    ) -> Self {
        Self { id, name, min, max, default, step }
    }
}

/// Runtime parameter values — maps param id → current value.
pub type ParamValues = HashMap<String, f32>;

/// Context passed to every effect on each frame.
pub struct FrameCtx {
    pub frame_number: u32,
    pub width: u32,
    pub height: u32,
}

/// The core effect trait. Each visual effect implements this.
pub trait Effect: Send {
    /// Unique identifier (e.g. "quantize").
    fn id(&self) -> &'static str;
    /// Human-readable name (e.g. "Color Quantize").
    fn name(&self) -> &'static str;
    /// Parameter definitions with ranges and defaults.
    fn params(&self) -> Vec<ParamDef>;
    /// Allocate scratch buffers for the given frame dimensions.
    fn init(&mut self, width: u32, height: u32);
    /// Apply the effect to a frame in-place.
    fn apply(&mut self, frame: &mut crate::effects::RawFrame, params: &ParamValues, ctx: &FrameCtx);
}

/// Factory for creating effect instances. Used by the registration macro.
pub struct EffectFactory(pub fn() -> Box<dyn Effect>);

inventory::collect!(EffectFactory);

/// Return a fresh instance of every registered effect.
pub fn all_effects() -> Vec<Box<dyn Effect>> {
    inventory::iter::<EffectFactory>
        .into_iter()
        .map(|f| (f.0)())
        .collect()
}

/// Build a ParamValues map with defaults from the given ParamDefs.
pub fn default_params(defs: &[ParamDef]) -> ParamValues {
    defs.iter().map(|p| (p.id.to_string(), p.default)).collect()
}

/// Registration macro. Place at the bottom of each effect file.
/// The effect type must implement Default.
#[macro_export]
macro_rules! register_effect {
    ($ty:ty) => {
        inventory::submit! {
            $crate::registry::EffectFactory(|| Box::new(<$ty>::default()))
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_def_new() {
        let p = ParamDef::new("radius", "Blur Radius", 0.0, 20.0, 3.0, 0.5);
        assert_eq!(p.id, "radius");
        assert_eq!(p.min, 0.0);
        assert_eq!(p.max, 20.0);
        assert_eq!(p.default, 3.0);
        assert_eq!(p.step, 0.5);
    }

    #[test]
    fn default_params_builds_map() {
        let defs = vec![
            ParamDef::new("a", "A", 0.0, 1.0, 0.5, 0.1),
            ParamDef::new("b", "B", 0.0, 10.0, 5.0, 1.0),
        ];
        let vals = default_params(&defs);
        assert_eq!(vals.get("a"), Some(&0.5));
        assert_eq!(vals.get("b"), Some(&5.0));
    }

    #[test]
    fn param_def_serializes_to_json() {
        let p = ParamDef::new("test", "Test", 0.0, 1.0, 0.5, 0.1);
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"id\":\"test\""));
        assert!(json.contains("\"default\":0.5"));
    }
}
```

- [ ] **Step 2: Add module to main.rs**

In `src/main.rs`, add `mod registry;` after the existing module declarations (but before the `use` statements):

```rust
mod codec;
mod effects;
mod hls;
mod pipeline;
mod registry;
mod server;
mod stream_source;
mod time_color;
```

- [ ] **Step 3: Run tests**

Run: `cd /Users/worthy/TestCode/livestream-morphing/livestream-morphing-rs && cargo test registry`
Expected: All 3 tests pass

- [ ] **Step 4: Commit**

```bash
cd /Users/worthy/TestCode/livestream-morphing
git add livestream-morphing-rs/src/registry.rs livestream-morphing-rs/src/main.rs
git commit -m "feat: add Effect trait, ParamDef, and registration macro"
```

---

### Task 3: Refactor Effects into Module Directory

**Files:**
- Create: `livestream-morphing-rs/src/effects/mod.rs`
- Create: `livestream-morphing-rs/src/effects/quantize.rs`
- Create: `livestream-morphing-rs/src/effects/distortion.rs`
- Create: `livestream-morphing-rs/src/effects/edges.rs`
- Create: `livestream-morphing-rs/src/effects/canvas_texture.rs`
- Delete: `livestream-morphing-rs/src/effects.rs`

This task refactors the monolithic `effects.rs` into individual files. Each effect implements the `Effect` trait and uses `register_effect!`.

- [ ] **Step 1: Create effects directory and mod.rs**

Create directory `src/effects/`. Create `src/effects/mod.rs` with `RawFrame`, utility functions, and module declarations:

```rust
pub mod quantize;
pub mod distortion;
pub mod edges;
pub mod canvas_texture;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downsample_halves_dimensions() {
        let src = RawFrame::filled(8, 6, 100, 100, 100);
        let dst = downsample_2x(&src);
        assert_eq!(dst.width, 4);
        assert_eq!(dst.height, 3);
        assert_eq!(dst.data.len(), (4 * 3 * 3) as usize);
    }

    #[test]
    fn upsample_doubles_dimensions() {
        let src = RawFrame::filled(4, 3, 50, 50, 50);
        let dst = upsample_2x(&src, 8, 6);
        assert_eq!(dst.width, 8);
        assert_eq!(dst.height, 6);
        assert_eq!(dst.data[0], 50);
    }
}
```

- [ ] **Step 2: Create effects/quantize.rs**

```rust
use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

#[derive(Default)]
pub struct Quantize;

impl Effect for Quantize {
    fn id(&self) -> &'static str { "quantize" }
    fn name(&self) -> &'static str { "Color Quantize" }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("levels", "Quantize Levels", 2.0, 32.0, 10.0, 1.0)]
    }

    fn init(&mut self, _width: u32, _height: u32) {}

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let levels = params.get("levels").copied().unwrap_or(10.0) as u8;
        let step = 255.0 / (levels - 1) as f32;
        for byte in frame.data.iter_mut() {
            let val = *byte as f32;
            *byte = ((val / step).round() * step).clamp(0.0, 255.0) as u8;
        }
    }
}

crate::register_effect!(Quantize);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn quantize_snaps_to_levels() {
        let mut frame = RawFrame::new(2, 1);
        frame.data = vec![50, 50, 50, 200, 200, 200];
        let mut fx = Quantize;
        let mut params = default_params(&fx.params());
        params.insert("levels".into(), 2.0);
        let ctx = FrameCtx { frame_number: 0, width: 2, height: 1 };
        fx.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, vec![0, 0, 0, 255, 255, 255]);
    }

    #[test]
    fn quantize_with_more_levels() {
        let mut frame = RawFrame::new(1, 1);
        frame.data = vec![100, 100, 100];
        let mut fx = Quantize;
        let mut params = default_params(&fx.params());
        params.insert("levels".into(), 4.0);
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        fx.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, vec![85, 85, 85]);
    }
}
```

- [ ] **Step 3: Create effects/distortion.rs**

```rust
use std::f32::consts::PI;
use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

pub struct Distortion {
    scratch: Option<RawFrame>,
}

impl Default for Distortion {
    fn default() -> Self {
        Self { scratch: None }
    }
}

impl Effect for Distortion {
    fn id(&self) -> &'static str { "distortion" }
    fn name(&self) -> &'static str { "Psychedelic Distortion" }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef::new("amplitude", "Distortion Amplitude", 0.0, 0.2, 0.02, 0.005),
            ParamDef::new("frequency", "Wave Frequency", 1.0, 50.0, 12.0, 0.5),
            ParamDef::new("cycle_length", "Cycle Length (frames)", 30.0, 600.0, 180.0, 1.0),
        ]
    }

    fn init(&mut self, width: u32, height: u32) {
        self.scratch = Some(RawFrame::new(width, height));
    }

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, ctx: &FrameCtx) {
        let scratch = self.scratch.as_mut().expect("Distortion not initialized");
        let amplitude = params.get("amplitude").copied().unwrap_or(0.02);
        let frequency = params.get("frequency").copied().unwrap_or(12.0);
        let cycle_length = params.get("cycle_length").copied().unwrap_or(180.0) as u32;

        let w = frame.width;
        let h = frame.height;
        let wf = w as f32;
        let hf = h as f32;
        let time = (ctx.frame_number % cycle_length) as f32 * (2.0 * PI / cycle_length as f32);

        for y in 0..h {
            let y_offset = (time + y as f32 * frequency / hf).sin() * hf * amplitude;
            for x in 0..w {
                let x_offset = (time + x as f32 * frequency / wf).sin() * wf * amplitude;

                let src_x = (x as f32 + x_offset).clamp(0.0, wf - 1.0);
                let src_y = (y as f32 + y_offset).clamp(0.0, hf - 1.0);

                let x0 = src_x.floor() as u32;
                let y0 = src_y.floor() as u32;
                let x1 = (x0 + 1).min(w - 1);
                let y1 = (y0 + 1).min(h - 1);
                let fx = src_x.fract();
                let fy = src_y.fract();

                let dst_idx = ((y * w + x) * 3) as usize;
                for c in 0..3 {
                    let p00 = frame.data[((y0 * w + x0) * 3) as usize + c] as f32;
                    let p10 = frame.data[((y0 * w + x1) * 3) as usize + c] as f32;
                    let p01 = frame.data[((y1 * w + x0) * 3) as usize + c] as f32;
                    let p11 = frame.data[((y1 * w + x1) * 3) as usize + c] as f32;
                    let val = p00 * (1.0 - fx) * (1.0 - fy)
                        + p10 * fx * (1.0 - fy)
                        + p01 * (1.0 - fx) * fy
                        + p11 * fx * fy;
                    scratch.data[dst_idx + c] = val.clamp(0.0, 255.0) as u8;
                }
            }
        }
        std::mem::swap(&mut frame.data, &mut scratch.data);
    }
}

crate::register_effect!(Distortion);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn zero_amplitude_is_identity() {
        let mut frame = RawFrame::filled(4, 4, 128, 64, 32);
        let original = frame.data.clone();
        let mut fx = Distortion::default();
        fx.init(4, 4);
        let mut params = default_params(&fx.params());
        params.insert("amplitude".into(), 0.0);
        let ctx = FrameCtx { frame_number: 0, width: 4, height: 4 };
        fx.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original);
    }

    #[test]
    fn nonzero_amplitude_changes_pixels() {
        let mut frame = RawFrame::new(8, 8);
        for y in 0..8u32 {
            for x in 0..8u32 {
                let idx = ((y * 8 + x) * 3) as usize;
                frame.data[idx] = (x * 32) as u8;
                frame.data[idx + 1] = (y * 32) as u8;
                frame.data[idx + 2] = 0;
            }
        }
        let original = frame.data.clone();
        let mut fx = Distortion::default();
        fx.init(8, 8);
        let mut params = default_params(&fx.params());
        params.insert("amplitude".into(), 0.05);
        let ctx = FrameCtx { frame_number: 10, width: 8, height: 8 };
        fx.apply(&mut frame, &params, &ctx);
        assert_ne!(frame.data, original);
    }
}
```

- [ ] **Step 4: Create effects/edges.rs**

```rust
use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

pub struct EdgeDetect {
    grayscale: Vec<u8>,
    edges: Vec<u8>,
}

impl Default for EdgeDetect {
    fn default() -> Self {
        Self { grayscale: Vec::new(), edges: Vec::new() }
    }
}

impl Effect for EdgeDetect {
    fn id(&self) -> &'static str { "edges" }
    fn name(&self) -> &'static str { "Edge Detection" }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef::new("threshold", "Edge Threshold", 1.0, 128.0, 30.0, 1.0),
            ParamDef::new("darkness", "Edge Darkness", 0.0, 255.0, 80.0, 1.0),
        ]
    }

    fn init(&mut self, width: u32, height: u32) {
        let n = (width * height) as usize;
        self.grayscale = vec![0u8; n];
        self.edges = vec![0u8; n];
    }

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let threshold = params.get("threshold").copied().unwrap_or(30.0) as u8;
        let darkness = params.get("darkness").copied().unwrap_or(80.0) as u8;
        let w = frame.width as usize;
        let h = frame.height as usize;

        // RGB → grayscale (BT.601 weights)
        for i in 0..(w * h) {
            let r = frame.data[i * 3] as u16;
            let g = frame.data[i * 3 + 1] as u16;
            let b = frame.data[i * 3 + 2] as u16;
            self.grayscale[i] = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
        }

        // Clear edges
        self.edges.iter_mut().for_each(|e| *e = 0);

        // Sobel (skip border pixels)
        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                let g = |dy: i32, dx: i32| -> i16 {
                    self.grayscale[((y as i32 + dy) as usize) * w + (x as i32 + dx) as usize]
                        as i16
                };
                let gx =
                    -g(-1, -1) + g(-1, 1) - 2 * g(0, -1) + 2 * g(0, 1) - g(1, -1) + g(1, 1);
                let gy =
                    -g(-1, -1) - 2 * g(-1, 0) - g(-1, 1) + g(1, -1) + 2 * g(1, 0) + g(1, 1);
                let mag = ((gx.unsigned_abs() + gy.unsigned_abs()) / 2).min(255) as u8;
                self.edges[y * w + x] = if mag > threshold { 255 } else { 0 };
            }
        }

        // Overlay dark edges
        for i in 0..(w * h) {
            if self.edges[i] > 0 {
                frame.data[i * 3] = frame.data[i * 3].saturating_sub(darkness);
                frame.data[i * 3 + 1] = frame.data[i * 3 + 1].saturating_sub(darkness);
                frame.data[i * 3 + 2] = frame.data[i * 3 + 2].saturating_sub(darkness);
            }
        }
    }
}

crate::register_effect!(EdgeDetect);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn edges_detected_at_sharp_boundary() {
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
        let mut fx = EdgeDetect::default();
        fx.init(8, 4);
        let params = default_params(&fx.params());
        let ctx = FrameCtx { frame_number: 0, width: 8, height: 4 };
        fx.apply(&mut frame, &params, &ctx);
        let mid_pixel = frame.data[((1 * 8 + 4) * 3) as usize];
        assert!(mid_pixel < original[((1 * 8 + 4) * 3) as usize], "Edge pixel should be darkened");
    }

    #[test]
    fn no_edges_on_uniform_frame() {
        let mut frame = RawFrame::filled(8, 8, 128, 128, 128);
        let original = frame.data.clone();
        let mut fx = EdgeDetect::default();
        fx.init(8, 8);
        let params = default_params(&fx.params());
        let ctx = FrameCtx { frame_number: 0, width: 8, height: 8 };
        fx.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original, "Uniform frame should have no edges");
    }
}
```

- [ ] **Step 5: Create effects/canvas_texture.rs**

```rust
use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

pub struct CanvasTexture {
    texture: Vec<u8>,
}

impl Default for CanvasTexture {
    fn default() -> Self {
        Self { texture: Vec::new() }
    }
}

fn generate_canvas_texture(width: u32, height: u32) -> Vec<u8> {
    let mut texture = vec![0u8; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let h = x
                .wrapping_mul(374761393)
                .wrapping_add(y.wrapping_mul(668265263))
                .wrapping_mul(1274126177);
            let noise = ((h >> 24) & 0x1F) as u8;
            let weave: u8 = if (x % 4 < 2) ^ (y % 4 < 2) { 10 } else { 0 };
            texture[(y * width + x) as usize] = 200u8.wrapping_add(noise).wrapping_add(weave);
        }
    }
    texture
}

impl Effect for CanvasTexture {
    fn id(&self) -> &'static str { "canvas_texture" }
    fn name(&self) -> &'static str { "Canvas Texture" }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("strength", "Texture Strength", 0.0, 1.0, 0.15, 0.01)]
    }

    fn init(&mut self, width: u32, height: u32) {
        self.texture = generate_canvas_texture(width, height);
    }

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let strength = params.get("strength").copied().unwrap_or(0.15);
        let pixel_count = (frame.width * frame.height) as usize;
        for i in 0..pixel_count {
            let tex = self.texture[i % self.texture.len()] as f32 / 255.0;
            let factor = 1.0 - strength + strength * tex;
            for c in 0..3 {
                let idx = i * 3 + c;
                frame.data[idx] = (frame.data[idx] as f32 * factor).clamp(0.0, 255.0) as u8;
            }
        }
    }
}

crate::register_effect!(CanvasTexture);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn texture_blend_darkens_pixels() {
        let mut frame = RawFrame::filled(4, 4, 200, 200, 200);
        let mut fx = CanvasTexture::default();
        fx.init(4, 4);
        let mut params = default_params(&fx.params());
        params.insert("strength".into(), 0.5);
        let ctx = FrameCtx { frame_number: 0, width: 4, height: 4 };
        fx.apply(&mut frame, &params, &ctx);
        assert!(frame.data[0] < 200, "Should be darkened");
        assert!(frame.data[0] > 100, "Shouldn't be too dark");
    }

    #[test]
    fn zero_strength_is_identity() {
        let mut frame = RawFrame::filled(2, 2, 100, 100, 100);
        let original = frame.data.clone();
        let mut fx = CanvasTexture::default();
        fx.init(2, 2);
        let mut params = default_params(&fx.params());
        params.insert("strength".into(), 0.0);
        let ctx = FrameCtx { frame_number: 0, width: 2, height: 2 };
        fx.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original);
    }
}
```

- [ ] **Step 6: Delete old effects.rs**

Delete `src/effects.rs`. The `mod effects;` declaration in `main.rs` will now resolve to `src/effects/mod.rs`.

- [ ] **Step 7: Update pipeline.rs imports**

In `src/pipeline.rs`, update the import at line 5. Change:

```rust
use crate::effects::{downsample_2x, upsample_2x, FrameProcessor};
```

To (temporarily, until Task 5 replaces FrameProcessor with dynamic Pipeline):

```rust
use crate::effects::{downsample_2x, upsample_2x, RawFrame};
```

Also replace all `FrameProcessor` usage in `pipeline.rs` with inline effect processing. In the `spawn_blocking` closures (lines 112-157 and 194-217), replace the `FrameProcessor` creation and usage with direct effect calls:

```rust
// Replace FrameProcessor::new + process_frame with this temporary approach:
use crate::effects::{quantize, distortion, edges, canvas_texture};
use crate::registry::{default_params, FrameCtx};

let mut quant = quantize::Quantize::default();
let mut dist = distortion::Distortion::default();
dist.init(half_w, half_h);
let mut edge = edges::EdgeDetect::default();
edge.init(half_w, half_h);
let mut tex = canvas_texture::CanvasTexture::default();
tex.init(half_w, half_h);

let (edge_color, _bg) = crate::time_color::get_colors_now();
let edge_darkness = if edge_color == (0, 0, 0) { 100.0 } else { 40.0 };

let dist_params = default_params(&dist.params());
let quant_params = default_params(&quant.params());
let mut edge_params = default_params(&edge.params());
edge_params.insert("darkness".into(), edge_darkness);
let tex_params = default_params(&tex.params());

for (i, frame) in half_frames.iter_mut().enumerate() {
    let ctx = FrameCtx { frame_number: i as u32, width: half_w, height: half_h };
    dist.apply(frame, &dist_params, &ctx);
    quant.apply(frame, &quant_params, &ctx);
    edge.apply(frame, &edge_params, &ctx);
    tex.apply(frame, &tex_params, &ctx);
}
```

Apply this same replacement to both `spawn_blocking` closures in pipeline.rs (the main processing block and the prefetched block).

- [ ] **Step 8: Run all tests**

Run: `cd /Users/worthy/TestCode/livestream-morphing/livestream-morphing-rs && cargo test`
Expected: All existing tests pass (quantize, distortion, edges, canvas_texture, downsample, upsample, codec, hls, stream_source, time_color, registry)

- [ ] **Step 9: Commit**

```bash
cd /Users/worthy/TestCode/livestream-morphing
git add -A livestream-morphing-rs/src/effects/ livestream-morphing-rs/src/pipeline.rs
git rm livestream-morphing-rs/src/effects.rs
git commit -m "refactor: split effects.rs into per-effect module files with Effect trait"
```

---

### Task 4: Add New Effects (Blur + Color Shift)

**Files:**
- Create: `livestream-morphing-rs/src/effects/blur.rs`
- Create: `livestream-morphing-rs/src/effects/color_shift.rs`
- Modify: `livestream-morphing-rs/src/effects/mod.rs`

- [ ] **Step 1: Create effects/blur.rs**

```rust
use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

pub struct Blur {
    buffer: Vec<u8>,
}

impl Default for Blur {
    fn default() -> Self {
        Self { buffer: Vec::new() }
    }
}

impl Effect for Blur {
    fn id(&self) -> &'static str { "blur" }
    fn name(&self) -> &'static str { "Box Blur" }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("radius", "Blur Radius", 0.0, 20.0, 3.0, 1.0)]
    }

    fn init(&mut self, width: u32, height: u32) {
        self.buffer = vec![0u8; (width * height * 3) as usize];
    }

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let radius = params.get("radius").copied().unwrap_or(3.0) as i32;
        if radius <= 0 {
            return;
        }
        let w = frame.width as i32;
        let h = frame.height as i32;

        // Horizontal pass: frame → buffer
        for y in 0..h {
            for x in 0..w {
                let mut r_sum: u32 = 0;
                let mut g_sum: u32 = 0;
                let mut b_sum: u32 = 0;
                let mut count: u32 = 0;
                for dx in -radius..=radius {
                    let sx = (x + dx).clamp(0, w - 1);
                    let idx = ((y * w + sx) * 3) as usize;
                    r_sum += frame.data[idx] as u32;
                    g_sum += frame.data[idx + 1] as u32;
                    b_sum += frame.data[idx + 2] as u32;
                    count += 1;
                }
                let idx = ((y * w + x) * 3) as usize;
                self.buffer[idx] = (r_sum / count) as u8;
                self.buffer[idx + 1] = (g_sum / count) as u8;
                self.buffer[idx + 2] = (b_sum / count) as u8;
            }
        }

        // Vertical pass: buffer → frame
        for y in 0..h {
            for x in 0..w {
                let mut r_sum: u32 = 0;
                let mut g_sum: u32 = 0;
                let mut b_sum: u32 = 0;
                let mut count: u32 = 0;
                for dy in -radius..=radius {
                    let sy = (y + dy).clamp(0, h - 1);
                    let idx = ((sy * w + x) * 3) as usize;
                    r_sum += self.buffer[idx] as u32;
                    g_sum += self.buffer[idx + 1] as u32;
                    b_sum += self.buffer[idx + 2] as u32;
                    count += 1;
                }
                let idx = ((y * w + x) * 3) as usize;
                frame.data[idx] = (r_sum / count) as u8;
                frame.data[idx + 1] = (g_sum / count) as u8;
                frame.data[idx + 2] = (b_sum / count) as u8;
            }
        }
    }
}

crate::register_effect!(Blur);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn blur_smooths_sharp_edge() {
        let mut frame = RawFrame::new(8, 1);
        // Left half white, right half black
        for x in 0..8u32 {
            let idx = (x * 3) as usize;
            let val = if x < 4 { 255 } else { 0 };
            frame.data[idx] = val;
            frame.data[idx + 1] = val;
            frame.data[idx + 2] = val;
        }
        let mut fx = Blur::default();
        fx.init(8, 1);
        let mut params = default_params(&fx.params());
        params.insert("radius".into(), 1.0);
        let ctx = FrameCtx { frame_number: 0, width: 8, height: 1 };
        fx.apply(&mut frame, &params, &ctx);
        // Pixel at boundary (x=3) should be less than 255 (blurred with neighbor)
        let boundary_val = frame.data[3 * 3];
        assert!(boundary_val < 255 && boundary_val > 0, "Boundary should be blurred, got {boundary_val}");
    }

    #[test]
    fn zero_radius_is_identity() {
        let mut frame = RawFrame::filled(4, 4, 100, 150, 200);
        let original = frame.data.clone();
        let mut fx = Blur::default();
        fx.init(4, 4);
        let mut params = default_params(&fx.params());
        params.insert("radius".into(), 0.0);
        let ctx = FrameCtx { frame_number: 0, width: 4, height: 4 };
        fx.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original);
    }
}
```

- [ ] **Step 2: Create effects/color_shift.rs**

```rust
use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

#[derive(Default)]
pub struct ColorShift;

impl Effect for ColorShift {
    fn id(&self) -> &'static str { "color_shift" }
    fn name(&self) -> &'static str { "Color Shift" }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef::new("hue", "Hue Rotation", 0.0, 360.0, 0.0, 1.0),
            ParamDef::new("saturation", "Saturation", 0.0, 2.0, 1.0, 0.05),
            ParamDef::new("brightness", "Brightness", 0.0, 2.0, 1.0, 0.05),
        ]
    }

    fn init(&mut self, _width: u32, _height: u32) {}

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let hue_shift = params.get("hue").copied().unwrap_or(0.0);
        let sat_scale = params.get("saturation").copied().unwrap_or(1.0);
        let bright_scale = params.get("brightness").copied().unwrap_or(1.0);

        if hue_shift == 0.0 && sat_scale == 1.0 && bright_scale == 1.0 {
            return;
        }

        for pixel in frame.data.chunks_exact_mut(3) {
            let r = pixel[0] as f32 / 255.0;
            let g = pixel[1] as f32 / 255.0;
            let b = pixel[2] as f32 / 255.0;

            // RGB → HSV
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let delta = max - min;

            let mut h = if delta == 0.0 {
                0.0
            } else if max == r {
                60.0 * (((g - b) / delta) % 6.0)
            } else if max == g {
                60.0 * (((b - r) / delta) + 2.0)
            } else {
                60.0 * (((r - g) / delta) + 4.0)
            };
            if h < 0.0 { h += 360.0; }

            let s = if max == 0.0 { 0.0 } else { delta / max };
            let v = max;

            // Apply shifts
            h = (h + hue_shift) % 360.0;
            let s = (s * sat_scale).clamp(0.0, 1.0);
            let v = (v * bright_scale).clamp(0.0, 1.0);

            // HSV → RGB
            let c = v * s;
            let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
            let m = v - c;

            let (r1, g1, b1) = if h < 60.0 {
                (c, x, 0.0)
            } else if h < 120.0 {
                (x, c, 0.0)
            } else if h < 180.0 {
                (0.0, c, x)
            } else if h < 240.0 {
                (0.0, x, c)
            } else if h < 300.0 {
                (x, 0.0, c)
            } else {
                (c, 0.0, x)
            };

            pixel[0] = ((r1 + m) * 255.0).clamp(0.0, 255.0) as u8;
            pixel[1] = ((g1 + m) * 255.0).clamp(0.0, 255.0) as u8;
            pixel[2] = ((b1 + m) * 255.0).clamp(0.0, 255.0) as u8;
        }
    }
}

crate::register_effect!(ColorShift);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn identity_when_defaults() {
        let mut frame = RawFrame::filled(2, 2, 100, 150, 200);
        let original = frame.data.clone();
        let mut fx = ColorShift;
        let params = default_params(&fx.params());
        let ctx = FrameCtx { frame_number: 0, width: 2, height: 2 };
        fx.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original);
    }

    #[test]
    fn brightness_scales_values() {
        let mut frame = RawFrame::filled(1, 1, 100, 100, 100);
        let mut fx = ColorShift;
        let mut params = default_params(&fx.params());
        params.insert("brightness".into(), 0.5);
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        fx.apply(&mut frame, &params, &ctx);
        assert!(frame.data[0] < 100, "Brightness 0.5 should darken, got {}", frame.data[0]);
    }

    #[test]
    fn hue_rotation_changes_color() {
        let mut frame = RawFrame::filled(1, 1, 255, 0, 0); // pure red
        let mut fx = ColorShift;
        let mut params = default_params(&fx.params());
        params.insert("hue".into(), 120.0); // shift red → green
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        fx.apply(&mut frame, &params, &ctx);
        // Should now be green-ish
        assert!(frame.data[1] > frame.data[0], "Green should dominate after 120° shift");
    }
}
```

- [ ] **Step 3: Update effects/mod.rs**

Add the new module declarations after the existing ones:

```rust
pub mod blur;
pub mod color_shift;
```

- [ ] **Step 4: Run all tests**

Run: `cd /Users/worthy/TestCode/livestream-morphing/livestream-morphing-rs && cargo test`
Expected: All tests pass including new blur and color_shift tests

- [ ] **Step 5: Commit**

```bash
cd /Users/worthy/TestCode/livestream-morphing
git add livestream-morphing-rs/src/effects/blur.rs livestream-morphing-rs/src/effects/color_shift.rs livestream-morphing-rs/src/effects/mod.rs
git commit -m "feat: add Blur and Color Shift effects"
```

---

### Task 5: Dynamic Pipeline Engine

**Files:**
- Modify: `livestream-morphing-rs/src/pipeline.rs`

This task adds the `PipelineSlot` and `Pipeline` structs to `pipeline.rs`, replaces the inline effect processing from Task 3 with the dynamic pipeline, and adds the pipeline to `AppState`.

- [ ] **Step 1: Write Pipeline tests**

Add at the bottom of `pipeline.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::RawFrame;
    use crate::registry::{default_params, all_effects};

    #[test]
    fn pipeline_processes_frame() {
        let mut pipeline = Pipeline::new();
        // Add quantize effect
        pipeline.add_effect("quantize").unwrap();
        pipeline.set_dimensions(8, 8);

        let mut frame = RawFrame::new(8, 8);
        for i in 0..frame.data.len() {
            frame.data[i] = (i % 256) as u8;
        }
        let original = frame.data.clone();
        pipeline.process_frame(&mut frame, 0);
        assert_ne!(frame.data, original, "Pipeline should modify frame");
    }

    #[test]
    fn disabled_effect_skipped() {
        let mut pipeline = Pipeline::new();
        let slot_id = pipeline.add_effect("quantize").unwrap();
        pipeline.set_dimensions(4, 4);
        pipeline.set_enabled(&slot_id, false);

        let mut frame = RawFrame::filled(4, 4, 100, 100, 100);
        let original = frame.data.clone();
        pipeline.process_frame(&mut frame, 0);
        assert_eq!(frame.data, original, "Disabled effect should not modify frame");
    }

    #[test]
    fn add_unknown_effect_fails() {
        let mut pipeline = Pipeline::new();
        assert!(pipeline.add_effect("nonexistent").is_err());
    }

    #[test]
    fn remove_effect() {
        let mut pipeline = Pipeline::new();
        let slot_id = pipeline.add_effect("quantize").unwrap();
        assert_eq!(pipeline.slot_count(), 1);
        pipeline.remove_slot(&slot_id);
        assert_eq!(pipeline.slot_count(), 0);
    }
}
```

- [ ] **Step 2: Implement Pipeline structs**

Add the following above the existing `AppState` struct in `pipeline.rs`:

```rust
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use crate::registry::{self, Effect, FrameCtx, ParamDef, ParamValues};

/// A single slot in the pipeline — one effect instance with its params.
pub struct PipelineSlot {
    pub slot_id: String,
    pub effect_id: String,
    pub effect: Box<dyn Effect>,
    pub params: ParamValues,
    pub enabled: bool,
}

/// Serializable view of a slot for API responses.
#[derive(Serialize, Deserialize, Clone)]
pub struct PipelineSlotView {
    pub slot_id: String,
    pub effect_id: String,
    pub params: ParamValues,
    pub enabled: bool,
}

/// The dynamic effect pipeline.
pub struct Pipeline {
    slots: Vec<PipelineSlot>,
    dimensions: Option<(u32, u32)>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self { slots: Vec::new(), dimensions: None }
    }

    pub fn set_dimensions(&mut self, width: u32, height: u32) {
        self.dimensions = Some((width, height));
        for slot in &mut self.slots {
            slot.effect.init(width, height);
        }
    }

    /// Add an effect by its registered ID. Returns the new slot_id.
    pub fn add_effect(&mut self, effect_id: &str) -> Result<String, String> {
        let factories = registry::all_effects();
        let mut effect = factories
            .into_iter()
            .find(|e| e.id() == effect_id)
            .ok_or_else(|| format!("Unknown effect: {effect_id}"))?;

        let params = registry::default_params(&effect.params());
        if let Some((w, h)) = self.dimensions {
            effect.init(w, h);
        }

        let slot_id = uuid::Uuid::new_v4().to_string();
        self.slots.push(PipelineSlot {
            slot_id: slot_id.clone(),
            effect_id: effect_id.to_string(),
            effect,
            params,
            enabled: true,
        });
        Ok(slot_id)
    }

    /// Remove a slot by ID.
    pub fn remove_slot(&mut self, slot_id: &str) {
        self.slots.retain(|s| s.slot_id != slot_id);
    }

    /// Set enabled state for a slot.
    pub fn set_enabled(&mut self, slot_id: &str, enabled: bool) {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.slot_id == slot_id) {
            slot.enabled = enabled;
        }
    }

    /// Update params for a slot (merges new values into existing).
    pub fn update_params(&mut self, slot_id: &str, new_params: &ParamValues) {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.slot_id == slot_id) {
            for (k, v) in new_params {
                slot.params.insert(k.clone(), *v);
            }
        }
    }

    /// Replace the entire pipeline with a new slot ordering.
    /// Each entry is (effect_id, params, enabled).
    pub fn replace(&mut self, entries: Vec<(String, ParamValues, bool)>) -> Result<(), String> {
        let mut new_slots = Vec::new();
        for (effect_id, params, enabled) in entries {
            let factories = registry::all_effects();
            let mut effect = factories
                .into_iter()
                .find(|e| e.id() == effect_id)
                .ok_or_else(|| format!("Unknown effect: {effect_id}"))?;

            if let Some((w, h)) = self.dimensions {
                effect.init(w, h);
            }

            let slot_id = uuid::Uuid::new_v4().to_string();
            new_slots.push(PipelineSlot {
                slot_id,
                effect_id,
                effect,
                params,
                enabled,
            });
        }
        self.slots = new_slots;
        Ok(())
    }

    /// Get a serializable view of the pipeline.
    pub fn view(&self) -> Vec<PipelineSlotView> {
        self.slots
            .iter()
            .map(|s| PipelineSlotView {
                slot_id: s.slot_id.clone(),
                effect_id: s.effect_id.clone(),
                params: s.params.clone(),
                enabled: s.enabled,
            })
            .collect()
    }

    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Process a frame through all enabled effects in order.
    pub fn process_frame(&mut self, frame: &mut crate::effects::RawFrame, frame_number: u32) {
        let ctx = FrameCtx {
            frame_number,
            width: frame.width,
            height: frame.height,
        };
        for slot in &mut self.slots {
            if slot.enabled {
                slot.effect.apply(frame, &slot.params, &ctx);
            }
        }
    }
}
```

- [ ] **Step 3: Add Pipeline to AppState**

Update `AppState` to include the shared pipeline. Change the struct definition:

```rust
pub struct AppState {
    pub hls_buffer: RwLock<HlsBuffer>,
    pub pipeline_active: watch::Sender<bool>,
    pub last_client_request: std::sync::atomic::AtomicU64,
    pub pipeline: Mutex<Pipeline>,
}
```

Update `AppState::new()` to initialize the pipeline with the default effects:

```rust
pub fn new() -> (Arc<Self>, watch::Receiver<bool>) {
    let (tx, rx) = watch::channel(false);

    let mut pipeline = Pipeline::new();
    // Default pipeline matches original behavior
    let _ = pipeline.add_effect("distortion");
    let _ = pipeline.add_effect("quantize");
    let _ = pipeline.add_effect("edges");
    let _ = pipeline.add_effect("canvas_texture");

    let state = Arc::new(Self {
        hls_buffer: RwLock::new(HlsBuffer::new(10)),
        pipeline_active: tx,
        last_client_request: std::sync::atomic::AtomicU64::new(0),
        pipeline: Mutex::new(pipeline),
    });
    (state, rx)
}
```

- [ ] **Step 4: Update pipeline::run to use dynamic Pipeline**

In the `run()` function, update both `spawn_blocking` closures to use the shared pipeline. The key change is passing `Arc<AppState>` into the blocking closure and locking the pipeline mutex.

Replace the first `spawn_blocking` block (around lines 112-157) — the closure needs access to `state`:

```rust
let state_clone = state.clone();
let process_handle = tokio::task::spawn_blocking(
    move || -> Result<(Vec<u8>, i64), Box<dyn std::error::Error + Send + Sync>> {
        let t0 = std::time::Instant::now();

        let all_frames = codec::decode_segment(&ts_bytes)?;
        let t_decode = t0.elapsed();

        if all_frames.is_empty() {
            return Err("No frames decoded".into());
        }

        let mut half_frames: Vec<_> = all_frames
            .iter()
            .map(|f| downsample_2x(f))
            .collect();
        let t_downsample = t0.elapsed();

        let half_w = half_frames[0].width;
        let half_h = half_frames[0].height;
        let out_frames = half_frames.len() as i64;

        {
            let mut pipeline = state_clone.pipeline.lock().unwrap();
            pipeline.set_dimensions(half_w, half_h);

            // Apply time-of-day edge darkness
            let (edge_color, _bg) = crate::time_color::get_colors_now();
            let edge_darkness = if edge_color == (0, 0, 0) { 100.0 } else { 40.0 };
            for slot in pipeline.view().iter() {
                if slot.effect_id == "edges" {
                    let mut params = std::collections::HashMap::new();
                    params.insert("darkness".to_string(), edge_darkness);
                    pipeline.update_params(&slot.slot_id, &params);
                }
            }

            for (i, frame) in half_frames.iter_mut().enumerate() {
                pipeline.process_frame(frame, i as u32);
            }
        }
        let t_effects = t0.elapsed();

        let encoded = codec::encode_segment(&half_frames, 30, pts_offset)?;
        let t_encode = t0.elapsed();

        tracing::info!(
            decode_ms = t_decode.as_millis() as u64,
            downsample_ms = (t_downsample - t_decode).as_millis() as u64,
            effects_ms = (t_effects - t_downsample).as_millis() as u64,
            encode_ms = (t_encode - t_effects).as_millis() as u64,
            total_ms = t_encode.as_millis() as u64,
            frames = out_frames,
            "Pipeline timing"
        );

        Ok((encoded, out_frames))
    },
);
```

Apply the same pattern to the second (prefetched) `spawn_blocking` block. Clone `state` before entering the closure and lock the pipeline inside.

- [ ] **Step 5: Run tests**

Run: `cd /Users/worthy/TestCode/livestream-morphing/livestream-morphing-rs && cargo test`
Expected: All tests pass including new pipeline tests

- [ ] **Step 6: Commit**

```bash
cd /Users/worthy/TestCode/livestream-morphing
git add livestream-morphing-rs/src/pipeline.rs
git commit -m "feat: dynamic pipeline engine with add/remove/reorder/toggle"
```

---

### Task 6: REST API + Preset Storage

**Files:**
- Create: `livestream-morphing-rs/src/api.rs`
- Create: `livestream-morphing-rs/presets/default.toml`

- [ ] **Step 1: Create api.rs with all endpoints**

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, patch, put, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::pipeline::{AppState, PipelineSlotView};
use crate::registry;

/// Effect definition returned by GET /api/effects.
#[derive(Serialize)]
struct EffectDef {
    id: &'static str,
    name: &'static str,
    params: Vec<registry::ParamDef>,
}

/// GET /api/effects — list all registered effects.
async fn list_effects() -> Json<Vec<EffectDef>> {
    let effects = registry::all_effects();
    let defs: Vec<_> = effects
        .iter()
        .map(|e| EffectDef {
            id: e.id(),
            name: e.name(),
            params: e.params(),
        })
        .collect();
    Json(defs)
}

/// GET /api/pipeline — get current pipeline state.
async fn get_pipeline(State(state): State<Arc<AppState>>) -> Json<Vec<PipelineSlotView>> {
    let pipeline = state.pipeline.lock().unwrap();
    Json(pipeline.view())
}

#[derive(Deserialize)]
struct PipelineEntry {
    effect_id: String,
    params: std::collections::HashMap<String, f32>,
    enabled: bool,
}

/// PUT /api/pipeline — replace entire pipeline.
async fn put_pipeline(
    State(state): State<Arc<AppState>>,
    Json(entries): Json<Vec<PipelineEntry>>,
) -> impl IntoResponse {
    let mut pipeline = state.pipeline.lock().unwrap();
    let entries: Vec<_> = entries
        .into_iter()
        .map(|e| (e.effect_id, e.params, e.enabled))
        .collect();
    match pipeline.replace(entries) {
        Ok(()) => (StatusCode::OK, Json(pipeline.view())).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

#[derive(Deserialize)]
struct PatchSlot {
    params: Option<std::collections::HashMap<String, f32>>,
    enabled: Option<bool>,
}

/// PATCH /api/pipeline/:slot_id — update one slot's params or enabled state.
async fn patch_slot(
    State(state): State<Arc<AppState>>,
    Path(slot_id): Path<String>,
    Json(patch): Json<PatchSlot>,
) -> impl IntoResponse {
    let mut pipeline = state.pipeline.lock().unwrap();
    if let Some(params) = &patch.params {
        pipeline.update_params(&slot_id, params);
    }
    if let Some(enabled) = patch.enabled {
        pipeline.set_enabled(&slot_id, enabled);
    }
    Json(pipeline.view())
}

/// POST /api/pipeline/add/:effect_id — add an effect to the pipeline.
async fn add_effect(
    State(state): State<Arc<AppState>>,
    Path(effect_id): Path<String>,
) -> impl IntoResponse {
    let mut pipeline = state.pipeline.lock().unwrap();
    match pipeline.add_effect(&effect_id) {
        Ok(_slot_id) => (StatusCode::OK, Json(pipeline.view())).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

/// DELETE /api/pipeline/:slot_id — remove a slot from the pipeline.
async fn remove_slot(
    State(state): State<Arc<AppState>>,
    Path(slot_id): Path<String>,
) -> Json<Vec<PipelineSlotView>> {
    let mut pipeline = state.pipeline.lock().unwrap();
    pipeline.remove_slot(&slot_id);
    Json(pipeline.view())
}

// --- Presets ---

#[derive(Serialize, Deserialize, Clone)]
pub struct Preset {
    pub name: String,
    pub effects: Vec<PresetEffect>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PresetEffect {
    pub effect_id: String,
    pub enabled: bool,
    pub params: std::collections::HashMap<String, f32>,
}

#[derive(Serialize)]
struct PresetSummary {
    id: String,
    name: String,
}

fn presets_dir() -> std::path::PathBuf {
    let dir = std::path::PathBuf::from("presets");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn load_preset(id: &str) -> Option<Preset> {
    let path = presets_dir().join(format!("{id}.toml"));
    let content = std::fs::read_to_string(path).ok()?;
    toml::from_str(&content).ok()
}

fn save_preset(id: &str, preset: &Preset) -> std::io::Result<()> {
    let path = presets_dir().join(format!("{id}.toml"));
    let content = toml::to_string_pretty(preset).unwrap();
    std::fs::write(path, content)
}

/// GET /api/presets — list saved presets.
async fn list_presets() -> Json<Vec<PresetSummary>> {
    let dir = presets_dir();
    let mut presets = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "toml").unwrap_or(false) {
                let id = path.file_stem().unwrap().to_string_lossy().to_string();
                if let Some(preset) = load_preset(&id) {
                    presets.push(PresetSummary { id, name: preset.name });
                }
            }
        }
    }
    Json(presets)
}

#[derive(Deserialize)]
struct SavePresetBody {
    name: String,
}

/// POST /api/presets — save current pipeline as a preset.
async fn create_preset(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SavePresetBody>,
) -> impl IntoResponse {
    let pipeline = state.pipeline.lock().unwrap();
    let view = pipeline.view();
    let preset = Preset {
        name: body.name.clone(),
        effects: view
            .into_iter()
            .map(|s| PresetEffect {
                effect_id: s.effect_id,
                enabled: s.enabled,
                params: s.params,
            })
            .collect(),
    };
    let id = body.name.to_lowercase().replace(' ', "_");
    match save_preset(&id, &preset) {
        Ok(()) => (StatusCode::CREATED, Json(PresetSummary { id, name: body.name })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to save: {e}")).into_response(),
    }
}

/// PUT /api/presets/:id/apply — load a preset into the active pipeline.
async fn apply_preset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let preset = match load_preset(&id) {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Preset not found").into_response(),
    };
    let mut pipeline = state.pipeline.lock().unwrap();
    let entries: Vec<_> = preset
        .effects
        .into_iter()
        .map(|e| (e.effect_id, e.params, e.enabled))
        .collect();
    match pipeline.replace(entries) {
        Ok(()) => Json(pipeline.view()).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

/// DELETE /api/presets/:id — delete a preset.
async fn delete_preset(Path(id): Path<String>) -> impl IntoResponse {
    let path = presets_dir().join(format!("{id}.toml"));
    match std::fs::remove_file(path) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Build the API router.
pub fn api_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/effects", get(list_effects))
        .route("/api/pipeline", get(get_pipeline).put(put_pipeline))
        .route("/api/pipeline/add/{effect_id}", post(add_effect))
        .route("/api/pipeline/{slot_id}", patch(patch_slot).delete(remove_slot))
        .route("/api/presets", get(list_presets).post(create_preset))
        .route("/api/presets/{id}/apply", put(apply_preset))
        .route("/api/presets/{id}", delete(delete_preset))
}
```

- [ ] **Step 2: Create default preset**

Create `livestream-morphing-rs/presets/default.toml`:

```toml
name = "Default"

[[effects]]
effect_id = "distortion"
enabled = true

[effects.params]
amplitude = 0.02
frequency = 12.0
cycle_length = 180.0

[[effects]]
effect_id = "quantize"
enabled = true

[effects.params]
levels = 10.0

[[effects]]
effect_id = "edges"
enabled = true

[effects.params]
threshold = 30.0
darkness = 80.0

[[effects]]
effect_id = "canvas_texture"
enabled = true

[effects.params]
strength = 0.15
```

- [ ] **Step 3: Add module to main.rs**

Add `mod api;` to `src/main.rs`:

```rust
mod api;
mod codec;
mod effects;
mod hls;
mod pipeline;
mod registry;
mod server;
mod stream_source;
mod time_color;
```

- [ ] **Step 4: Verify it compiles**

Run: `cd /Users/worthy/TestCode/livestream-morphing/livestream-morphing-rs && cargo check`
Expected: Compiles with no errors

- [ ] **Step 5: Commit**

```bash
cd /Users/worthy/TestCode/livestream-morphing
git add livestream-morphing-rs/src/api.rs livestream-morphing-rs/presets/default.toml livestream-morphing-rs/src/main.rs
git commit -m "feat: REST API endpoints for effects, pipeline, and presets"
```

---

### Task 7: Wire Up Server + CORS

**Files:**
- Modify: `livestream-morphing-rs/src/server.rs`

- [ ] **Step 1: Update server.rs to include API routes**

Replace the entire `server.rs` with:

```rust
use axum::{
    extract::{Path, State},
    http::{header, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::api;
use crate::pipeline::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods([Method::GET, Method::PUT, Method::PATCH, Method::POST, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE]);

    Router::new()
        .route("/api/stream", get(stream_playlist))
        .route("/api/segments/{segment_id}", get(get_segment))
        .route("/health", get(health))
        .merge(api::api_router())
        .layer(cors)
        .with_state(state)
}

async fn stream_playlist(State(state): State<Arc<AppState>>) -> Response {
    state.touch();

    let playlist = {
        let buf = state.hls_buffer.read().await;
        buf.generate_playlist()
    };

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/vnd.apple.mpegurl"),
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

    if segment_id.len() > 64
        || !segment_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
    {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let id = segment_id.strip_suffix(".ts").unwrap_or(&segment_id);

    let data = {
        let buf = state.hls_buffer.read().await;
        buf.get_segment(id).map(|d| d.to_vec())
    };

    match data {
        Some(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "video/mp2t"),
                (header::CACHE_CONTROL, "max-age=3600"),
            ],
            bytes,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn health() -> &'static str {
    "ok"
}
```

- [ ] **Step 2: Verify it compiles and tests pass**

Run: `cd /Users/worthy/TestCode/livestream-morphing/livestream-morphing-rs && cargo test`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
cd /Users/worthy/TestCode/livestream-morphing
git add livestream-morphing-rs/src/server.rs
git commit -m "feat: wire API routes into server with full CORS support"
```

---

### Task 8: Scaffold React App

**Files:**
- Create: `studio/` directory with Vite + React + TypeScript + Tailwind

- [ ] **Step 1: Initialize Vite project**

```bash
cd /Users/worthy/TestCode/livestream-morphing
npm create vite@latest studio -- --template react-ts
cd studio
npm install
```

- [ ] **Step 2: Install additional dependencies**

```bash
cd /Users/worthy/TestCode/livestream-morphing/studio
npm install hls.js @dnd-kit/core @dnd-kit/sortable @dnd-kit/utilities
npm install -D tailwindcss @tailwindcss/vite
```

- [ ] **Step 3: Configure Tailwind**

Replace `studio/src/index.css` with:

```css
@import "tailwindcss";
```

Update `studio/vite.config.ts`:

```typescript
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    port: 5173,
    proxy: {
      '/api': 'http://localhost:8080',
      '/stream': 'http://localhost:8080',
    },
  },
})
```

- [ ] **Step 4: Clean up scaffolded files**

Delete `studio/src/App.css`. Replace `studio/src/App.tsx` with a placeholder:

```tsx
function App() {
  return (
    <div className="min-h-screen bg-gray-950 text-white">
      <h1 className="text-2xl p-4">Morph Studio</h1>
    </div>
  )
}

export default App
```

- [ ] **Step 5: Verify it builds**

```bash
cd /Users/worthy/TestCode/livestream-morphing/studio
npm run build
```

Expected: Build succeeds, outputs to `studio/dist/`

- [ ] **Step 6: Commit**

```bash
cd /Users/worthy/TestCode/livestream-morphing
git add studio/
git commit -m "feat: scaffold React studio app with Vite, TypeScript, Tailwind"
```

---

### Task 9: TypeScript Types + API Hooks

**Files:**
- Create: `studio/src/types/index.ts`
- Create: `studio/src/hooks/useEffects.ts`
- Create: `studio/src/hooks/usePipeline.ts`
- Create: `studio/src/hooks/usePresets.ts`

- [ ] **Step 1: Create shared types**

Create `studio/src/types/index.ts`:

```typescript
export interface ParamDef {
  id: string
  name: string
  min: number
  max: number
  default: number
  step: number
}

export interface EffectDef {
  id: string
  name: string
  params: ParamDef[]
}

export interface PipelineSlot {
  slot_id: string
  effect_id: string
  params: Record<string, number>
  enabled: boolean
}

export interface PipelineEntry {
  effect_id: string
  params: Record<string, number>
  enabled: boolean
}

export interface PresetSummary {
  id: string
  name: string
}
```

- [ ] **Step 2: Create useEffects hook**

Create `studio/src/hooks/useEffects.ts`:

```typescript
import { useState, useEffect } from 'react'
import type { EffectDef } from '../types'

export function useEffects() {
  const [effects, setEffects] = useState<EffectDef[]>([])

  useEffect(() => {
    fetch('/api/effects')
      .then((r) => r.json())
      .then(setEffects)
      .catch(console.error)
  }, [])

  return effects
}
```

- [ ] **Step 3: Create usePipeline hook**

Create `studio/src/hooks/usePipeline.ts`:

```typescript
import { useState, useEffect, useCallback, useRef } from 'react'
import type { PipelineSlot, PipelineEntry } from '../types'

export function usePipeline() {
  const [slots, setSlots] = useState<PipelineSlot[]>([])
  const debounceTimers = useRef<Record<string, ReturnType<typeof setTimeout>>>({})

  const refresh = useCallback(() => {
    fetch('/api/pipeline')
      .then((r) => r.json())
      .then(setSlots)
      .catch(console.error)
  }, [])

  useEffect(() => { refresh() }, [refresh])

  const addEffect = useCallback(async (effectId: string) => {
    const res = await fetch(`/api/pipeline/add/${effectId}`, { method: 'POST' })
    const data = await res.json()
    setSlots(data)
  }, [])

  const removeSlot = useCallback(async (slotId: string) => {
    const res = await fetch(`/api/pipeline/${slotId}`, { method: 'DELETE' })
    const data = await res.json()
    setSlots(data)
  }, [])

  const updateParam = useCallback((slotId: string, paramId: string, value: number) => {
    // Optimistic update
    setSlots((prev) =>
      prev.map((s) =>
        s.slot_id === slotId
          ? { ...s, params: { ...s.params, [paramId]: value } }
          : s
      )
    )

    // Debounced API call
    const key = `${slotId}:${paramId}`
    if (debounceTimers.current[key]) {
      clearTimeout(debounceTimers.current[key])
    }
    debounceTimers.current[key] = setTimeout(async () => {
      await fetch(`/api/pipeline/${slotId}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ params: { [paramId]: value } }),
      })
      delete debounceTimers.current[key]
    }, 100)
  }, [])

  const setEnabled = useCallback(async (slotId: string, enabled: boolean) => {
    setSlots((prev) =>
      prev.map((s) => (s.slot_id === slotId ? { ...s, enabled } : s))
    )
    const res = await fetch(`/api/pipeline/${slotId}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled }),
    })
    const data = await res.json()
    setSlots(data)
  }, [])

  const reorder = useCallback(async (newSlots: PipelineSlot[]) => {
    setSlots(newSlots)
    const entries: PipelineEntry[] = newSlots.map((s) => ({
      effect_id: s.effect_id,
      params: s.params,
      enabled: s.enabled,
    }))
    const res = await fetch('/api/pipeline', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(entries),
    })
    const data = await res.json()
    setSlots(data)
  }, [])

  return { slots, addEffect, removeSlot, updateParam, setEnabled, reorder, refresh }
}
```

- [ ] **Step 4: Create usePresets hook**

Create `studio/src/hooks/usePresets.ts`:

```typescript
import { useState, useEffect, useCallback } from 'react'
import type { PresetSummary } from '../types'

export function usePresets() {
  const [presets, setPresets] = useState<PresetSummary[]>([])

  const refresh = useCallback(() => {
    fetch('/api/presets')
      .then((r) => r.json())
      .then(setPresets)
      .catch(console.error)
  }, [])

  useEffect(() => { refresh() }, [refresh])

  const savePreset = useCallback(async (name: string) => {
    await fetch('/api/presets', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name }),
    })
    refresh()
  }, [refresh])

  const applyPreset = useCallback(async (id: string) => {
    const res = await fetch(`/api/presets/${id}/apply`, { method: 'PUT' })
    return res.json()
  }, [])

  const deletePreset = useCallback(async (id: string) => {
    await fetch(`/api/presets/${id}`, { method: 'DELETE' })
    refresh()
  }, [refresh])

  return { presets, savePreset, applyPreset, deletePreset, refresh }
}
```

- [ ] **Step 5: Verify build**

```bash
cd /Users/worthy/TestCode/livestream-morphing/studio && npm run build
```

Expected: Build succeeds

- [ ] **Step 6: Commit**

```bash
cd /Users/worthy/TestCode/livestream-morphing
git add studio/src/types/ studio/src/hooks/
git commit -m "feat: TypeScript types and API hooks for effects, pipeline, presets"
```

---

### Task 10: Core UI Components (Slider, ParamPanel, EffectLibrary)

**Files:**
- Create: `studio/src/components/Slider.tsx`
- Create: `studio/src/components/ParamPanel.tsx`
- Create: `studio/src/components/EffectLibrary.tsx`

- [ ] **Step 1: Create Slider component**

Create `studio/src/components/Slider.tsx`:

```tsx
import type { ParamDef } from '../types'

interface SliderProps {
  param: ParamDef
  value: number
  onChange: (value: number) => void
}

export function Slider({ param, value, onChange }: SliderProps) {
  return (
    <div className="mb-4">
      <div className="flex justify-between mb-1">
        <span className="text-sm text-gray-300">{param.name}</span>
        <span className="text-sm font-bold text-indigo-400">
          {param.step >= 1 ? Math.round(value) : value.toFixed(2)}
        </span>
      </div>
      <input
        type="range"
        min={param.min}
        max={param.max}
        step={param.step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className="w-full h-1 bg-gray-700 rounded-lg appearance-none cursor-pointer accent-indigo-500"
      />
      <div className="flex justify-between mt-0.5">
        <span className="text-[10px] text-gray-500">{param.min}</span>
        <span className="text-[10px] text-gray-500">{param.max}</span>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Create ParamPanel component**

Create `studio/src/components/ParamPanel.tsx`:

```tsx
import { Slider } from './Slider'
import type { PipelineSlot, EffectDef } from '../types'

interface ParamPanelProps {
  slot: PipelineSlot | null
  effects: EffectDef[]
  onUpdateParam: (slotId: string, paramId: string, value: number) => void
  onRemove: (slotId: string) => void
}

export function ParamPanel({ slot, effects, onUpdateParam, onRemove }: ParamPanelProps) {
  if (!slot) {
    return (
      <div className="p-3 text-sm text-gray-500 italic">
        Select an effect in the pipeline to edit its parameters
      </div>
    )
  }

  const effectDef = effects.find((e) => e.id === slot.effect_id)
  if (!effectDef) return null

  return (
    <div className="p-3">
      <div className="text-xs uppercase tracking-wider text-gray-400 mb-3">
        {effectDef.name} — Parameters
      </div>
      {effectDef.params.map((param) => (
        <Slider
          key={param.id}
          param={param}
          value={slot.params[param.id] ?? param.default}
          onChange={(v) => onUpdateParam(slot.slot_id, param.id, v)}
        />
      ))}
      <div className="mt-5 pt-3 border-t border-gray-800">
        <button
          onClick={() => onRemove(slot.slot_id)}
          className="w-full py-2 text-sm text-red-300 bg-red-950 rounded hover:bg-red-900 transition-colors"
        >
          Remove from Pipeline
        </button>
      </div>
    </div>
  )
}
```

- [ ] **Step 3: Create EffectLibrary component**

Create `studio/src/components/EffectLibrary.tsx`:

```tsx
import type { EffectDef } from '../types'

interface EffectLibraryProps {
  effects: EffectDef[]
  onAdd: (effectId: string) => void
}

export function EffectLibrary({ effects, onAdd }: EffectLibraryProps) {
  return (
    <div className="p-3">
      <div className="text-xs uppercase tracking-wider text-gray-400 mb-3">
        Effect Library
      </div>
      {effects.map((effect) => (
        <button
          key={effect.id}
          onClick={() => onAdd(effect.id)}
          className="w-full text-left px-3 py-2 mb-1 text-sm text-gray-200 bg-gray-800 rounded border-l-2 border-indigo-500 hover:bg-gray-700 transition-colors"
        >
          {effect.name}
        </button>
      ))}
      <div className="mt-2 text-[11px] text-gray-500 text-center">
        click to add to pipeline
      </div>
    </div>
  )
}
```

- [ ] **Step 4: Verify build**

```bash
cd /Users/worthy/TestCode/livestream-morphing/studio && npm run build
```

Expected: Build succeeds

- [ ] **Step 5: Commit**

```bash
cd /Users/worthy/TestCode/livestream-morphing
git add studio/src/components/Slider.tsx studio/src/components/ParamPanel.tsx studio/src/components/EffectLibrary.tsx
git commit -m "feat: Slider, ParamPanel, and EffectLibrary components"
```

---

### Task 11: Pipeline Editor + Video Player

**Files:**
- Create: `studio/src/components/PipelineEditor.tsx`
- Create: `studio/src/components/VideoPlayer.tsx`

- [ ] **Step 1: Create PipelineEditor with drag-and-drop**

Create `studio/src/components/PipelineEditor.tsx`:

```tsx
import { DndContext, closestCenter, type DragEndEvent } from '@dnd-kit/core'
import { SortableContext, verticalListSortingStrategy, useSortable, arrayMove } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import type { PipelineSlot, EffectDef } from '../types'

interface PipelineEditorProps {
  slots: PipelineSlot[]
  effects: EffectDef[]
  selectedSlotId: string | null
  onSelect: (slotId: string) => void
  onToggle: (slotId: string, enabled: boolean) => void
  onReorder: (newSlots: PipelineSlot[]) => void
}

function SortableSlot({
  slot,
  effects,
  index,
  isSelected,
  onSelect,
  onToggle,
}: {
  slot: PipelineSlot
  effects: EffectDef[]
  index: number
  isSelected: boolean
  onSelect: () => void
  onToggle: () => void
}) {
  const { attributes, listeners, setNodeRef, transform, transition } = useSortable({
    id: slot.slot_id,
  })

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  }

  const effectName = effects.find((e) => e.id === slot.effect_id)?.name ?? slot.effect_id

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={`flex items-center gap-2 mb-1 px-2 py-1.5 rounded text-sm cursor-pointer transition-colors ${
        isSelected ? 'bg-indigo-900/50 border-l-[3px] border-indigo-500' : 'bg-gray-800 border-l-[3px] border-green-400'
      } ${!slot.enabled ? 'opacity-50' : ''}`}
      onClick={onSelect}
    >
      <span
        {...attributes}
        {...listeners}
        className="text-gray-500 cursor-grab text-[10px] select-none"
      >
        ⠿
      </span>
      <span className={`flex-1 ${!slot.enabled ? 'line-through text-gray-500' : 'text-gray-200'}`}>
        {index + 1}. {effectName}
      </span>
      <button
        onClick={(e) => { e.stopPropagation(); onToggle() }}
        className={`text-[10px] ${slot.enabled ? 'text-green-400' : 'text-red-400'}`}
      >
        ●
      </button>
    </div>
  )
}

export function PipelineEditor({
  slots,
  effects,
  selectedSlotId,
  onSelect,
  onToggle,
  onReorder,
}: PipelineEditorProps) {
  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event
    if (!over || active.id === over.id) return

    const oldIndex = slots.findIndex((s) => s.slot_id === active.id)
    const newIndex = slots.findIndex((s) => s.slot_id === over.id)
    const newSlots = arrayMove(slots, oldIndex, newIndex)
    onReorder(newSlots)
  }

  return (
    <div className="p-3">
      <div className="text-xs uppercase tracking-wider text-gray-400 mb-3">
        Pipeline Order
      </div>
      <DndContext collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={slots.map((s) => s.slot_id)} strategy={verticalListSortingStrategy}>
          {slots.map((slot, i) => (
            <SortableSlot
              key={slot.slot_id}
              slot={slot}
              effects={effects}
              index={i}
              isSelected={slot.slot_id === selectedSlotId}
              onSelect={() => onSelect(slot.slot_id)}
              onToggle={() => onToggle(slot.slot_id, !slot.enabled)}
            />
          ))}
        </SortableContext>
      </DndContext>
      {slots.length === 0 && (
        <div className="text-sm text-gray-500 italic">No effects — add from library</div>
      )}
      <div className="text-[10px] text-gray-500 mt-2">
        drag to reorder · click dot to toggle
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Create VideoPlayer component**

Create `studio/src/components/VideoPlayer.tsx`:

```tsx
import { useEffect, useRef } from 'react'
import Hls from 'hls.js'

export function VideoPlayer() {
  const videoRef = useRef<HTMLVideoElement>(null)

  useEffect(() => {
    const video = videoRef.current
    if (!video) return

    const src = '/api/stream'

    if (Hls.isSupported()) {
      const hls = new Hls({
        liveSyncDurationCount: 2,
        liveMaxLatencyDurationCount: 4,
        enableWorker: true,
      })
      hls.loadSource(src)
      hls.attachMedia(video)
      hls.on(Hls.Events.MANIFEST_PARSED, () => {
        video.play().catch(() => {})
      })
      return () => hls.destroy()
    } else if (video.canPlayType('application/vnd.apple.mpegurl')) {
      // Safari native HLS
      video.src = src
      video.addEventListener('loadedmetadata', () => {
        video.play().catch(() => {})
      })
    }
  }, [])

  return (
    <div className="flex-1 bg-black flex items-center justify-center relative">
      <video
        ref={videoRef}
        className="w-full h-full object-contain"
        muted
        playsInline
      />
      <div className="absolute top-2 left-2 bg-red-600 text-white text-[10px] px-2 py-0.5 rounded font-bold">
        LIVE
      </div>
    </div>
  )
}
```

- [ ] **Step 3: Verify build**

```bash
cd /Users/worthy/TestCode/livestream-morphing/studio && npm run build
```

Expected: Build succeeds

- [ ] **Step 4: Commit**

```bash
cd /Users/worthy/TestCode/livestream-morphing
git add studio/src/components/PipelineEditor.tsx studio/src/components/VideoPlayer.tsx
git commit -m "feat: PipelineEditor with drag-and-drop and HLS VideoPlayer"
```

---

### Task 12: PresetBar + App Layout

**Files:**
- Create: `studio/src/components/PresetBar.tsx`
- Modify: `studio/src/App.tsx`

- [ ] **Step 1: Create PresetBar component**

Create `studio/src/components/PresetBar.tsx`:

```tsx
import { useState } from 'react'
import type { PresetSummary } from '../types'

interface PresetBarProps {
  presets: PresetSummary[]
  onSave: (name: string) => void
  onApply: (id: string) => void
  onDelete: (id: string) => void
}

export function PresetBar({ presets, onSave, onApply, onDelete }: PresetBarProps) {
  const [showSave, setShowSave] = useState(false)
  const [name, setName] = useState('')

  const handleSave = () => {
    if (name.trim()) {
      onSave(name.trim())
      setName('')
      setShowSave(false)
    }
  }

  return (
    <div className="bg-gray-900/80 border-b border-gray-800 px-4 py-2 flex items-center gap-3">
      <span className="text-amber-500 font-bold text-sm">Morph Studio</span>

      <div className="flex-1" />

      <div className="flex items-center gap-2">
        {presets.map((p) => (
          <div key={p.id} className="flex items-center gap-1">
            <button
              onClick={() => onApply(p.id)}
              className="px-3 py-1 text-xs bg-gray-800 text-gray-300 rounded hover:bg-gray-700 transition-colors"
            >
              {p.name}
            </button>
            <button
              onClick={() => onDelete(p.id)}
              className="text-gray-600 hover:text-red-400 text-xs transition-colors"
            >
              ×
            </button>
          </div>
        ))}
      </div>

      {showSave ? (
        <div className="flex items-center gap-2">
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSave()}
            placeholder="Preset name..."
            className="px-2 py-1 text-xs bg-gray-800 text-white rounded border border-gray-700 focus:border-indigo-500 outline-none w-36"
            autoFocus
          />
          <button
            onClick={handleSave}
            className="px-3 py-1 text-xs bg-indigo-600 text-white rounded hover:bg-indigo-500"
          >
            Save
          </button>
          <button
            onClick={() => setShowSave(false)}
            className="text-xs text-gray-500 hover:text-gray-300"
          >
            Cancel
          </button>
        </div>
      ) : (
        <button
          onClick={() => setShowSave(true)}
          className="px-3 py-1 text-xs bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
        >
          Save Preset
        </button>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Wire up App.tsx**

Replace `studio/src/App.tsx` with the full layout:

```tsx
import { useState } from 'react'
import { useEffects } from './hooks/useEffects'
import { usePipeline } from './hooks/usePipeline'
import { usePresets } from './hooks/usePresets'
import { EffectLibrary } from './components/EffectLibrary'
import { VideoPlayer } from './components/VideoPlayer'
import { PipelineEditor } from './components/PipelineEditor'
import { ParamPanel } from './components/ParamPanel'
import { PresetBar } from './components/PresetBar'

function App() {
  const effects = useEffects()
  const { slots, addEffect, removeSlot, updateParam, setEnabled, reorder, refresh } = usePipeline()
  const { presets, savePreset, applyPreset, deletePreset } = usePresets()
  const [selectedSlotId, setSelectedSlotId] = useState<string | null>(null)

  const selectedSlot = slots.find((s) => s.slot_id === selectedSlotId) ?? null

  const handleApplyPreset = async (id: string) => {
    await applyPreset(id)
    refresh()
    setSelectedSlotId(null)
  }

  return (
    <div className="h-screen flex flex-col bg-gray-950 text-white overflow-hidden">
      <PresetBar
        presets={presets}
        onSave={savePreset}
        onApply={handleApplyPreset}
        onDelete={deletePreset}
      />

      <div className="flex flex-1 min-h-0">
        {/* Left: Effect Library */}
        <div className="w-[200px] bg-gray-900/50 border-r border-gray-800 overflow-y-auto">
          <EffectLibrary effects={effects} onAdd={addEffect} />
        </div>

        {/* Center: Video Player */}
        <VideoPlayer />

        {/* Right: Pipeline + Params */}
        <div className="w-[280px] bg-gray-900/50 border-l border-gray-800 flex flex-col">
          <div className="border-b border-gray-800 overflow-y-auto max-h-[50%]">
            <PipelineEditor
              slots={slots}
              effects={effects}
              selectedSlotId={selectedSlotId}
              onSelect={setSelectedSlotId}
              onToggle={setEnabled}
              onReorder={reorder}
            />
          </div>
          <div className="flex-1 overflow-y-auto">
            <ParamPanel
              slot={selectedSlot}
              effects={effects}
              onUpdateParam={updateParam}
              onRemove={(id) => {
                removeSlot(id)
                if (selectedSlotId === id) setSelectedSlotId(null)
              }}
            />
          </div>
        </div>
      </div>
    </div>
  )
}

export default App
```

- [ ] **Step 3: Verify build**

```bash
cd /Users/worthy/TestCode/livestream-morphing/studio && npm run build
```

Expected: Build succeeds

- [ ] **Step 4: Commit**

```bash
cd /Users/worthy/TestCode/livestream-morphing
git add studio/src/components/PresetBar.tsx studio/src/App.tsx
git commit -m "feat: PresetBar component and full App layout"
```

---

### Task 13: Production Build — Static File Serving

**Files:**
- Modify: `livestream-morphing-rs/Cargo.toml`
- Modify: `livestream-morphing-rs/src/server.rs`

- [ ] **Step 1: Add tower-http serve-dir feature**

The existing `tower-http` dependency already has `cors`. Add `fs` feature for `ServeDir`:

In `Cargo.toml`, change:

```toml
tower-http = { version = "0.6", features = ["cors"] }
```

To:

```toml
tower-http = { version = "0.6", features = ["cors", "fs"] }
```

- [ ] **Step 2: Add static file fallback to server.rs**

In `server.rs`, add the import and fallback at the end of the router chain. Add this import at the top:

```rust
use tower_http::services::{ServeDir, ServeFile};
```

In the `router()` function, add a fallback for serving the React app. After the `.layer(cors)` line and before `.with_state(state)`, add:

```rust
.fallback_service(
    ServeDir::new("../studio/dist")
        .not_found_service(ServeFile::new("../studio/dist/index.html")),
)
```

This serves static files from the built React app, with SPA fallback (all unmatched routes serve `index.html` so React Router can handle them).

- [ ] **Step 3: Build the React app and test**

```bash
cd /Users/worthy/TestCode/livestream-morphing/studio && npm run build
cd /Users/worthy/TestCode/livestream-morphing/livestream-morphing-rs && cargo check
```

Expected: Both compile/build successfully

- [ ] **Step 4: Commit**

```bash
cd /Users/worthy/TestCode/livestream-morphing
git add livestream-morphing-rs/Cargo.toml livestream-morphing-rs/src/server.rs
git commit -m "feat: serve React studio as static files from Rust backend"
```

---

### Task 14: End-to-End Verification

- [ ] **Step 1: Build everything**

```bash
cd /Users/worthy/TestCode/livestream-morphing/studio && npm run build
cd /Users/worthy/TestCode/livestream-morphing/livestream-morphing-rs && cargo build
```

Expected: Both build successfully

- [ ] **Step 2: Run all Rust tests**

```bash
cd /Users/worthy/TestCode/livestream-morphing/livestream-morphing-rs && cargo test
```

Expected: All tests pass (registry, quantize, distortion, edges, canvas_texture, blur, color_shift, pipeline, codec, hls, stream_source, time_color)

- [ ] **Step 3: Add .superpowers to .gitignore**

```bash
cd /Users/worthy/TestCode/livestream-morphing
echo ".superpowers/" >> .gitignore
```

- [ ] **Step 4: Final commit**

```bash
cd /Users/worthy/TestCode/livestream-morphing
git add .gitignore
git commit -m "chore: add .superpowers to gitignore"
```
