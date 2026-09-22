# Hetzner and Twitch Livestream Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run the unchanged Abbey Road painter continuously on an hourly Hetzner x86 server, relay one Twitch-compatible stream, and switch the portfolio player through a runtime-controlled, reversible Twitch embed.

**Architecture:** The existing painter remains the only renderer and serves private HLS inside Docker Compose. A supervised FFmpeg container consumes that HLS continuously, adds silent AAC, and stream-copies H.264 to Twitch; the portfolio backend selects Twitch or the existing Railway HLS player at runtime so rollback needs no frontend rebuild.

**Tech Stack:** Rust 1.86, ffmpeg-next/libx264, Python 3.11, Docker Compose, FFmpeg 5.1 on Debian Bookworm, React 18, TypeScript, Vitest, FastAPI, Hetzner Cloud, Twitch embedded player.

**Spec:** `docs/superpowers/specs/2026-09-22-hetzner-twitch-migration-design.md`

## Global Constraints

- Keep the Loaded Bristle painter, brush populations, grading, wave, 1920 x 1080 dimensions, and adaptive paint cadence unchanged.
- Do not reintroduce motion warping, synthetic frames, or a one-second update path.
- Use an x86 CX43 candidate; the ARM path did not sustain the approved workload.
- Keep the Twitch stream key out of Git, Docker image layers, command output, and application logs.
- Do not expose the painter publicly from Hetzner; bind its diagnostic port to `127.0.0.1` only.
- Do not change the website or Railway until the private Hetzner benchmark passes.
- Do not switch the public website until the user approves a Twitch-player sample.
- Preserve Railway for a 24-hour rollback window after cutover.
- Stop only the Railway livestream service after the rollback window; preserve the personal website service and project history.
- Keep recurring infrastructure below $30 per month before tax.
- Branches created during execution use `worthy/DD_MM_YYYY/lowercase_snake_case`, with the actual local creation date.

---

## File map

### Livestream repository

- Modify `livestream-morphing-rs/src/codec.rs`: optional two-second GOP configuration for Twitch-compatible stream copy.
- Create `deploy/hetzner/compose.yaml`: private painter, relay, and qualification-sink services.
- Create `deploy/hetzner/relay/Dockerfile`: reproducible FFmpeg runtime.
- Create `deploy/hetzner/relay/relay.sh`: secret-safe reconnecting Twitch relay.
- Create `deploy/hetzner/relay/healthcheck.sh`: reject stalled FFmpeg progress.
- Create `deploy/hetzner/painter.env.example`: non-secret exact production variable names.
- Create `deploy/hetzner/cloud-init.yaml`: Docker installation and locked-down host bootstrap.
- Create `deploy/hetzner/test-requirements.txt`: pinned local-only verification dependency.
- Create `deploy/hetzner/verify_bundle.py`: static safety checks for the deployment bundle.
- Create `deploy/hetzner/test_verify_bundle.py`: bundle verification tests.
- Create `deploy/hetzner/qualify.py`: one-hour status sampler and acceptance report.
- Create `deploy/hetzner/test_qualify.py`: deterministic qualification-policy tests.
- Modify `README.md`: operator commands, secret placement, qualification, recovery, and shutdown procedure.

### Personal website repository (`/Users/worthy/Test/personal`)

- Create `backend/livestream.py`: validate and expose the public runtime player selection.
- Modify `backend/main.py`: add `GET /api/livestream/config` with `no-store` caching.
- Create `backend/test_livestream.py`: backend configuration tests.
- Create `src/components/LiveStream/liveProvider.ts`: parse/fetch runtime configuration.
- Create `src/components/LiveStream/TwitchLiveStreamPlayer.tsx`: safe Twitch iframe and fallback layer.
- Rename `src/components/LiveStream/LiveStreamPlayer.tsx` to `src/components/LiveStream/HlsLiveStreamPlayer.tsx`: preserve the current HLS implementation unchanged behind a new wrapper.
- Create `src/components/LiveStream/LiveStreamPlayer.tsx`: runtime provider wrapper with HLS fallback.
- Modify `src/components/LiveStream/__tests__/LiveStreamPlayer.test.tsx`: target the renamed HLS component.
- Create `src/components/LiveStream/__tests__/TwitchLiveStreamPlayer.test.tsx`: Twitch URL and layout tests.
- Create `src/components/LiveStream/__tests__/liveProvider.test.ts`: public config parser tests.
- Create `src/components/LiveStream/__tests__/LiveStreamProvider.test.tsx`: provider selection and rollback tests.

---

### Task 1: Add opt-in Twitch keyframe spacing without changing Railway defaults

**Files:**
- Modify: `livestream-morphing-rs/src/codec.rs:168-240`
- Test: `livestream-morphing-rs/src/codec.rs` test module

**Interfaces:**
- Consumes: optional environment variable `ENCODE_GOP_SECONDS`.
- Produces: `EncoderOptions::from_value(value: Option<&str>, fps: f64) -> Result<EncoderOptions, String>` and `TsEncoder::new_with_options(width: u32, height: u32, fps: f64, pts_offset: i64, options: EncoderOptions) -> Result<Self, Box<dyn std::error::Error + Send + Sync>>`.
- Default: missing `ENCODE_GOP_SECONDS` retains the current libx264 behavior byte-for-byte at the option layer.

- [ ] **Step 1: Write parser tests for absent, valid, and invalid GOP values**

```rust
#[test]
fn twitch_gop_is_opt_in_and_uses_source_fps() {
    assert_eq!(EncoderOptions::from_value(None, 30000.0 / 1001.0).unwrap().gop_frames, None);
    assert_eq!(EncoderOptions::from_value(Some("2"), 30000.0 / 1001.0).unwrap().gop_frames, Some(60));
    assert!(EncoderOptions::from_value(Some("0"), 30.0).is_err());
    assert!(EncoderOptions::from_value(Some("not-a-number"), 30.0).is_err());
}
```

- [ ] **Step 2: Run the focused test and confirm it fails**

Run: `cargo test --manifest-path livestream-morphing-rs/Cargo.toml twitch_gop_is_opt_in_and_uses_source_fps`

Expected: FAIL because `EncoderOptions` does not exist.

- [ ] **Step 3: Implement the pure options parser and option-aware constructor**

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
struct EncoderOptions { gop_frames: Option<u32> }

impl EncoderOptions {
    fn from_value(value: Option<&str>, fps: f64) -> Result<Self, String> {
        let Some(value) = value else { return Ok(Self { gop_frames: None }); };
        let seconds = value.parse::<f64>().map_err(|_| "ENCODE_GOP_SECONDS must be numeric".to_string())?;
        if !seconds.is_finite() || seconds <= 0.0 || seconds > 10.0 {
            return Err("ENCODE_GOP_SECONDS must be in (0, 10]".into());
        }
        Ok(Self { gop_frames: Some((seconds * fps).round().max(1.0) as u32) })
    }
}
```

Have `TsEncoder::new` call `EncoderOptions::from_value(std::env::var("ENCODE_GOP_SECONDS").ok().as_deref(), fps)` and delegate to `new_with_options`. When `gop_frames` is present, call `encoder.set_gop(frames)` and set libx264 options `keyint` and `min-keyint` to that decimal frame count, plus `scenecut=0`. Do not add any option when it is absent.

- [ ] **Step 4: Add an encoded-packet test proving a keyframe at least every 60 frames**

```rust
#[test]
fn configured_two_second_gop_marks_each_interval_as_key() {
    init();
    let frames: Vec<_> = (0..180).map(|i| RawFrame::filled(64, 48, i as u8, 80, 160)).collect();
    let bytes = encode_timed_with_options(&frames, 30000.0 / 1001.0, 0, EncoderOptions { gop_frames: Some(60) }).unwrap();
    let keys = video_keyframe_indices(&bytes);
    assert!(keys.contains(&0));
    assert!(keys.windows(2).all(|pair| pair[1] - pair[0] <= 60), "keyframes: {keys:?}");
}
```

Implement the test-only `video_keyframe_indices` by writing the bytes to `NamedTempFile`, demuxing the video packets, sorting packet PTS, and returning presentation-order indices whose packet has `ffmpeg::packet::Flags::KEY`.

- [ ] **Step 5: Run codec and full Rust tests**

Run: `cargo test --manifest-path livestream-morphing-rs/Cargo.toml codec::tests`

Expected: PASS.

Run: `cargo test --manifest-path livestream-morphing-rs/Cargo.toml`

Expected: PASS with the existing test count plus the new GOP tests.

- [ ] **Step 6: Verify the absent-variable encoded behavior remains valid**

Run: `env -u ENCODE_GOP_SECONDS cargo test --manifest-path livestream-morphing-rs/Cargo.toml independently_muxed_segments_keep_continuous_timestamps`

Expected: PASS; timestamps remain continuous and Railway has no new encoder option.

- [ ] **Step 7: Commit the encoder change**

```bash
git add livestream-morphing-rs/src/codec.rs
git commit -m "feat: support Twitch keyframe spacing"
```

---

### Task 2: Build a secret-safe private painter and Twitch relay bundle

**Files:**
- Create: `deploy/hetzner/compose.yaml`
- Create: `deploy/hetzner/relay/Dockerfile`
- Create: `deploy/hetzner/relay/relay.sh`
- Create: `deploy/hetzner/relay/healthcheck.sh`
- Create: `deploy/hetzner/painter.env.example`
- Create: `deploy/hetzner/cloud-init.yaml`
- Create: `deploy/hetzner/test-requirements.txt`
- Create: `deploy/hetzner/verify_bundle.py`
- Create: `deploy/hetzner/test_verify_bundle.py`

**Interfaces:**
- Painter private URL: `http://painter:8080/api/stream`.
- Host-only diagnostic URL: `http://127.0.0.1:18080/api/live/status`.
- Secret file: `/etc/abbey-road-stream/twitch_stream_key`, mounted at `/run/secrets/twitch_stream_key`.
- Relay variables: `TWITCH_INGEST_URL` and `HLS_URL`; neither contains the stream key.
- Qualification service: Compose profile `qualify`, consuming HLS to FFmpeg's null muxer.

- [ ] **Step 1: Write bundle tests before the files exist**

```python
class BundleTests(unittest.TestCase):
    def test_compose_never_publishes_painter_beyond_loopback(self):
        compose = load_yaml(ROOT / "compose.yaml")
        self.assertEqual(compose["services"]["painter"]["ports"], ["127.0.0.1:18080:8080"])

    def test_twitch_key_is_a_file_secret_not_an_environment_value(self):
        compose_text = (ROOT / "compose.yaml").read_text()
        self.assertIn("/run/secrets/twitch_stream_key", compose_text)
        self.assertNotIn("TWITCH_STREAM_KEY=", compose_text)

    def test_relay_requires_healthy_painter_and_restarts(self):
        relay = load_yaml(ROOT / "compose.yaml")["services"]["relay"]
        self.assertEqual(relay["depends_on"]["painter"]["condition"], "service_healthy")
        self.assertEqual(relay["restart"], "unless-stopped")
```

Use `yaml.safe_load`; add `PyYAML==6.0.2` only to a local test requirements file `deploy/hetzner/test-requirements.txt`, not the production image.

- [ ] **Step 2: Run the bundle tests and confirm file-not-found failures**

Run: `python3 -m unittest -v deploy/hetzner/test_verify_bundle.py`

Expected: FAIL because `compose.yaml` does not exist.

- [ ] **Step 3: Create the Compose topology**

The essential service contract is:

```yaml
services:
  painter:
    build:
      context: ../..
      dockerfile: Dockerfile.railway
    env_file: /etc/abbey-road-stream/painter.env
    ports: ["127.0.0.1:18080:8080"]
    expose: ["8080"]
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "/opt/paint/bin/python", "-c", "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8080/health', timeout=3).read()"]
      interval: 10s
      timeout: 5s
      retries: 12
  relay:
    build: ./relay
    environment:
      HLS_URL: http://painter:8080/api/stream
      TWITCH_INGEST_URL: rtmps://live.twitch.tv/app
    secrets: [twitch_stream_key]
    depends_on:
      painter: { condition: service_healthy }
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "/usr/local/bin/healthcheck.sh"]
      interval: 15s
      timeout: 5s
      retries: 4
  qualify:
    build: ./relay
    profiles: ["qualify"]
    environment: { HLS_URL: "http://painter:8080/api/stream" }
    depends_on:
      painter: { condition: service_healthy }
    entrypoint: ["ffmpeg", "-hide_banner", "-loglevel", "warning", "-i", "http://painter:8080/api/stream", "-t", "3600", "-f", "null", "-"]
secrets:
  twitch_stream_key:
    file: /etc/abbey-road-stream/twitch_stream_key
```

- [ ] **Step 4: Create the pinned relay runtime and reconnecting entrypoint**

`deploy/hetzner/relay/Dockerfile`:

```dockerfile
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ffmpeg ca-certificates util-linux \
    && rm -rf /var/lib/apt/lists/*
COPY relay.sh healthcheck.sh /usr/local/bin/
RUN chmod 0555 /usr/local/bin/relay.sh /usr/local/bin/healthcheck.sh
ENTRYPOINT ["/usr/local/bin/relay.sh"]
```

`relay.sh` starts as root only long enough to read and trim the root-owned `/run/secrets/twitch_stream_key`, rejects an empty key, removes any stale progress file, and uses `exec setpriv --reuid=65534 --regid=65534 --clear-groups ffmpeg` so FFmpeg runs unprivileged and signals reach it. Its FFmpeg arguments must include reconnect flags for the HLS input, `-progress /tmp/relay-progress`, `-map 0:v:0`, a lavfi silent stereo source, `-c:v copy`, `-c:a aac`, `-b:a 64k`, `-ar 48000`, and `-f flv "${TWITCH_INGEST_URL}/${stream_key}"`. Disable shell tracing before reading the secret.

`healthcheck.sh` succeeds only when `/tmp/relay-progress` exists and its modification time is less than 45 seconds old.

- [ ] **Step 5: Add exact non-secret painter configuration names**

`painter.env.example` contains:

```dotenv
PORT=8080
PUBLIC_MODE=true
ALLOWED_ORIGIN=https://worthyrae.com
OUTPUT_WIDTH=1920
LIVE_STYLE=loaded_bristle
LOADED_PAINT_STRIDE=2
LOADED_CV_THREADS=4
RENDER_WORKERS=6
ENCODE_THREADS=2
ENCODE_CRF=30
ENCODE_GOP_SECONDS=2
NUMPY_MADVISE_HUGEPAGE=0
```

Copy the remaining current `LIVE_*` values from Railway by name during deployment. Never copy Railway-generated `RAILWAY_*` variables.

- [ ] **Step 6: Add cloud-init hardening**

`cloud-init.yaml` installs Docker Engine and the Compose plugin, creates `/opt/abbey-road-stream` and `/etc/abbey-road-stream`, enables UFW with inbound deny/outbound allow, enables SSH, creates a 1 GB swap file for build spikes, configures bounded journald storage, and enables unattended security upgrades without automatic reboot. Do not embed an SSH key, API token, Twitch key, Git credential, or application environment value.

- [ ] **Step 7: Implement and run static bundle verification**

`verify_bundle.py` must additionally reject `0.0.0.0`, `${TWITCH_STREAM_KEY}`, a world-readable secret mount, missing restart policies, and a relay health check without `relay-progress`.

Run: `python3 -m pip install -r deploy/hetzner/test-requirements.txt`

Run: `python3 -m unittest -v deploy/hetzner/test_verify_bundle.py`

Expected: PASS.

Run: `bash -n deploy/hetzner/relay/relay.sh deploy/hetzner/relay/healthcheck.sh`

Expected: no output and exit 0.

Run: `docker compose -f deploy/hetzner/compose.yaml --profile qualify config`

Expected: valid rendered configuration; the Twitch key value is absent.

- [ ] **Step 8: Build both images locally**

Run: `docker compose -f deploy/hetzner/compose.yaml build painter relay`

Expected: both images build successfully on the development host.

- [ ] **Step 9: Commit the deployment bundle**

```bash
git add deploy/hetzner
git commit -m "feat: add private Hetzner Twitch relay stack"
```

---

### Task 3: Automate the one-hour qualification decision

**Files:**
- Create: `deploy/hetzner/qualify.py`
- Create: `deploy/hetzner/test_qualify.py`
- Modify: `README.md`

**Interfaces:**
- Samples: newline-delimited JSON from `GET /api/live/status` plus host timestamps.
- Function: `evaluate(samples: list[dict], minimum_paint_fps: float) -> dict`.
- Exit 0: all acceptance gates pass. Exit 1: a named gate fails.
- Default benchmark duration: 3600 seconds; sample interval: 6 seconds.

- [ ] **Step 1: Write deterministic policy tests**

```python
def sample(frame, buffer_count=10, total_ms=5000, paint_fps=14.985):
    return {"state": "live", "buffer_count": buffer_count, "engine": {
        "rendered_frames": frame, "total_ms": total_ms,
        "duration_seconds": 6.006, "paint_fps": paint_fps,
        "width": 1920, "height": 1080,
    }}

class QualificationTests(unittest.TestCase):
    def test_accepts_stable_full_hd_run(self):
        report = evaluate([sample(i * 180) for i in range(1, 11)], 14.9)
        self.assertTrue(report["passed"])

    def test_rejects_segment_slower_than_playback(self):
        report = evaluate([sample(180), sample(360, total_ms=6100)], 14.9)
        self.assertIn("segment_deadline", report["failed_gates"])

    def test_rejects_restart_or_falling_buffer(self):
        restarted = [sample(180), sample(20)]
        falling = [sample(i * 180, buffer_count=10-i) for i in range(1, 7)]
        self.assertIn("render_restart", evaluate(restarted, 14.9)["failed_gates"])
        self.assertIn("buffer_trend", evaluate(falling, 14.9)["failed_gates"])
```

- [ ] **Step 2: Run tests and confirm import failure**

Run: `python3 -m unittest -v deploy/hetzner/test_qualify.py`

Expected: FAIL because `qualify.py` does not exist.

- [ ] **Step 3: Implement sampling and evaluation**

Require all samples to be 1920 x 1080, `state == "live"` after warmup, monotonically increasing `rendered_frames`, `total_ms <= duration_seconds * 1000`, `paint_fps >= minimum_paint_fps`, and no sustained negative buffer slope across six consecutive samples. Ignore repeated HTTP polls for the same completed segment when calculating segment gates.

The CLI accepts `--url`, `--seconds`, `--interval`, `--baseline`, and `--output`. The baseline file contains the paired Railway samples, and the CLI derives the minimum required paint FPS from those samples before calling `evaluate`. It prints one final JSON report containing `passed`, `failed_gates`, segment count, minimum paint FPS, maximum segment milliseconds, minimum buffer, first/last rendered frame, and HTTP failure count. It writes samples incrementally so a killed benchmark remains auditable.

- [ ] **Step 4: Run qualification tests**

Run: `python3 -m unittest -v deploy/hetzner/test_qualify.py`

Expected: PASS.

- [ ] **Step 5: Document exact operator workflow**

Add README commands for secret creation with `install -m 600`, Compose startup, qualification profile startup, `qualify.py`, relay startup, log inspection, reboot recovery, Twitch key rotation, and immediate shutdown. Explicitly state that the stream key is never passed on the command line or committed.

- [ ] **Step 6: Run documentation and bundle checks**

Run: `git diff --check`

Expected: no errors.

Run: `python3 -m unittest -v deploy/hetzner/test_verify_bundle.py deploy/hetzner/test_qualify.py`

Expected: PASS.

- [ ] **Step 7: Commit qualification tooling**

```bash
git add deploy/hetzner/qualify.py deploy/hetzner/test_qualify.py README.md
git commit -m "test: automate livestream host qualification"
```

---

### Task 4: Add runtime provider selection to the personal website backend

**Files (in `/Users/worthy/Test/personal`):**
- Create: `backend/livestream.py`
- Modify: `backend/main.py`
- Create: `backend/test_livestream.py`

**Interfaces:**
- Environment: `LIVESTREAM_PROVIDER` is `hls` or `twitch`; `TWITCH_CHANNEL` is a public Twitch login; `LIVE_STREAM_URL` defaults to `https://live.worthyrae.com`.
- Endpoint: `GET /api/livestream/config`.
- Response union: `{ "provider": "twitch", "channel": string }` or `{ "provider": "hls", "base_url": string }`.
- Invalid or incomplete Twitch configuration safely resolves to HLS.

- [ ] **Step 1: Create an isolated website worktree and correctly dated branch**

Use `superpowers:using-git-worktrees`. Confirm `/Users/worthy/Test/personal` is clean first. Create a branch following `worthy/DD_MM_YYYY/twitch_livestream_embed` with the local creation date.

- [ ] **Step 2: Write backend configuration tests**

```python
class LivestreamConfigTests(unittest.TestCase):
    def test_twitch_requires_a_valid_channel(self):
        self.assertEqual(public_config({"LIVESTREAM_PROVIDER": "twitch", "TWITCH_CHANNEL": "worthy_rae"}),
                         {"provider": "twitch", "channel": "worthy_rae"})
        self.assertEqual(public_config({"LIVESTREAM_PROVIDER": "twitch", "TWITCH_CHANNEL": "bad/channel"})["provider"], "hls")

    def test_hls_is_the_safe_default(self):
        self.assertEqual(public_config({}),
                         {"provider": "hls", "base_url": "https://live.worthyrae.com"})
```

Add a FastAPI `TestClient` test asserting status 200, `Cache-Control: no-store`, and no secret-valued field.

- [ ] **Step 3: Run tests and confirm module failure**

Run from the personal worktree: `python3 -m unittest -v backend/test_livestream.py`

Expected: FAIL because `backend.livestream` does not exist.

- [ ] **Step 4: Implement strict public configuration**

```python
CHANNEL = re.compile(r"^[A-Za-z0-9_]{1,25}$")

def public_config(env: Mapping[str, str]) -> dict[str, str]:
    provider = env.get("LIVESTREAM_PROVIDER", "hls").strip().lower()
    channel = env.get("TWITCH_CHANNEL", "").strip()
    if provider == "twitch" and CHANNEL.fullmatch(channel):
        return {"provider": "twitch", "channel": channel}
    return {"provider": "hls", "base_url": env.get("LIVE_STREAM_URL", "https://live.worthyrae.com").rstrip("/")}
```

Add the endpoint before the SPA fallback mount and return a `Response`/`JSONResponse` with `Cache-Control: no-store`.

- [ ] **Step 5: Run backend tests**

Run: `python3 -m unittest -v backend/test_livestream.py`

Expected: PASS.

- [ ] **Step 6: Commit runtime configuration**

```bash
git add backend/livestream.py backend/main.py backend/test_livestream.py
git commit -m "feat: expose runtime livestream provider"
```

---

### Task 5: Add the Twitch embed while preserving instant HLS rollback

**Files (in the personal website worktree):**
- Create: `src/components/LiveStream/liveProvider.ts`
- Create: `src/components/LiveStream/TwitchLiveStreamPlayer.tsx`
- Rename: `src/components/LiveStream/LiveStreamPlayer.tsx` -> `src/components/LiveStream/HlsLiveStreamPlayer.tsx`
- Create: `src/components/LiveStream/LiveStreamPlayer.tsx`
- Modify: `src/components/LiveStream/__tests__/LiveStreamPlayer.test.tsx`
- Create: `src/components/LiveStream/__tests__/liveProvider.test.ts`
- Create: `src/components/LiveStream/__tests__/TwitchLiveStreamPlayer.test.tsx`
- Create: `src/components/LiveStream/__tests__/LiveStreamProvider.test.tsx`

**Interfaces:**
- `LiveProvider = { provider: 'twitch'; channel: string } | { provider: 'hls'; baseUrl: string }`.
- `loadLiveProvider(signal: AbortSignal) -> Promise<LiveProvider>` fetches same-origin `/api/livestream/config`.
- `TwitchLiveStreamPlayerProps = { channel: string; fallbackUrl: string; fill?: boolean; hostname?: string }`.
- The existing public `LiveStreamPlayerProps` and `ArtMedia` call site remain compatible.

- [ ] **Step 1: Rename the existing implementation and keep its tests green**

Rename the component and export to `HlsLiveStreamPlayer`. Update the existing test import and JSX names only.

Run: `npm test -- --run src/components/LiveStream/__tests__/LiveStreamPlayer.test.tsx`

Expected: all existing HLS behavior tests PASS without assertion changes.

- [ ] **Step 2: Write strict provider parser tests**

```typescript
it('accepts only a valid Twitch channel', () => {
  expect(parseLiveProvider({ provider: 'twitch', channel: 'worthy_rae' })).toEqual({ provider: 'twitch', channel: 'worthy_rae' })
  expect(parseLiveProvider({ provider: 'twitch', channel: 'bad/channel' })).toBeNull()
})

it('accepts only an https HLS origin', () => {
  expect(parseLiveProvider({ provider: 'hls', base_url: 'https://live.worthyrae.com' })).toEqual({ provider: 'hls', baseUrl: 'https://live.worthyrae.com' })
  expect(parseLiveProvider({ provider: 'hls', base_url: 'javascript:alert(1)' })).toBeNull()
})
```

- [ ] **Step 3: Implement the parser and abortable loader**

Reject unknown fields by behavior, require HTTPS outside localhost tests, strip trailing slashes, and throw `Error('Invalid livestream configuration')` for invalid responses. Fetch with `{ cache: 'no-store', signal }`.

- [ ] **Step 4: Write Twitch embed tests**

```typescript
it('binds Twitch to the exact embedding hostname', () => {
  render(<TwitchLiveStreamPlayer channel="worthy_rae" fallbackUrl={fallbackUrl} hostname="worthyrae.com" />)
  expect(screen.getByTitle('Abbey Road painted livestream')).toHaveAttribute(
    'src',
    'https://player.twitch.tv/?channel=worthy_rae&parent=worthyrae.com&autoplay=true&muted=true',
  )
})

it('keeps the approved fallback beneath the Twitch iframe', () => {
  render(<TwitchLiveStreamPlayer channel="worthy_rae" fallbackUrl={fallbackUrl} />)
  expect(screen.getByTestId('fallback-video')).toHaveAttribute('src', fallbackUrl)
  expect(screen.getByTestId('twitch-player')).toHaveAttribute('allowFullScreen')
})
```

- [ ] **Step 5: Implement the Twitch component**

Construct the URL with `URLSearchParams`; never interpolate unsanitized query text. Use `hostname ?? window.location.hostname` as Twitch's required `parent`. Render the current muted looping fallback first and an absolute full-size iframe above it with `allow="autoplay; fullscreen"`, `allowFullScreen`, `referrerPolicy="strict-origin-when-cross-origin"`, and the existing `fill` versus `aspect-video` layout contract.

- [ ] **Step 6: Write provider wrapper tests**

Mock `loadLiveProvider` and assert:

```typescript
it('selects Twitch from runtime config', async () => {
  vi.mocked(loadLiveProvider).mockResolvedValue({ provider: 'twitch', channel: 'worthy_rae' })
  render(<LiveStreamPlayer fallbackUrl={fallbackUrl} baseUrl="https://live.worthyrae.com" />)
  expect(await screen.findByTestId('twitch-player')).toBeInTheDocument()
})

it('selects the existing HLS player for rollback', async () => {
  vi.mocked(loadLiveProvider).mockResolvedValue({ provider: 'hls', baseUrl: 'https://live.worthyrae.com' })
  render(<LiveStreamPlayer fallbackUrl={fallbackUrl} />)
  expect(await screen.findByTestId('live-stream-player')).toBeInTheDocument()
})

it('falls back to configured HLS when runtime config fails', async () => {
  vi.mocked(loadLiveProvider).mockRejectedValue(new Error('offline'))
  render(<LiveStreamPlayer fallbackUrl={fallbackUrl} baseUrl="https://live.worthyrae.com" />)
  expect(await screen.findByTestId('live-stream-player')).toBeInTheDocument()
})
```

- [ ] **Step 7: Implement the provider wrapper**

Load configuration in an effect with `AbortController`. Until resolved, show the existing fallback video. Select `TwitchLiveStreamPlayer` for Twitch and `HlsLiveStreamPlayer` otherwise. On fetch failure, use the `baseUrl` prop, preserving the current behavior and rollback path.

- [ ] **Step 8: Run focused and full website verification**

Run: `npm test -- --run src/components/LiveStream/__tests__/liveProvider.test.ts src/components/LiveStream/__tests__/TwitchLiveStreamPlayer.test.tsx src/components/LiveStream/__tests__/LiveStreamProvider.test.tsx src/components/LiveStream/__tests__/LiveStreamPlayer.test.tsx`

Expected: PASS.

Run: `npm test`

Expected: PASS.

Run: `npm run build`

Expected: TypeScript and Vite build PASS.

Run: `npm run lint`

Expected: PASS or only pre-existing, documented lint failures; no new lint error in changed files.

- [ ] **Step 9: Commit the reversible Twitch player**

```bash
git add src/components/LiveStream backend
git commit -m "feat: add reversible Twitch livestream player"
```

---

### Task 6: Provision and privately qualify the hourly Hetzner candidate

**Files:**
- Use: `deploy/hetzner/cloud-init.yaml`
- Use: `deploy/hetzner/compose.yaml`
- Generate outside Git: `/etc/abbey-road-stream/painter.env`
- Generate outside Git: `/etc/abbey-road-stream/twitch_stream_key`
- Artifact outside Git: `artifacts/hetzner/railway-baseline.json`
- Artifact outside Git: `artifacts/hetzner/qualification.jsonl`
- Artifact outside Git: `artifacts/hetzner/qualification-report.json`

**Interfaces:**
- Hetzner server: `abbey-road-stream-candidate`, location `nbg1`, type `cx43`, Ubuntu 24.04.
- Benchmark is private and uses the `qualify` Compose profile; it does not connect to Twitch.
- Qualification threshold is the Railway paint FPS observed immediately before the paired run, never a lower invented target.

- [ ] **Step 1: Resolve account resources without creating a server**

Authenticate `hcloud` interactively without printing the API token. Run `hcloud server-type describe cx43`, `hcloud location describe nbg1`, `hcloud ssh-key list`, and `hcloud server list`. Record availability, the exact hourly/monthly price shown by Hetzner, the SSH key ID, and confirm no candidate already exists.

- [ ] **Step 2: Create the firewall and hourly candidate**

Resolve the administrator's current public IP separately and validate it as one IPv4 `/32`. Create firewall `abbey-road-stream-ssh` with inbound TCP 22 only from that `/32` and unrestricted outbound traffic. Create `abbey-road-stream-candidate` using CX43, Ubuntu 24.04, Nuremberg, the existing SSH key, the firewall, and `deploy/hetzner/cloud-init.yaml` as user data.

Immediately verify in the Hetzner response that billing is hourly and the monthly cap is below $30 before tax. If CX43 is unavailable or exceeds the cap, delete no existing resource and stop for user direction.

- [ ] **Step 3: Verify host bootstrap and architecture**

Run over SSH: `uname -m`, `lscpu`, `docker version`, `docker compose version`, `sudo ufw status verbose`, and `sudo ss -lntup`.

Expected: `x86_64`; Docker and Compose available; only SSH publicly listening; UFW active.

- [ ] **Step 4: Transfer the exact candidate source and create non-secret environment**

Use `rsync` over SSH from the current clean livestream worktree to `/opt/abbey-road-stream/repo`, excluding `.git`, `artifacts`, target directories, caches, and local environment files. On the host, install `painter.env.example` to `/etc/abbey-road-stream/painter.env` with root ownership and mode `0600`, then copy the current non-Railway `LIVE_*`, `LOADED_*`, encoding, width, and source values from production. Confirm the file contains no `RAILWAY_*` names.

Do not create the Twitch key file yet. Start only `painter` and `qualify`.

- [ ] **Step 5: Establish the paired Railway baseline**

Query Railway `/api/live/status` for at least ten newly completed segments while Hetzner warms on the same live scene. Record minimum `engine.paint_fps`, maximum `engine.total_ms`, buffer count, deployment ID, and timestamps in `artifacts/hetzner/railway-baseline.json`.

- [ ] **Step 6: Run the private one-hour Hetzner qualification**

Run the Compose qualification sink and, locally over an SSH tunnel to `127.0.0.1:18080`, run:

```bash
python3 deploy/hetzner/qualify.py \
  --url http://127.0.0.1:18080/api/live/status \
  --seconds 3600 --interval 6 \
  --baseline artifacts/hetzner/railway-baseline.json \
  --output artifacts/hetzner/qualification.jsonl
```

- [ ] **Step 7: Inspect host contention and acceptance report**

Collect `docker stats --no-stream`, `free -h`, `uptime`, `journalctl -u docker --since '-70 minutes'`, and Compose logs with Twitch secrets still absent. The candidate passes only when `qualification-report.json` says `passed: true`, no container restarted, and host logs show no OOM, throttling, or disk error.

If it fails, save the report, stop Compose, delete `abbey-road-stream-candidate`, and report the exact failed gate. Do not reduce painter quality or deploy the website.

- [ ] **Step 8: Verify reboot recovery privately**

Reboot the candidate once. Confirm Docker starts, the painter becomes healthy, the qualification sink can reconnect, the buffer refills, and no port except SSH is externally reachable.

---

### Task 7: Connect Twitch and obtain explicit visual approval

**Files:**
- Generate outside Git: `/etc/abbey-road-stream/twitch_stream_key`
- Artifact outside Git: `artifacts/hetzner/twitch-approval.mp4`
- Artifact outside Git: `artifacts/hetzner/twitch-approval.jpg`

**Interfaces:**
- Twitch channel comes from the user's existing account.
- Twitch key is transferred directly from authenticated Twitch UI to the root-only server file without appearing in chat, shell history, Git, logs, or Compose output.
- This task ends at a user approval gate; it does not deploy the website.

- [ ] **Step 1: Confirm Twitch account settings without revealing credentials**

Use the authenticated Twitch dashboard to confirm the channel login, ingest selection, mature-content setting, archive preference, and stream title/category. Do not change publicity or monetization settings unrelated to this stream.

- [ ] **Step 2: Install the stream key through a non-echoing path**

Create `/etc/abbey-road-stream/twitch_stream_key` as root with mode `0600`. Verify only file mode and nonzero byte count; never display its contents. Run `docker compose config` and confirm the secret value is absent.

- [ ] **Step 3: Start the relay and verify Twitch ingest**

Start `relay` only after `painter` is healthy. Confirm FFmpeg progress updates, Twitch Inspector receives 1920 x 1080 at approximately 29.97 FPS, the connection remains stable for at least 20 minutes, keyframe spacing is accepted, and audio is a silent AAC track rather than missing or noisy audio.

- [ ] **Step 4: Capture the Twitch-player presentation**

Record a short browser-visible sample from the Twitch player and extract a representative still. Do not substitute the direct local HLS output; approval concerns Twitch's re-encoded presentation, controls, latency, and branding.

- [ ] **Step 5: Stop for explicit user approval**

Present the Twitch sample with the current Railway presentation identified clearly. Ask the user to approve or reject the Twitch presentation. Do not change the website, Railway, DNS, or service status until the user explicitly approves.

---

### Task 8: Deploy the reversible website cutover

**Files:**
- Deploy the tested personal website branch.
- Runtime variables on the personal Railway service: `LIVESTREAM_PROVIDER`, `TWITCH_CHANNEL`, `LIVE_STREAM_URL`.

**Interfaces:**
- Twitch cutover: `LIVESTREAM_PROVIDER=twitch`; `TWITCH_CHANNEL` is set to the channel login returned by the authenticated Twitch dashboard in Task 7.
- Immediate rollback: `LIVESTREAM_PROVIDER=hls`, `LIVE_STREAM_URL=https://live.worthyrae.com`.
- No frontend rebuild is required to switch providers; restarting/redeploying the website backend is sufficient.

- [ ] **Step 1: Merge or deploy the verified personal website change**

Review the personal repository diff, confirm its tests and build passed, then deploy that exact commit to the existing `personal` Railway service. Do not alter the livestream service.

- [ ] **Step 2: Verify HLS rollback mode first**

Set runtime provider to HLS and verify `GET https://worthyrae.com/api/livestream/config` returns the HLS union with `Cache-Control: no-store`. Open the artwork page and confirm it still uses the current Railway player.

- [ ] **Step 3: Switch runtime configuration to Twitch**

Set `LIVESTREAM_PROVIDER=twitch` and `TWITCH_CHANNEL` to the validated channel login. Restart only the personal website service. Verify the public config endpoint contains the public channel login and no stream key or ingest URL.

- [ ] **Step 4: Validate the production page**

Check desktop and mobile layouts on `worthyrae.com`, fullscreen behavior, autoplay-muted behavior, Twitch controls, the fallback layer, HTTPS, console errors, and the required Twitch `parent=worthyrae.com` parameter. If `www.worthyrae.com` serves the page, verify its embed uses `parent=www.worthyrae.com` dynamically.

- [ ] **Step 5: Exercise rollback once**

Set the provider to HLS, restart the personal service, and verify the old player returns. Set it back to Twitch only after the rollback test succeeds. Record the tested commit and Railway deployment IDs.

---

### Task 9: Observe for 24 hours and stop Railway livestream spend

**Files:**
- No repository changes expected.
- Preserve local artifacts: qualification report, Twitch sample, deployment IDs, and final cost snapshot.

**Interfaces:**
- Healthy: Twitch remains live, painter segments advance, buffer remains stable, cadence does not fall below the accepted Railway baseline, and containers do not restart repeatedly.
- Rollback: runtime provider HLS plus existing Railway livestream deployment.

- [ ] **Step 1: Create a quiet 24-hour heartbeat monitor**

Use the product heartbeat mechanism attached to this task. Check Hetzner/Twitch health periodically, stay quiet while healthy, and notify only on stream failure, repeated restart, cadence regression, required user action, or completion of the 24-hour window. Do not create a standalone workaround cron job.

- [ ] **Step 2: Record resource and cost evidence during the window**

At the start, midpoint, and end, capture container uptime/restarts, CPU, memory, buffer count, paint FPS, segment total milliseconds, Hetzner traffic, and Twitch availability. Confirm the projected monthly host charge remains below $30 before tax.

- [ ] **Step 3: Roll back immediately on a material failure**

Set the website provider to HLS and restart only the personal service. Keep the Hetzner candidate and logs intact for diagnosis. Do not alter painter behavior to conceal a failed capacity gate.

- [ ] **Step 4: Stop the Railway livestream after 24 healthy hours**

Resolve the exact Railway project, environment, service, and deployment IDs read-only. Stop the `livestream-morphing` deployment while preserving the `personal` service, project, variables, and deployment history. Verify the Twitch embed remains live after Railway stops.

- [ ] **Step 5: Confirm the final cost and recovery posture**

Check Railway usage after shutdown and Hetzner's current hourly/monthly estimate. Report the final recurring projection, the stopped Railway deployment ID, the Hetzner server ID, the personal website deployment ID, the runtime rollback setting, and how to rotate the Twitch key.

---

## Final verification matrix

- Livestream repository: full Rust tests pass; deployment and qualification Python tests pass; Docker images build; Compose renders without secrets.
- Personal repository: backend unit tests, focused player tests, full Vitest suite, TypeScript/Vite build, and changed-file lint pass.
- Host: x86_64, private painter port, SSH-key access, active firewall, restart-on-boot, no secret in logs or Compose output.
- Painter: 1920 x 1080, 30000/1001 output FPS, accepted paint FPS, stable ten-segment buffer, no quality-setting changes.
- Twitch: stable 20-minute ingest, accepted keyframes, silent AAC, user-approved re-encoded presentation.
- Website: runtime Twitch/HLS switch works both ways, correct `parent`, mobile/desktop/fullscreen verified.
- Operations: 24 healthy hours precede Railway shutdown; personal service and rollback history remain intact.
