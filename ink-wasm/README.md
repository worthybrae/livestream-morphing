# Browser drawing engine

This crate includes the native LivingInk and Distortion source files directly. Shared frame and effect types live in the native source tree. There is no JavaScript drawing-effect implementation. The browser worker handles video pixel transfer; Rust handles the image processing.

Build with Rust and Node installed:

```sh
rustup target add wasm32-unknown-unknown
RUSTFLAGS='-C target-feature=+simd128' cargo build --manifest-path ink-wasm/Cargo.toml --release --target wasm32-unknown-unknown
node ink-wasm/publish-assets.mjs /absolute/path/to/portfolio
cargo test --manifest-path ink-wasm/Cargo.toml
cargo test --manifest-path livestream-morphing-rs/Cargo.toml
```

The publishing script copies the compiled binary and derives slider metadata directly from that binary. Keep Cargo.lock committed. The portfolio's `inkWasm.test.js` validates the shipped binary and metadata together.

The recorded source MP4 is a stream-copy remux of the same local capture used for the rendered studies. No runtime rendering service or source URL proxy is exposed. Source/media and WASM load only after the visitor starts the studio.

The worker owns one renderer. Only one frame can be in flight, so slow devices skip preview frames rather than accumulate a queue. Explicit media time drives the flow phase; elapsed source time drives trail decay. Source loops and parameter changes clear temporal history. Paused edits redraw once. Visibility changes pause playback, and reentry renders pending edits. Users can choose 1080p, 720p, or 540p; reported preview FPS is measured from processed video frames. These figures are independent of the 1080p/29.97 live stream.

WebAssembly SIMD and OffscreenCanvas are required. The existing nine recorded studies remain available as a fallback. Preview performance is device-dependent; this is an interactive tuning surface, not a guarantee of 1080p/30 playback on every visitor's CPU.
