# Morph Studio — Design Spec

A web-based creative studio for real-time visual effect experimentation on the Abbey Road livestream.

## Goals

- **Experimentation infrastructure** — make it trivial to create, compose, tune, and save visual effect pipelines without recompiling for parameter changes
- **Full studio UI** — React app with effect library, drag-and-drop pipeline editor, parameter sliders, preset management, and live HLS preview
- **Extensible effect system** — trait-based effects with a registration macro so new effects auto-appear in the UI after a Rust recompile

## Architecture

### Repo Structure

```
livestream-morphing/
├── livestream-morphing-rs/          # Rust backend (existing, extended)
│   ├── src/
│   │   ├── effects/                 # one file per effect (refactored from effects.rs)
│   │   │   ├── mod.rs
│   │   │   ├── quantize.rs
│   │   │   ├── distortion.rs
│   │   │   ├── edges.rs
│   │   │   ├── canvas_texture.rs
│   │   │   ├── blur.rs             # new starter effects
│   │   │   └── color_shift.rs
│   │   ├── registry.rs             # Effect trait + macro + global registry
│   │   ├── pipeline.rs             # dynamic pipeline engine (replaces FrameProcessor)
│   │   ├── api.rs                  # REST endpoints for studio
│   │   ├── server.rs               # existing HLS server + static file serving
│   │   ├── codec.rs                # existing
│   │   ├── stream_source.rs        # existing
│   │   ├── hls.rs                  # existing
│   │   ├── time_color.rs           # existing
│   │   └── main.rs
│   ├── presets/                     # saved preset TOML files
│   └── Cargo.toml
└── studio/                          # React frontend (new)
    ├── src/
    │   ├── App.tsx
    │   ├── components/
    │   │   ├── EffectLibrary.tsx
    │   │   ├── VideoPlayer.tsx
    │   │   ├── PipelineEditor.tsx
    │   │   ├── ParamPanel.tsx
    │   │   ├── PresetBar.tsx
    │   │   └── Slider.tsx
    │   ├── hooks/
    │   │   ├── useEffects.ts
    │   │   ├── usePipeline.ts
    │   │   └── usePresets.ts
    │   └── types/
    │       └── index.ts
    ├── index.html
    ├── vite.config.ts
    ├── tailwind.config.ts
    ├── tsconfig.json
    └── package.json
```

### System Data Flow

1. Abbey Road MPEG-TS stream → Rust decodes to RGB frames
2. Downsample 2x (960x540)
3. Pipeline engine applies effects in order (each enabled slot runs its `Effect::apply`)
4. Upsample 2x (1920x1080)
5. Encode H.264 → HLS segments → served via existing endpoints
6. React studio fetches HLS playlist and plays via `hls.js`
7. User drags slider → debounced REST PATCH → Rust updates pipeline params → next frame uses new values → visible in HLS ~2-4 seconds later

### Communication

- React ↔ Rust via REST API (JSON)
- Dev: Vite dev server proxies `/api/*` and `/stream/*` to Rust backend (port 3000)
- Production: `vite build` output embedded in Rust binary via `include_dir` crate — single binary serves both API and static frontend

## Effect System

### Effect Trait

```rust
trait Effect: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn params(&self) -> Vec<ParamDef>;
    fn init(&mut self, width: u32, height: u32);
    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, ctx: &FrameCtx);
}
```

### ParamDef

```rust
struct ParamDef {
    id: String,       // "amplitude"
    name: String,     // "Distortion Amplitude"
    min: f32,         // 0.0
    max: f32,         // 0.2
    default: f32,     // 0.02
    step: f32,        // 0.005
}
```

All parameters are `f32`. Integer params use `step: 1.0`. Boolean toggles use `min: 0.0, max: 1.0, step: 1.0`. This keeps the UI uniform — every parameter is a slider.

### FrameCtx

```rust
struct FrameCtx {
    frame_number: u32,
    width: u32,
    height: u32,
}
```

Shared context passed to every effect. Enables time-based animation (distortion cycles) without global state.

### Registration Macro

```rust
// effects/blur.rs
pub struct Blur;

impl Effect for Blur {
    fn id(&self) -> &str { "blur" }
    fn name(&self) -> &str { "Gaussian Blur" }
    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("radius", "Blur Radius", 0.0, 20.0, 3.0, 0.5)]
    }
    fn init(&mut self, _w: u32, _h: u32) {}
    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let radius = params.get("radius");
        // blur implementation
    }
}

register_effect!(Blur);
```

The `register_effect!` macro adds the effect to a global registry using the `inventory` crate for automatic collection. At startup, all registered effects are collected and available for the API to enumerate.

### Multiple Instances

The pipeline holds effect *instances*, not types. Adding "Quantize" twice creates two independent slots, each with their own parameter values and scratch buffers.

## Pipeline Engine

### PipelineSlot

```rust
struct PipelineSlot {
    slot_id: String,              // UUID — stable across reorders
    effect_id: String,            // which effect type (e.g. "quantize")
    effect: Box<dyn Effect>,      // the instance
    params: ParamValues,          // current parameter values
    enabled: bool,                // toggle without removing
}
```

### Pipeline

```rust
struct Pipeline {
    slots: Vec<PipelineSlot>,
}

impl Pipeline {
    fn process_frame(&mut self, frame: &mut RawFrame, frame_number: u32) {
        let ctx = FrameCtx { frame_number, width: frame.width, height: frame.height };
        for slot in &mut self.slots {
            if slot.enabled {
                slot.effect.apply(frame, &slot.params, &ctx);
            }
        }
    }
}
```

### Thread Safety

The pipeline is shared between the processing thread and Axum's async API handlers via `Arc<Mutex<Pipeline>>`. The mutex is held briefly:

- **API writes**: lock → update params or swap slot list → unlock
- **Frame processing**: lock → clone current params snapshot → unlock → process frame with snapshot

This keeps the lock duration minimal and prevents API calls from blocking frame processing.

## REST API

### Effect Library

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/effects` | List all registered effects with their parameter definitions |

Response: `[{ id, name, params: [{ id, name, min, max, default, step }] }]`

### Pipeline

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/pipeline` | Get current pipeline (ordered slots with params and enabled state) |
| PUT | `/api/pipeline` | Replace entire pipeline (reorder, add, remove) |
| PATCH | `/api/pipeline/:slot_id` | Update one slot's params or enabled state |

GET response: `[{ slot_id, effect_id, params: { key: value }, enabled }]`

PUT body: `[{ effect_id, params, enabled }]` — new slot_ids are generated server-side

PATCH body: `{ params?: { key: value }, enabled?: bool }`

### Presets

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/presets` | List saved presets |
| POST | `/api/presets` | Save current pipeline as a new preset |
| PUT | `/api/presets/:id/apply` | Load a preset into the active pipeline |
| DELETE | `/api/presets/:id` | Delete a preset |

### Existing Endpoints (unchanged)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/stream/playlist.m3u8` | HLS playlist |
| GET | `/stream/segment_N.ts` | HLS video segments |

## Preset Storage

Presets are TOML files in `livestream-morphing-rs/presets/`. Each preset is a pipeline snapshot:

```toml
name = "Psychedelic v1"

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

Human-readable, version-controllable, shareable.

## Studio UI

### Layout

Three-panel layout:

- **Left (200px): Effect Library** — lists all registered effects. Click to add to pipeline.
- **Center: HLS Video Player** — live stream via `hls.js` with low-latency config. LIVE indicator.
- **Right (280px): Pipeline + Parameters**
  - **Top: Pipeline Editor** — drag-and-drop ordered list (`@dnd-kit/core`). Grip handle to reorder, colored dot to toggle enabled/disabled. Click to select.
  - **Bottom: Parameter Panel** — sliders for the selected effect's parameters. Min/max labels, current value display. "Remove from Pipeline" button.
- **Top bar: Preset Manager** — current preset name, Save Preset button, Load dropdown.

### Tech Stack

- React 18+ with TypeScript
- Vite for build/dev
- Tailwind CSS for styling
- `hls.js` for HLS playback
- `@dnd-kit/core` + `@dnd-kit/sortable` for drag-and-drop pipeline reordering

### Component Tree

| Component | Purpose |
|-----------|---------|
| `App.tsx` | Layout shell, top-level state coordination |
| `EffectLibrary.tsx` | Left panel — fetches effect list, click to add |
| `VideoPlayer.tsx` | Center — HLS.js player pointing at `/stream/playlist.m3u8` |
| `PipelineEditor.tsx` | Right top — sortable list of pipeline slots |
| `ParamPanel.tsx` | Right bottom — sliders for selected slot's params |
| `PresetBar.tsx` | Top bar — preset name, save/load |
| `Slider.tsx` | Reusable slider with label, value display, min/max |

### Hooks

| Hook | Purpose |
|------|---------|
| `useEffects.ts` | `GET /api/effects` — cached effect library |
| `usePipeline.ts` | `GET/PUT /api/pipeline`, `PATCH` params — debounced slider updates |
| `usePresets.ts` | CRUD for `/api/presets` |

### Dev Workflow

1. Terminal 1: `cd livestream-morphing-rs && cargo run` (backend on port 3000)
2. Terminal 2: `cd studio && npm run dev` (Vite on port 5173, proxies `/api/*` and `/stream/*` to 3000)
3. Open `http://localhost:5173` — React app with live API proxying

### Production Build

1. `cd studio && npm run build` → outputs to `studio/dist/`
2. Rust binary uses `include_dir!("../studio/dist")` to embed static files
3. Axum serves embedded files as fallback route (SPA routing)
4. Single binary deployment — no separate frontend server needed

## Effects to Ship With

### Existing (refactored from effects.rs)

1. **Distortion** — sine-wave coordinate remapping with bilinear interpolation
2. **Quantize** — color channel posterization to N levels
3. **Edge Detect** — Sobel edge detection with dark overlay
4. **Canvas Texture** — procedural canvas-weave multiply blend

### New Starter Effects

5. **Blur** — box or Gaussian blur with configurable radius (addresses the "noisy" feedback — smoothing before quantization creates cleaner color fields)
6. **Color Shift** — hue rotation, saturation, and brightness adjustment

### Default Pipeline

The default pipeline on first run matches the current behavior: Distortion → Quantize → Edge Detect → Canvas Texture. Saved as `presets/default.toml`.

## Migration Strategy

The existing `FrameProcessor` struct and monolithic `effects.rs` are replaced by:
- `effects/` directory with individual effect files
- `registry.rs` with the `Effect` trait and macro
- `pipeline.rs` with the dynamic `Pipeline` engine

The existing pipeline behavior is preserved by the default preset. All current tests are adapted to use the new trait-based effects.
