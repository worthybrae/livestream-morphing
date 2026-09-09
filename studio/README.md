# Living Ink studio

The studio previews a captured source clip through the same native Rust renderer as the public stream. Slider changes stay in a browser draft. Click **Render preview** to run one bounded local render; no Railway request is made. Named versions are saved in browser storage. **Export for production** downloads a versioned JSON preset.

Use Node 22.20+ or Node 24 for this Vite 8 studio:

```sh
npm install
npm run build
```

Run the Rust service locally with `STUDIO_SOURCE=/absolute/path/to/source.ts` and optionally `STUDIO_WIDTH=960` (default 640). With the service running from `livestream-morphing-rs`, its default port is 3000 and it serves `../studio/dist`. Open `http://localhost:3000`. Use a short, captured source segment, under 8 MB and 720 frames. The studio render is CPU work and temporarily allocates a decoded RGB batch. Only one preview render runs at once.

For Vite development, `npm run dev` proxies to port 3000. Set `ALLOWED_ORIGIN=http://localhost:5173` when running the native service separately.

## Publish a reviewed preset

Copy the exported JSON's `params` object into `livestream-morphing-rs/src/living_ink_preset.json`, replay and benchmark it, then deploy the Rust service. The preset is compiled into that release, so rollback also restores its appearance. The studio does not have production credentials or a remote mutation endpoint. `PUBLIC_MODE=true` excludes all studio/admin routes.

## Offline comparison

The built Rust executable supports:

```sh
morphing render source.ts ink.ts 640
morphing render source.ts legacy.ts 640 legacy
morphing render source.ts draft.ts 960 ink params.json
```

Output is MPEG-TS and a JSON metrics line. Use the `params` object alone for `params.json`. Decode, render and encode times are local measurements, not Railway billing data. Production remains at 640×360 unless `OUTPUT_WIDTH` is explicitly set and measured; the maximum is 960. No browser shader or alternate renderer is used.
