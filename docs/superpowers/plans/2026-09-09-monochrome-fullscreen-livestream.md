# Monochrome Fullscreen Livestream Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore the Abbey Road stream’s monochrome line-art treatment and make it the full opening viewport of its portfolio page before the existing project text.

**Architecture:** Add a registered, parameter-free grayscale effect to the Rust processor and place it between distortion and quantization in the default pipeline. In the portfolio, make the existing live player optionally fill its parent, then add a livestream-only breakout hero that spans the viewport while leaving every other detail page on the current shared layout.

**Tech Stack:** Rust, inventory effect registry, Tokio; React 18, TypeScript, Tailwind CSS, Vitest and Testing Library; Railway.

**Spec:** `docs/superpowers/specs/2026-09-09-monochrome-fullscreen-livestream-design.md`

## Global Constraints

- Preserve immediate prerecorded playback, readiness polling, crossfade, retry, and cleanup behavior.
- Preserve the existing navigation, typography, black background, project metadata, and melt transition.
- Apply the fullscreen composition only to `/art/livestream-art`.
- Keep the current wake-on-view Railway architecture, resolution, bitrate, public routes, and cost controls unchanged.
- Introduce no new dependencies, credentials, AWS resources, Cloudflare configuration, or services.
- Use the existing branch `worthy/09_09_2026/portfolio_live_stream` in both worktrees.

---

### Task 1: Add the Rust grayscale effect

**Files:**
- Create: `livestream-morphing-rs/src/effects/grayscale.rs`
- Modify: `livestream-morphing-rs/src/effects/mod.rs`

**Interfaces:**
- Consumes: `Effect`, `FrameCtx`, `ParamDef`, `ParamValues`, and `RawFrame` from the existing effect framework.
- Produces: a registered effect with ID `grayscale`, name `Grayscale`, no parameters, and in-place RGB luminance conversion.

- [ ] **Step 1: Write the failing effect test**

Create `livestream-morphing-rs/src/effects/grayscale.rs` with the effect shell and this test first:

```rust
use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

#[derive(Default)]
pub struct Grayscale;

impl Effect for Grayscale {
    fn id(&self) -> &'static str { "grayscale" }
    fn name(&self) -> &'static str { "Grayscale" }
    fn params(&self) -> Vec<ParamDef> { vec![] }
    fn init(&mut self, _width: u32, _height: u32) {}
    fn apply(&mut self, _frame: &mut RawFrame, _params: &ParamValues, _ctx: &FrameCtx) {
        // Deliberately empty until the failing test is observed.
    }
}

crate::register_effect!(Grayscale);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_rgb_pixels_to_equal_luminance_channels() {
        let mut effect = Grayscale;
        let mut frame = RawFrame::new(2, 1);
        frame.data = vec![255, 0, 0, 0, 255, 0];
        let ctx = FrameCtx { frame_number: 0, width: 2, height: 1 };

        effect.apply(&mut frame, &ParamValues::new(), &ctx);

        assert_eq!(frame.data, vec![76, 76, 76, 149, 149, 149]);
    }
}
```

Add `pub mod grayscale;` to `livestream-morphing-rs/src/effects/mod.rs` so the registration is linked.

- [ ] **Step 2: Run the focused test and verify it fails**

Run:

```bash
cargo test --manifest-path livestream-morphing-rs/Cargo.toml grayscale::tests::converts_rgb_pixels_to_equal_luminance_channels
```

Expected: FAIL because the input RGB channels remain unequal.

- [ ] **Step 3: Implement the minimal luminance conversion**

Replace the empty `apply` body with:

```rust
fn apply(&mut self, frame: &mut RawFrame, _params: &ParamValues, _ctx: &FrameCtx) {
    for pixel in frame.data.chunks_exact_mut(3) {
        let r = pixel[0] as u16;
        let g = pixel[1] as u16;
        let b = pixel[2] as u16;
        let luminance = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
        pixel.fill(luminance);
    }
}
```

- [ ] **Step 4: Run the effect tests**

Run:

```bash
cargo test --manifest-path livestream-morphing-rs/Cargo.toml effects::grayscale
```

Expected: PASS.

- [ ] **Step 5: Commit the effect**

```bash
git add livestream-morphing-rs/src/effects/grayscale.rs livestream-morphing-rs/src/effects/mod.rs
git commit -m "feat: add grayscale stream effect"
```

---

### Task 2: Make monochrome part of the production default pipeline

**Files:**
- Modify: `livestream-morphing-rs/src/pipeline.rs`

**Interfaces:**
- Consumes: registered effect ID `grayscale` from Task 1.
- Produces: `AppState::new()` default effect order `distortion`, `grayscale`, `quantize`, `edges`, `canvas_texture`.

- [ ] **Step 1: Write a failing default-order test**

Add this test inside `pipeline.rs`’s existing `tests` module:

```rust
#[test]
fn default_pipeline_is_monochrome_before_quantization_and_edges() {
    let (state, _rx) = AppState::new();
    let effect_ids: Vec<String> = state
        .pipeline
        .lock()
        .unwrap()
        .view()
        .into_iter()
        .map(|slot| slot.effect_id)
        .collect();

    assert_eq!(
        effect_ids,
        ["distortion", "grayscale", "quantize", "edges", "canvas_texture"]
    );
}
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```bash
cargo test --manifest-path livestream-morphing-rs/Cargo.toml default_pipeline_is_monochrome_before_quantization_and_edges
```

Expected: FAIL because `grayscale` is absent.

- [ ] **Step 3: Add grayscale to `AppState::new()`**

Insert the new stage after distortion:

```rust
p.add_effect("distortion").expect("distortion registered");
p.add_effect("grayscale").expect("grayscale registered");
p.add_effect("quantize").expect("quantize registered");
p.add_effect("edges").expect("edges registered");
p.add_effect("canvas_texture").expect("canvas_texture registered");
```

- [ ] **Step 4: Run focused and full Rust tests**

Run:

```bash
cargo test --manifest-path livestream-morphing-rs/Cargo.toml default_pipeline_is_monochrome_before_quantization_and_edges
cargo test --manifest-path livestream-morphing-rs/Cargo.toml
```

Expected: all tests PASS.

- [ ] **Step 5: Commit the default pipeline change**

```bash
git add livestream-morphing-rs/src/pipeline.rs
git commit -m "feat: make live stream monochrome"
```

---

### Task 3: Let the existing React player fill a hero container

Run this task in `/Users/worthy/Test/personal/.worktrees/portfolio_live_stream`.

**Files:**
- Modify: `src/components/LiveStream/LiveStreamPlayer.tsx`
- Modify: `src/components/LiveStream/ArtMedia.tsx`
- Modify: `src/components/LiveStream/__tests__/LiveStreamPlayer.test.tsx`
- Modify: `src/components/LiveStream/__tests__/ArtMedia.test.tsx`

**Interfaces:**
- Produces: `LiveStreamPlayerProps.fill?: boolean`; `fill=true` uses a `h-full w-full` root instead of `aspect-video`.
- Produces: `ArtMedia({ piece, fill? })`; the optional flag is forwarded only to the livestream player.
- Preserves: all current playback state and lifecycle behavior.

- [ ] **Step 1: Write failing player layout tests**

Append to `LiveStreamPlayer.test.tsx`:

```tsx
it('fills its parent when requested without forcing an aspect ratio', () => {
  vi.mocked(fetch).mockImplementation(() => new Promise(() => undefined))

  render(<LiveStreamPlayer fallbackUrl={fallbackUrl} baseUrl={baseUrl} fill />)

  const root = screen.getByTestId('live-stream-player')
  expect(root).toHaveClass('h-full', 'w-full')
  expect(root).not.toHaveClass('aspect-video')
})

it('keeps the inline aspect ratio by default', () => {
  vi.mocked(fetch).mockImplementation(() => new Promise(() => undefined))

  render(<LiveStreamPlayer fallbackUrl={fallbackUrl} baseUrl={baseUrl} />)

  expect(screen.getByTestId('live-stream-player')).toHaveClass('aspect-video')
})
```

Update the `LiveStreamPlayer` mock in `ArtMedia.test.tsx` to record `fill`, then add:

```tsx
it('forwards the fill layout to livestream-art', () => {
  render(<ArtMedia piece={getArtPiece('livestream-art')!} fill />)

  expect(screen.getByTestId('mock-live-player')).toHaveAttribute('data-fill', 'true')
})
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run:

```bash
npm test -- src/components/LiveStream/__tests__/LiveStreamPlayer.test.tsx src/components/LiveStream/__tests__/ArtMedia.test.tsx
```

Expected: TypeScript/test failures because `fill` and `live-stream-player` do not exist.

- [ ] **Step 3: Implement the optional fill layout**

Add `fill?: boolean` to `LiveStreamPlayerProps`, default it to `false`, and change the root to:

```tsx
<div
  data-testid="live-stream-player"
  className={`relative overflow-hidden bg-black ${fill ? 'h-full w-full' : 'aspect-video'}`}
>
```

Update `ArtMedia`’s signature and live branch:

```tsx
export function ArtMedia({ piece, fill = false }: { piece: ArtPiece; fill?: boolean }): JSX.Element | null {
  if (piece.slug === 'livestream-art') {
    return (
      <LiveStreamPlayer
        fallbackUrl={piece.videoUrl!}
        baseUrl={import.meta.env.VITE_LIVE_STREAM_URL}
        fill={fill}
      />
    )
  }
```

- [ ] **Step 4: Run the focused tests and build**

Run:

```bash
npm test -- src/components/LiveStream/__tests__/LiveStreamPlayer.test.tsx src/components/LiveStream/__tests__/ArtMedia.test.tsx
npm run build
```

Expected: PASS.

- [ ] **Step 5: Commit the reusable player layout**

```bash
git add src/components/LiveStream/LiveStreamPlayer.tsx src/components/LiveStream/ArtMedia.tsx src/components/LiveStream/__tests__/LiveStreamPlayer.test.tsx src/components/LiveStream/__tests__/ArtMedia.test.tsx
git commit -m "feat: support fullscreen live player layout"
```

---

### Task 4: Add the livestream-only cinematic hero

Run this task in `/Users/worthy/Test/personal/.worktrees/portfolio_live_stream`.

**Files:**
- Create: `src/components/LiveStream/LivestreamArtHero.tsx`
- Create: `src/components/LiveStream/__tests__/LivestreamArtHero.test.tsx`
- Modify: `src/pages/Home.tsx`

**Interfaces:**
- Consumes: `ArtMedia({ piece, fill: true })` from Task 3.
- Produces: `LivestreamArtHero({ piece }: { piece: ArtPiece })`, a viewport-width breakout with height `calc(100dvh - 100px)`.
- Preserves: the existing `DetailOverlay` and all existing header, summary, tools, link, and transition markup.

- [ ] **Step 1: Write the failing hero component test**

Create `LivestreamArtHero.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { getArtPiece } from '@/lib/art'
import { LivestreamArtHero } from '../LivestreamArtHero'

vi.mock('../ArtMedia', () => ({
  ArtMedia: ({ fill }: { fill?: boolean }) => (
    <div data-testid="mock-art-media" data-fill={String(fill)} />
  ),
}))

describe('LivestreamArtHero', () => {
  it('breaks out to viewport width and gives the player the opening viewport', () => {
    render(<LivestreamArtHero piece={getArtPiece('livestream-art')!} />)

    expect(screen.getByTestId('livestream-art-hero')).toHaveClass(
      'w-screen',
      'h-[calc(100dvh-100px)]',
    )
    expect(screen.getByTestId('mock-art-media')).toHaveAttribute('data-fill', 'true')
  })
})
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```bash
npm test -- src/components/LiveStream/__tests__/LivestreamArtHero.test.tsx
```

Expected: FAIL because `LivestreamArtHero` does not exist.

- [ ] **Step 3: Implement the hero**

Create `LivestreamArtHero.tsx`:

```tsx
import type { ArtPiece } from '@/lib/art'
import { ArtMedia } from './ArtMedia'

export function LivestreamArtHero({ piece }: { piece: ArtPiece }) {
  return (
    <section
      data-testid="livestream-art-hero"
      aria-label="Abbey Road live artwork"
      className="relative left-1/2 -mt-5 h-[calc(100dvh-100px)] min-h-[360px] w-screen -translate-x-1/2 overflow-hidden bg-black"
    >
      <ArtMedia piece={piece} fill />
    </section>
  )
}
```

- [ ] **Step 4: Place the hero before the existing art text**

Import `LivestreamArtHero` in `Home.tsx`. Inside the existing art `DetailOverlay`, compute the branch directly from `displayArtPiece.slug`:

```tsx
{displayArtPiece.slug === 'livestream-art' && (
  <LivestreamArtHero piece={displayArtPiece} />
)}
```

Place it immediately before the existing title/header block. Give that header block `mt-16` only for the livestream piece:

```tsx
<div className={`flex items-start justify-between gap-4 ${
  displayArtPiece.slug === 'livestream-art' ? 'mt-16' : ''
}`}>
```

Render the existing inline media block only for other art pieces:

```tsx
{displayArtPiece.slug !== 'livestream-art' && (
  <div className="mt-8">
    <ArtMedia piece={displayArtPiece} />
  </div>
)}
```

Do not alter the project detail branch, `DetailOverlay`, navigation canvas, summary, tools, GitHub link, or melt cover canvas.

- [ ] **Step 5: Run focused tests, the full suite, and the build**

Run:

```bash
npm test -- src/components/LiveStream/__tests__/LivestreamArtHero.test.tsx src/components/LiveStream/__tests__/ArtMedia.test.tsx src/components/LiveStream/__tests__/LiveStreamPlayer.test.tsx
npm test
npm run build
npx eslint src/components/LiveStream src/pages/Home.tsx
git diff --check
```

Expected: all tests and the build pass. The focused eslint run may still report the already-recorded pre-existing hook dependency warning in `Home.tsx`; no new lint errors are acceptable.

- [ ] **Step 6: Commit the cinematic hero**

```bash
git add src/components/LiveStream/LivestreamArtHero.tsx src/components/LiveStream/__tests__/LivestreamArtHero.test.tsx src/pages/Home.tsx
git commit -m "feat: open livestream art fullscreen"
```

---

### Task 5: Verify and deploy both services

**Files:**
- No source changes expected.
- Do not commit generated `tsconfig.tsbuildinfo`.

**Interfaces:**
- Consumes: monochrome stream commits from Tasks 1–2 and portfolio commits from Tasks 3–4.
- Produces: verified production deployments of Railway services `livestream-morphing` and `personal` in project `lavish-energy`.

- [ ] **Step 1: Run final stream verification**

From `/Users/worthy/Test/livestream-morphing/.worktrees/portfolio_live_stream`:

```bash
cargo fmt --manifest-path livestream-morphing-rs/Cargo.toml -- --check
cargo test --manifest-path livestream-morphing-rs/Cargo.toml
cargo clippy --manifest-path livestream-morphing-rs/Cargo.toml --all-targets
git diff --check
git status --short
```

Expected: formatting and tests pass, no new clippy warnings, and the worktree is clean.

- [ ] **Step 2: Run final portfolio verification**

From `/Users/worthy/Test/personal/.worktrees/portfolio_live_stream`:

```bash
npm test
npm run build
npx eslint src/components/LiveStream src/pages/Home.tsx
git diff --check
git status --short
```

Expected: tests and build pass; only the known generated `tsconfig.tsbuildinfo` modification may remain and must not be committed.

- [ ] **Step 3: Deploy the stream service using the existing Railway session**

```bash
railway status
railway up --service livestream-morphing --environment production --detach
railway deployment list --service livestream-morphing
```

Wait for the new deployment to report `SUCCESS`, then confirm:

```bash
curl -fsS https://live.worthyrae.com/health
curl -fsS https://live.worthyrae.com/api/live/status
```

Do not list Railway variables or print credentials.

- [ ] **Step 4: Deploy the portfolio service using the existing Railway session**

From the personal worktree:

```bash
railway status
railway up --service personal --environment production --detach
railway deployment list --service personal
```

Wait for the new deployment to report `SUCCESS`, then confirm `https://worthyrae.com/art/livestream-art` returns HTTP 200.

- [ ] **Step 5: Perform production browser acceptance**

Open `https://worthyrae.com/art/livestream-art` in a Chromium browser and Safari. Verify:

1. The prerecorded Abbey Road image appears immediately.
2. The opening video fills the available viewport beneath the unchanged site navigation.
3. Scrolling moves the hero completely offscreen and reveals the existing title, description, summary, tools, and repository link on black.
4. The starting label changes to `LIVE` after the stream is ready.
5. The live image contains no colored pixels by visual inspection and retains stepped gray tones and visible edge lines.
6. Returning home and reopening the route preserves the existing melt transition and starts playback again.

If the feed is asleep, allow the existing 90-second startup window before diagnosing a failure. If either service fails acceptance, stop and redeploy the last known good commit for that service; do not change infrastructure.

- [ ] **Step 6: Record final deployment IDs and repository status**

Run `railway status` in each worktree and record the successful deployment IDs in the completion response. Also report exact test totals, build status, any pre-existing lint/clippy warnings, and whether each worktree is clean.

