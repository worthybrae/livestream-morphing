# Portfolio Live Stream Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `/art/livestream-art` wake an on-demand transformed Abbey Road stream, show the existing recording during startup or failure, and deliver live HLS through Railway and Cloudflare for no more than $5 per month at the expected usage.

**Architecture:** The Rust service remains the stateful ingest, effects, encoding, and in-memory HLS origin. Railway Serverless wakes it on the portfolio's public status request and sleeps it after the existing viewer idle period; Cloudflare caches immutable segments. A focused React player keeps the prerecorded MP4 mounted, manages readiness and HLS playback, and crossfades only after live media can play.

**Tech Stack:** Rust 2021, Axum 0.8, Tokio, FFmpeg/libx264, React 18, TypeScript, Vite, Vitest, Testing Library, `hls.js`, Railway, Cloudflare CDN.

**Spec:** `docs/superpowers/specs/2026-09-09-portfolio-live-stream-design.md`

## Global Constraints

- Design for one to five simultaneous viewers and a $5 monthly infrastructure ceiling.
- Opening `/art/livestream-art` starts the live system without an additional click.
- Keep the existing `abbey_road_best.mp4` recording visible until live playback emits `canplay`, and restore it on live failure.
- Use muted, inline playback and ship no live audio in this version.
- Require three fresh six-second HLS segments before reporting ready.
- Treat startup lasting more than 90 seconds as unavailable and keep retrying with capped backoff.
- Stop processing five minutes after the final viewer request, then allow Railway Serverless to sleep.
- Do not use AWS compute or Cloudflare R2 unless the final benchmark fails after one frame-rate or dimension tuning pass.
- Do not expose the existing mutable pipeline, source, effect, or preset endpoints on the production public router.
- Allow only `https://worthyrae.com` as the production browser origin; local development uses `http://localhost:5176`.
- Use `live.worthyrae.com` as the public stream origin and `VITE_LIVE_STREAM_URL` as the portfolio build-time setting.
- Create the portfolio branch as `worthy/09_09_2026/portfolio_live_stream`; the stream repository is already on that branch.
- Keep commits repository-local: never combine paths from both repositories in one commit command.

---

## File Structure

### `livestream-morphing`

- Modify `livestream-morphing-rs/src/pipeline.rs`: own live readiness, session identity, timestamps, and viewer lease state.
- Modify `livestream-morphing-rs/src/server.rs`: define the production-safe router, public status response, cache headers, and exact-origin CORS.
- Modify `livestream-morphing-rs/Cargo.toml`: add the direct test dependency used for router request tests.
- Create `Dockerfile.railway`: build the Rust service correctly from the repository root.
- Create `railway.json`: declare the Railway build, health check, and restart policy.
- Modify `README.md`: document the public deployment environment and measured cost.

### `/Users/worthy/Test/personal`

- Modify `package.json` and `package-lock.json`: add `hls.js` and browser-component test dependencies and scripts.
- Modify `vite.config.ts`: run component tests in jsdom with the shared setup file.
- Modify `src/vite-env.d.ts`: type `VITE_LIVE_STREAM_URL`.
- Create `src/test/setup.ts`: install Testing Library DOM matchers and cleanup.
- Create `src/components/LiveStream/liveStreamState.ts`: own pure status parsing and retry timing.
- Create `src/components/LiveStream/hlsAdapter.ts`: isolate native-HLS and `hls.js` media attachment.
- Create `src/components/LiveStream/LiveStreamPlayer.tsx`: own wake, fallback, crossfade, stall recovery, and cleanup.
- Create `src/components/LiveStream/ArtMedia.tsx`: select the live player only for the livestream artwork.
- Create tests beside the new LiveStream modules.
- Modify `src/pages/Home.tsx`: render `ArtMedia` in the existing artwork detail without changing work-project videos.
- Modify `README.md`: document the live service environment variable and fallback behavior.

---

### Task 1: Model Live Readiness and Session-Safe Segment IDs

**Files:**
- Modify: `livestream-morphing-rs/src/pipeline.rs`

**Interfaces:**
- Produces: `pub enum LiveState { Starting, Live, Unavailable }`, serialized in lowercase.
- Produces: `pub struct LiveStatus { state: LiveState, buffer_count: usize, ready: bool, last_segment_at: u64 }`.
- Produces: `pub async fn live_status(&self, now_ms: u64) -> LiveStatus` on `AppState`.
- Produces: `pub fn mark_pipeline_started(&self, now_ms: u64)` and `pub fn output_segment_id(&self, source_id: &str) -> String` on `AppState`.

- [ ] **Step 1: Write failing readiness and session-ID tests**

Add these focused tests to the existing `pipeline.rs` test module:

```rust
#[tokio::test]
async fn public_status_requires_three_fresh_segments() {
    let (state, _rx) = AppState::new();
    state.mark_pipeline_started(1_000);
    for id in ["1", "2"] {
        state.hls_buffer.write().await.push_segment(id.into(), vec![1]);
    }
    state.stats.lock().unwrap().segment_completed_at = 10_000;
    let status = state.live_status(20_000).await;
    assert_eq!(status.state, LiveState::Starting);
    assert!(!status.ready);

    state.hls_buffer.write().await.push_segment("3".into(), vec![1]);
    let status = state.live_status(20_000).await;
    assert_eq!(status.state, LiveState::Live);
    assert!(status.ready);
}

#[tokio::test]
async fn stale_stream_is_unavailable() {
    let (state, _rx) = AppState::new();
    state.mark_pipeline_started(1_000);
    for id in ["1", "2", "3"] {
        state.hls_buffer.write().await.push_segment(id.into(), vec![1]);
    }
    state.stats.lock().unwrap().segment_completed_at = 10_000;
    assert_eq!(state.live_status(101_001).await.state, LiveState::Unavailable);
}

#[test]
fn output_ids_change_between_sessions() {
    let (state, _rx) = AppState::new();
    state.mark_pipeline_started(1_000);
    let first = state.output_segment_id("44");
    state.mark_pipeline_started(2_000);
    let second = state.output_segment_id("44");
    assert_eq!(first, "1000-44");
    assert_eq!(second, "2000-44");
}
```

- [ ] **Step 2: Run the new tests and confirm they fail**

```bash
cargo test --manifest-path livestream-morphing-rs/Cargo.toml public_status_requires_three_fresh_segments
cargo test --manifest-path livestream-morphing-rs/Cargo.toml output_ids_change_between_sessions
```

Expected: compilation fails because `LiveState`, `live_status`, `mark_pipeline_started`, and `output_segment_id` do not exist.

- [ ] **Step 3: Add the minimal readiness state**

Add atomics to `AppState` for `pipeline_started_at` and `session_id`. Derive `Debug`, `PartialEq`, and `Serialize` on `LiveState`, with `#[serde(rename_all = "lowercase")]`. Implement these rules:

```rust
const READY_SEGMENTS: usize = 3;
const FRESH_SEGMENT_MS: u64 = 18_000;
const UNAVAILABLE_AFTER_MS: u64 = 90_000;

pub async fn live_status(&self, now_ms: u64) -> LiveStatus {
    let buffer_count = self.hls_buffer.read().await.segment_count();
    let last_segment_at = self.stats.lock().unwrap().segment_completed_at;
    let started_at = self.pipeline_started_at.load(Ordering::Relaxed);
    let fresh = last_segment_at > 0 && now_ms.saturating_sub(last_segment_at) <= FRESH_SEGMENT_MS;
    let ready = buffer_count >= READY_SEGMENTS && fresh;
    let state = if ready {
        LiveState::Live
    } else if started_at > 0 && now_ms.saturating_sub(started_at) > UNAVAILABLE_AFTER_MS {
        LiveState::Unavailable
    } else {
        LiveState::Starting
    };
    LiveStatus { state, buffer_count, ready, last_segment_at }
}
```

`mark_pipeline_started` stores `now_ms` in both atomics and resets `ProcessingStats` so data from a previous session cannot mark a new session ready. `output_segment_id` returns `"{session_id}-{source_id}"`.

Call `mark_pipeline_started` immediately after first activation and after every reactivation. Prefix both normal and prefetched output segment IDs before pushing them into `HlsBuffer`.

- [ ] **Step 4: Run all Rust tests**

```bash
cargo test --manifest-path livestream-morphing-rs/Cargo.toml
```

Expected: all tests pass, including existing HLS and effects tests.

- [ ] **Step 5: Commit the state model**

```bash
git add livestream-morphing-rs/src/pipeline.rs
git commit -m "feat: model public live stream readiness"
```

---

### Task 2: Build a Production-Safe Public Router

**Files:**
- Modify: `livestream-morphing-rs/src/server.rs`
- Modify: `livestream-morphing-rs/Cargo.toml`

**Interfaces:**
- Consumes: `AppState::live_status(now_ms)` and `AppState::touch()` from Task 1.
- Produces: `pub struct ServerConfig { pub public_mode: bool, pub allowed_origin: HeaderValue }`.
- Produces: `pub fn router_with_config(state: Arc<AppState>, config: ServerConfig) -> Router`.
- Produces: public `GET /api/live/status` with `LiveStatus` JSON.
- Preserves: `pub fn router(state: Arc<AppState>) -> Router`, implemented with `ServerConfig::from_env()`.

- [ ] **Step 1: Add failing router behavior tests**

Add `tower = { version = "0.5", features = ["util"] }` under `[dev-dependencies]`. Add Axum request tests in `server.rs` using `tower::ServiceExt`:

```rust
#[tokio::test]
async fn public_status_wakes_pipeline_and_is_not_cached() {
    let (state, rx) = AppState::new();
    let app = router_with_config(state, test_config(true));
    let response = app
        .oneshot(Request::builder().uri("/api/live/status").body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(*rx.borrow());
}

#[tokio::test]
async fn public_router_does_not_mount_mutation_routes() {
    let (state, _rx) = AppState::new();
    let app = router_with_config(state, test_config(true));
    let response = app
        .oneshot(Request::builder().method(Method::PUT).uri("/api/source")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"url":"https://example.com"}"#)).unwrap())
        .await.unwrap();
    assert!(matches!(response.status(), StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED));
}

#[tokio::test]
async fn successful_segments_are_public_and_immutable() {
    let (state, _rx) = AppState::new();
    state.hls_buffer.write().await.push_segment("1000-44".into(), vec![1, 2, 3]);
    let app = router_with_config(state, test_config(true));
    let response = app.oneshot(Request::builder().uri("/api/segments/1000-44.ts")
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "public, max-age=3600, immutable");
}
```

Also test that `https://worthyrae.com` receives `Access-Control-Allow-Origin` and `https://attacker.example` does not.

- [ ] **Step 2: Run the server tests and confirm they fail**

```bash
cargo test --manifest-path livestream-morphing-rs/Cargo.toml server::tests
```

Expected: compilation fails because `ServerConfig`, `test_config`, `router_with_config`, and `/api/live/status` do not exist.

- [ ] **Step 3: Split public and local routes**

Build the router in two stages:

```rust
let public = Router::new()
    .route("/api/live/status", get(live_status))
    .route("/api/stream", get(stream_playlist))
    .route("/api/segments/{segment_id}", get(get_segment))
    .route("/health", get(health));

let app = if config.public_mode {
    public
} else {
    public.merge(api::api_router()).fallback_service(studio_service())
};
```

`live_status` calls `state.touch()`, computes Unix milliseconds, calls `state.live_status(now_ms).await`, and returns JSON with `Cache-Control: no-store`. Change the playlist header to `Cache-Control: no-store`. Change successful segment responses to `Cache-Control: public, max-age=3600, immutable`; retain non-cacheable errors.

Create CORS with one parsed `HeaderValue` from `ALLOWED_ORIGIN`. Public mode allows `GET` only. Local mode may allow the existing mutation methods but never uses `AllowOrigin::any()`.

`ServerConfig::from_env()` uses:

```rust
let public_mode = std::env::var("PUBLIC_MODE").as_deref() == Ok("true");
let default_origin = if public_mode { "https://worthyrae.com" } else { "http://localhost:5176" };
let allowed_origin = std::env::var("ALLOWED_ORIGIN").unwrap_or_else(|_| default_origin.into())
    .parse::<HeaderValue>().expect("ALLOWED_ORIGIN must be a valid HTTP header value");
```

- [ ] **Step 4: Run router and full Rust checks**

```bash
cargo test --manifest-path livestream-morphing-rs/Cargo.toml server::tests
cargo test --manifest-path livestream-morphing-rs/Cargo.toml
cargo clippy --manifest-path livestream-morphing-rs/Cargo.toml --all-targets -- -D warnings
```

Expected: all commands exit successfully and public router tests prove mutable routes are absent.

- [ ] **Step 5: Commit the public router**

```bash
git add livestream-morphing-rs/src/server.rs livestream-morphing-rs/Cargo.toml livestream-morphing-rs/Cargo.lock
git commit -m "feat: expose safe public stream routes"
```

---

### Task 3: Package the Rust Service for Railway

**Files:**
- Create: `Dockerfile.railway`
- Create: `railway.json`
- Modify: `README.md`

**Interfaces:**
- Consumes: `PUBLIC_MODE`, `ALLOWED_ORIGIN`, `PORT`, and optional `STREAM_URL` environment variables.
- Produces: a repository-root Docker build that runs `/usr/local/bin/morphing` on Railway.
- Produces: Railway health checking through `GET /health`.

- [ ] **Step 1: Document the runtime contract**

Add these exact production settings to the README:

```text
PUBLIC_MODE=true
ALLOWED_ORIGIN=https://worthyrae.com
PORT is supplied by Railway
STREAM_URL is optional and defaults to the Abbey Road source
Serverless must be enabled for the livestream-morphing service
```

Document that `live.worthyrae.com` is the only public playback hostname and the local studio is mounted only when `PUBLIC_MODE` is false.

- [ ] **Step 2: Add a repository-root Railway Dockerfile**

Create `Dockerfile.railway`:

```dockerfile
FROM rust:1.86-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    libavcodec-dev libavformat-dev libavutil-dev libswscale-dev \
    libavfilter-dev libavdevice-dev pkg-config libclang-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY livestream-morphing-rs/Cargo.toml livestream-morphing-rs/Cargo.lock ./
RUN mkdir src && printf 'fn main() {}\n' > src/main.rs \
    && cargo build --release \
    && rm -f target/release/deps/livestream_morphing_rs*
COPY livestream-morphing-rs/src/ src/
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    libavcodec59 libavformat59 libavutil57 libswscale6 libavfilter8 \
    libavdevice59 libx264-164 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN adduser --system --no-create-home --shell /sbin/nologin morphing
COPY --from=builder /app/target/release/livestream-morphing-rs /usr/local/bin/morphing
USER morphing
EXPOSE 8080
ENV PORT=8080 PUBLIC_MODE=true ALLOWED_ORIGIN=https://worthyrae.com
CMD ["morphing"]
```

- [ ] **Step 3: Add Railway build configuration**

Create `railway.json`:

```json
{
  "$schema": "https://railway.com/railway.schema.json",
  "build": { "builder": "DOCKERFILE", "dockerfilePath": "Dockerfile.railway" },
  "deploy": {
    "healthcheckPath": "/health",
    "healthcheckTimeout": 300,
    "restartPolicyType": "ON_FAILURE",
    "restartPolicyMaxRetries": 3
  }
}
```

- [ ] **Step 4: Build and smoke-test the production image locally**

```bash
docker build -f Dockerfile.railway -t livestream-morphing:railway .
docker run --rm -d --name livestream-morphing-smoke -p 18080:8080 \
  -e PUBLIC_MODE=true -e ALLOWED_ORIGIN=https://worthyrae.com \
  livestream-morphing:railway
curl --fail http://127.0.0.1:18080/health
curl --fail -H 'Origin: https://worthyrae.com' -D - http://127.0.0.1:18080/api/live/status
docker stop livestream-morphing-smoke
```

Expected: health returns `ok`, status returns JSON and the exact allowed origin, and the named container is stopped.

- [ ] **Step 5: Commit Railway packaging**

```bash
git add Dockerfile.railway railway.json README.md
git commit -m "chore: package stream service for Railway"
```

---

### Task 4: Add the Portfolio Live-State Model and Test Harness

**Files:**
- Modify: `/Users/worthy/Test/personal/package.json`
- Modify: `/Users/worthy/Test/personal/package-lock.json`
- Modify: `/Users/worthy/Test/personal/vite.config.ts`
- Create: `/Users/worthy/Test/personal/src/test/setup.ts`
- Create: `/Users/worthy/Test/personal/src/components/LiveStream/liveStreamState.ts`
- Create: `/Users/worthy/Test/personal/src/components/LiveStream/__tests__/liveStreamState.test.ts`

**Interfaces:**
- Produces: `export type LivePhase = 'fallback' | 'waking' | 'buffering' | 'live' | 'recovering' | 'unavailable'`.
- Produces: `export interface LiveStatus { state: 'starting' | 'live' | 'unavailable'; buffer_count: number; ready: boolean; last_segment_at: number }`.
- Produces: `export function parseLiveStatus(value: unknown): LiveStatus | null`.
- Produces: `export function retryDelay(attempt: number): number`, returning 2–30 seconds.

- [ ] **Step 1: Create the portfolio branch and install dependencies**

```bash
git -C /Users/worthy/Test/personal switch -c worthy/09_09_2026/portfolio_live_stream
cd /Users/worthy/Test/personal
npm install hls.js
npm install --save-dev @testing-library/react @testing-library/jest-dom jsdom
npm pkg set scripts.test="vitest run"
```

Expected: manifests record the dependencies and the personal repository is on the required branch.

- [ ] **Step 2: Configure jsdom and shared cleanup**

Change the config imports so Vitest's `test` property is typed, while preserving
the existing Vite `Plugin` type:

```ts
import { defineConfig } from 'vitest/config';
import type { Plugin } from 'vite';
```

Then extend `vite.config.ts`:

```ts
test: {
  environment: 'jsdom',
  setupFiles: ['./src/test/setup.ts'],
},
```

Create `src/test/setup.ts`:

```ts
import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

afterEach(() => cleanup());
```

- [ ] **Step 3: Write failing state tests**

```ts
import { describe, expect, it } from 'vitest';
import { parseLiveStatus, retryDelay } from '../liveStreamState';

describe('parseLiveStatus', () => {
  it('accepts the public backend shape', () => {
    expect(parseLiveStatus({ state: 'live', buffer_count: 3, ready: true, last_segment_at: 42 }))
      .toEqual({ state: 'live', buffer_count: 3, ready: true, last_segment_at: 42 });
  });
  it('rejects malformed status payloads', () => {
    expect(parseLiveStatus({ state: 'live', ready: 'yes' })).toBeNull();
    expect(parseLiveStatus(null)).toBeNull();
  });
});

describe('retryDelay', () => {
  it('starts at two seconds and caps at thirty seconds', () => {
    expect(retryDelay(0)).toBe(2_000);
    expect(retryDelay(2)).toBe(8_000);
    expect(retryDelay(20)).toBe(30_000);
  });
});
```

- [ ] **Step 4: Run the state test and confirm it fails**

```bash
cd /Users/worthy/Test/personal
npm test -- src/components/LiveStream/__tests__/liveStreamState.test.ts
```

Expected: FAIL because `liveStreamState.ts` does not exist.

- [ ] **Step 5: Implement strict parsing and retry timing**

Implement `parseLiveStatus` with explicit object, enum, boolean, and finite-number checks; do not cast unvalidated JSON. Implement:

```ts
export function retryDelay(attempt: number): number {
  return Math.min(2_000 * 2 ** Math.max(0, attempt), 30_000);
}
```

- [ ] **Step 6: Run the state tests and commit**

```bash
cd /Users/worthy/Test/personal
npm test -- src/components/LiveStream/__tests__/liveStreamState.test.ts
git add package.json package-lock.json vite.config.ts src/test/setup.ts src/components/LiveStream/liveStreamState.ts src/components/LiveStream/__tests__/liveStreamState.test.ts
git commit -m "test: add live stream state model"
```

Expected: the focused tests pass and the commit contains only test setup and the pure state model.

---

### Task 5: Implement HLS Attachment and the Resilient Live Player

**Files:**
- Create: `/Users/worthy/Test/personal/src/components/LiveStream/hlsAdapter.ts`
- Create: `/Users/worthy/Test/personal/src/components/LiveStream/LiveStreamPlayer.tsx`
- Create: `/Users/worthy/Test/personal/src/components/LiveStream/__tests__/hlsAdapter.test.ts`
- Create: `/Users/worthy/Test/personal/src/components/LiveStream/__tests__/LiveStreamPlayer.test.tsx`

**Interfaces:**
- Consumes: `LiveStatus`, `parseLiveStatus`, `retryDelay`, and `LivePhase` from Task 4.
- Produces: `export interface StreamHandle { destroy(): void }`.
- Produces: `export function attachHls(video: HTMLVideoElement, playlistUrl: string, onFatal: () => void): StreamHandle`.
- Produces: `export interface LiveStreamPlayerProps { fallbackUrl: string; baseUrl?: string; attachStream?: typeof attachHls }`.

- [ ] **Step 1: Write failing HLS adapter tests**

Test native HLS by stubbing `video.canPlayType` to return `'probably'`. Mock `hls.js` for the other branch and assert `loadSource`, `attachMedia`, fatal-error callback registration, and `destroy` each run once. Include this native assertion:

```ts
Object.defineProperty(video, 'canPlayType', { value: () => 'probably' });
const handle = attachHls(video, 'https://live.worthyrae.com/api/stream', onFatal);
expect(video.src).toBe('https://live.worthyrae.com/api/stream');
handle.destroy();
expect(video.getAttribute('src')).toBeNull();
```

- [ ] **Step 2: Run the adapter test and confirm it fails**

```bash
cd /Users/worthy/Test/personal
npm test -- src/components/LiveStream/__tests__/hlsAdapter.test.ts
```

Expected: FAIL because `attachHls` does not exist.

- [ ] **Step 3: Implement the HLS adapter**

Native HLS uses `video.canPlayType('application/vnd.apple.mpegurl')`. Otherwise construct `new Hls({ lowLatencyMode: false, liveSyncDurationCount: 3 })`, subscribe to `Hls.Events.ERROR`, and call `onFatal` only when `data.fatal` is true. `destroy()` removes the native `src` and calls `video.load()`, or destroys the `hls.js` instance.

- [ ] **Step 4: Write failing player lifecycle tests**

Use fake timers, mocked `fetch`, and injected `attachStream`. Cover these behaviors:

```ts
it('renders the fallback before requesting status');
it('polls status until ready and attaches the live playlist');
it('does not reveal live video before canplay');
it('shows LIVE only after canplay');
it('keeps fallback and shows retrying after ninety seconds');
it('returns to fallback after a fatal HLS error');
it('returns to fallback when live playback stops progressing');
it('clears timers and destroys the stream on unmount');
it('uses fallback only when baseUrl is absent');
```

For the central transition:

```ts
render(<LiveStreamPlayer
  fallbackUrl="https://portfolio-worthy.s3.us-east-1.amazonaws.com/abbey_road_best.mp4"
  baseUrl="https://live.worthyrae.com"
  attachStream={attachStream}
/>);
await vi.advanceTimersByTimeAsync(2_000);
expect(attachStream).toHaveBeenCalledWith(expect.any(HTMLVideoElement),
  'https://live.worthyrae.com/api/stream', expect.any(Function));
expect(screen.queryByText('LIVE')).not.toBeInTheDocument();
fireEvent.canPlay(screen.getByTestId('live-video'));
expect(screen.getByText('LIVE')).toBeInTheDocument();
```

- [ ] **Step 5: Run player tests and confirm they fail**

```bash
cd /Users/worthy/Test/personal
npm test -- src/components/LiveStream/__tests__/LiveStreamPlayer.test.tsx
```

Expected: FAIL because `LiveStreamPlayer` does not exist.

- [ ] **Step 6: Implement the player state machine**

Implement these rules:

- Mount fallback and call `${baseUrl}/api/live/status` with `cache: 'no-store'`.
- Poll every two seconds while the response is valid but not ready.
- When ready, call `attachStream(liveVideo, `${baseUrl}/api/stream`, recover)` once.
- Keep live opacity zero until `canplay`; then call `video.play()`, set phase `live`, and transition opacity over 500 ms.
- Track `currentTime` every five seconds while live. Two unchanged checks trigger recovery.
- On fetch failure, malformed JSON, backend `unavailable`, startup timeout, or fatal HLS error, keep or restore fallback and schedule `retryDelay(attempt)`.
- After 90 seconds without first reaching live, set phase `unavailable`.
- Abort fetches, clear all timers, remove listeners, and call `StreamHandle.destroy()` on recovery and unmount.
- When `baseUrl` is absent, render fallback without requests or a failure label.

Render two absolutely stacked `<video>` elements in one 16:9 container. Both are muted and `playsInline`; only fallback loops. Use labels `STARTING LIVE FEED`, `LIVE`, and `LIVE FEED UNAVAILABLE · RETRYING` verbatim.

- [ ] **Step 7: Run focused and full portfolio tests**

```bash
cd /Users/worthy/Test/personal
npm test -- src/components/LiveStream/__tests__/hlsAdapter.test.ts src/components/LiveStream/__tests__/LiveStreamPlayer.test.tsx
npm test
```

Expected: all tests pass with no leaked timers or post-unmount state updates.

- [ ] **Step 8: Commit the player**

```bash
cd /Users/worthy/Test/personal
git add src/components/LiveStream/hlsAdapter.ts src/components/LiveStream/LiveStreamPlayer.tsx src/components/LiveStream/__tests__/hlsAdapter.test.ts src/components/LiveStream/__tests__/LiveStreamPlayer.test.tsx
git commit -m "feat: add resilient live stream player"
```

---

### Task 6: Integrate Live Playback into the Artwork Detail

**Files:**
- Create: `/Users/worthy/Test/personal/src/components/LiveStream/ArtMedia.tsx`
- Create: `/Users/worthy/Test/personal/src/components/LiveStream/__tests__/ArtMedia.test.tsx`
- Modify: `/Users/worthy/Test/personal/src/pages/Home.tsx`
- Modify: `/Users/worthy/Test/personal/src/vite-env.d.ts`
- Modify: `/Users/worthy/Test/personal/README.md`

**Interfaces:**
- Consumes: `LiveStreamPlayer` from Task 5 and `ArtPiece` from `src/lib/art.ts`.
- Produces: `export function ArtMedia({ piece }: { piece: ArtPiece }): JSX.Element | null`.
- Consumes: `import.meta.env.VITE_LIVE_STREAM_URL` only for `piece.slug === 'livestream-art'`.

- [ ] **Step 1: Write failing artwork selection tests**

Mock `LiveStreamPlayer` and test:

```ts
it('uses the live player for livestream-art', () => {
  render(<ArtMedia piece={getArtPiece('livestream-art')!} />);
  expect(screen.getByTestId('mock-live-player')).toHaveAttribute(
    'data-fallback',
    'https://portfolio-worthy.s3.us-east-1.amazonaws.com/abbey_road_best.mp4'
  );
});

it('keeps an ordinary looping video for other video artwork', () => {
  const { container } = render(<ArtMedia piece={getArtPiece('ai-architecture')!} />);
  expect(container.querySelector('video')).toHaveAttribute(
    'src',
    'https://portfolio-worthy.s3.us-east-1.amazonaws.com/flesh_digression.mp4'
  );
  expect(screen.queryByTestId('mock-live-player')).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run the artwork test and confirm it fails**

```bash
cd /Users/worthy/Test/personal
npm test -- src/components/LiveStream/__tests__/ArtMedia.test.tsx
```

Expected: FAIL because `ArtMedia` does not exist.

- [ ] **Step 3: Implement and mount `ArtMedia`**

For the live slug, render:

```tsx
<LiveStreamPlayer
  fallbackUrl={piece.videoUrl!}
  baseUrl={import.meta.env.VITE_LIVE_STREAM_URL}
/>
```

For other artwork, preserve the existing YouTube iframe, looping video, image, and empty behavior from `MediaBlock`. In `Home.tsx`, replace only the art-detail call with `<ArtMedia piece={displayArtPiece} />`. Do not change the work-detail `MediaBlock` call.

Type the variable in `vite-env.d.ts`:

```ts
interface ImportMetaEnv {
  readonly VITE_LIVE_STREAM_URL?: string;
}
interface ImportMeta {
  readonly env: ImportMetaEnv;
}
```

Document `VITE_LIVE_STREAM_URL=https://live.worthyrae.com` and state that an unset value intentionally leaves the recording in place.

- [ ] **Step 4: Run integration tests, build, and lint**

```bash
cd /Users/worthy/Test/personal
npm test
npm run build
npm run lint
```

Expected: tests and build pass. Lint introduces no new errors; if the repository has pre-existing failures, record the unchanged baseline and verify changed files with `npx eslint src/components/LiveStream src/pages/Home.tsx`.

- [ ] **Step 5: Commit portfolio integration**

```bash
cd /Users/worthy/Test/personal
git add src/components/LiveStream/ArtMedia.tsx src/components/LiveStream/__tests__/ArtMedia.test.tsx src/pages/Home.tsx src/vite-env.d.ts README.md
git commit -m "feat: open livestream art with live playback"
```

---

### Task 7: Deploy, Configure Cloudflare, and Enforce the Cost Gate

**Files:**
- Modify if one tuning pass is required: `livestream-morphing-rs/src/codec.rs`
- Modify: `README.md`
- No infrastructure secrets are written to either repository.

**Interfaces:**
- Consumes: production container and public routes from Tasks 1–3.
- Consumes: `VITE_LIVE_STREAM_URL=https://live.worthyrae.com` from Task 6.
- Produces: Railway service `livestream-morphing` and Cloudflare hostname `live.worthyrae.com`.
- Produces: recorded cold-start, CPU, memory, egress, segment size, and projected monthly cost in `README.md`.

- [ ] **Step 1: Verify Railway identity and select the existing project**

```bash
railway whoami
railway list
railway status
```

If the repository is not linked to the intended existing Railway project, run `railway link` and select it interactively. Do not create a second paid workspace.

- [ ] **Step 2: Create or select the service and set public variables**

Inspect services with `railway status`. If `livestream-morphing` does not exist:

```bash
railway add --service livestream-morphing
```

Set non-secret variables without listing the environment:

```bash
railway variable set PUBLIC_MODE=true ALLOWED_ORIGIN=https://worthyrae.com --service livestream-morphing
```

Do not run `railway variable list`, whose machine-readable output can include raw secrets.

- [ ] **Step 3: Deploy and verify Railway before the custom domain**

```bash
railway up --service livestream-morphing --environment production --detach
railway deployment list --service livestream-morphing
railway logs --service livestream-morphing --environment production --lines 100
railway domain --service livestream-morphing
```

Use the generated Railway domain to verify `/health`, `/api/live/status`, `/api/stream`, and rejection of `PUT /api/source`. Keep the portfolio production variable unset until these checks pass.

- [ ] **Step 4: Enable Serverless and add the custom domain**

In Railway service settings, enable Serverless for `livestream-morphing`. Then run:

```bash
railway domain live.worthyrae.com --port 8080 --service livestream-morphing
railway domain status live.worthyrae.com --service livestream-morphing
```

Add the returned CNAME target to the existing `worthyrae.com` Cloudflare zone with proxying enabled. Use Cloudflare SSL/TLS Full mode so Railway terminates a valid origin certificate.

- [ ] **Step 5: Configure and verify the Cloudflare cache rule**

Create one cache rule for hostname `live.worthyrae.com` and URI path `/api/segments/*`: eligible for cache, edge TTL one hour. Do not cache `/api/stream`, `/api/live/status`, or `/health`.

After three segments exist, extract one exact filename from the current playlist
and request that URL twice:

```bash
stream_segment_path=$(curl -sS https://live.worthyrae.com/api/stream | awk '/\.ts$/{line=$0} END{print line}')
test -n "$stream_segment_path"
curl -sSI "https://live.worthyrae.com${stream_segment_path}"
curl -sSI "https://live.worthyrae.com${stream_segment_path}"
```

Expected: `Cache-Control: public, max-age=3600, immutable`, followed by a
Cloudflare cache hit when both requests reach the same edge.

- [ ] **Step 6: Configure and redeploy the portfolio**

Set this build-time variable on the existing portfolio Railway service:

```text
VITE_LIVE_STREAM_URL=https://live.worthyrae.com
```

Redeploy because Vite embeds the value during build. Open `https://worthyrae.com/art/livestream-art` and verify fallback, starting label, crossfade, and live playback in desktop Safari and one Chromium browser.

- [ ] **Step 7: Run the one- and five-viewer acceptance session**

With the service initially asleep:

1. Record the timestamp before opening the page.
2. Record first successful status and first `canplay` timestamps.
3. Keep one viewer open for 15 minutes and check for stalls.
4. Open four more clients and keep all five playing for 15 minutes.
5. Close every client and confirm logs show pipeline idle after about five minutes.
6. Confirm Railway marks the service asleep after its outbound inactivity window.

Collect metrics without exposing variables:

```bash
railway metrics --service livestream-morphing --since 1h --cpu --memory --network
railway usage projects --period current
```

- [ ] **Step 8: Calculate and apply the cost gate**

Record average CPU, memory, segment bytes per hour, and startup time in the README. Calculate:

```text
CPU/hour    = average_vCPU × ($20 / 730)
RAM/hour    = average_GB × ($10 / 730)
Egress/hour = origin_GB_per_hour × $0.05
Total/hour  = CPU/hour + RAM/hour + Egress/hour
```

Project 50 active hours plus ten minutes of wake/idle overhead per session. Accept Railway when the projection is at most $5 and processing stays ahead of the six-second cadence.

If either condition fails, make exactly one tuning pass by lowering output frame rate or changing the existing 2x downsample to a documented smaller output dimension. Repeat Rust checks and the acceptance session. If the second measurement fails, stop and write a separate AWS Fargate design; do not improvise an AWS migration in this plan.

- [ ] **Step 9: Configure a non-destructive billing alert**

Run `railway usage limit status` and confirm the workspace is shared with the portfolio. Configure a $4 soft usage alert in the Railway dashboard. Do not set a workspace hard limit because it could stop the portfolio service.

- [ ] **Step 10: Run final verification and commit measurements**

Stream repository:

```bash
cd /Users/worthy/Test/livestream-morphing
cargo test --manifest-path livestream-morphing-rs/Cargo.toml
cargo clippy --manifest-path livestream-morphing-rs/Cargo.toml --all-targets -- -D warnings
git diff --check
git add README.md
git diff --cached --quiet || git commit -m "docs: record live stream deployment benchmark"
```

If `codec.rs` was tuned, add it to the same benchmark commit after its tests pass.

Portfolio repository:

```bash
cd /Users/worthy/Test/personal
npm test
npm run build
npx eslint src/components/LiveStream src/pages/Home.tsx
git diff --check
```

Expected: checks pass, the public page recovers to fallback when interrupted, and README records the measured cost decision.

- [ ] **Step 11: Verify and document rollback**

Build the portfolio once without the live URL and confirm that the livestream
art entry renders only its recording:

```bash
cd /Users/worthy/Test/personal
env -u VITE_LIVE_STREAM_URL npm run build
npm test -- src/components/LiveStream/__tests__/ArtMedia.test.tsx src/components/LiveStream/__tests__/LiveStreamPlayer.test.tsx
```

Document the production rollback in both READMEs: remove
`VITE_LIVE_STREAM_URL` from the portfolio service and redeploy it, then disable
the `livestream-morphing` Railway service after the fallback-only deployment is
healthy. This order keeps `/art/livestream-art` usable throughout rollback.

---

## Completion Criteria

- `/art/livestream-art` immediately shows the recording and wakes the live service.
- The page switches only after three fresh segments and `canplay`, then displays `LIVE`.
- Startup timeout, malformed status, fatal HLS error, and playback stall preserve or restore the recording and retry.
- Leaving releases timers and media resources; the processor idles and Railway sleeps.
- Cloudflare caches successful segments but never playlists or status.
- Production rejects anonymous mutation of source, pipeline, effects, and presets.
- Rust tests, Rust clippy, portfolio tests, portfolio build, changed-file lint, and deployment smoke checks pass.
- The measured 50-active-hour projection is at most $5, or implementation stops for a separate AWS fallback design.
