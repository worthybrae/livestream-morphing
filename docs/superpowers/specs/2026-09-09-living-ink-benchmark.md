# Living Ink benchmark and release decision

Captured Abbey Road source on 2026-09-09: 1920×1080, 30000/1001 fps, 180 frames, 6.006 seconds. Tests run in the existing ARM64 Linux Docker builder on the local Mac, not Railway. Isolated runs exclude concurrent compilation. These are single-clip measurements, not p95 across different scenes or a guaranteed monthly bill.

| Native pipeline | Output | Frame p50 / p95 | Entire segment | Output bytes |
|---|---|---|---|---|
| Living Ink | 640×360 | 8.96 / 9.61 ms | 3.009 s | 263,952 |
| Living Ink | 960×540 | 19.18 / 20.76 ms | 5.554 s | 402,884 |
| Legacy filters, directly resized | 640×360 | 5.87 / 5.97 ms | 2.173 s | 777,004 |

360p Living Ink CPU time: 3.073 user + 0.388 system seconds per 6.006 source seconds (shell time includes startup). 540p CPU: 5.974 + 0.350 seconds. The new renderer costs more CPU than the legacy filters but its sampled 360p stream is 66% smaller. The old production path additionally retained a 1280×720 decoded batch and downsampled it; this comparison deliberately isolates the effect cost.

Local preview process peaked at 233,764 KiB for one clip. With local live ingestion and a simultaneous studio render, peak process RSS reached 445,152 KiB (~435 MiB). After the viewer closed and the one-minute idle grace expired, RSS fell to 67,604 KiB (~66 MiB). Linux/glibc malloc_trim returns unused codec allocator pages at idle. No active frame data is discarded by trim.

## Monthly estimate

Assumptions: 50 active source hours, 50–250 viewer-hours (1–5 simultaneous viewers), sampled 0.352 Mbps encoded bitrate, 66 MiB idle RSS, conservatively 435 MiB active RSS, 0.576 average active CPU cores, and 720 billing hours/month. Railway public rates checked September 9: CPU $20/vCPU-month, RAM $10/GB-month, outbound $0.05/GB. Units and platform accounting can differ slightly.

- CPU approximately $0.80.
- Memory approximately $0.90, using the conservative active RSS above.
- Egress approximately $0.40–$1.98.
- Total approximately $2.10–$3.70; with 20% headroom, approximately $2.50–$4.45.

This is stream resource usage only. Hobby has a $5 minimum including resource usage, not an additional $5 charge on top of covered usage. Other project services may consume that allowance. Longer viewing, busier imagery, different CPU performance or idle memory increase cost. This implementation does not enforce a hard account spending cap.

Sources: https://docs.railway.com/pricing and https://docs.railway.com/guides/right-size-cpu-memory

## Decision

Deploy 640×360 on the existing Railway service, no additional infrastructure. Retain 960×540 as a local studio option: its measured 5.554 seconds per 6.006-second segment leaves inadequate production headroom. Stop demand when hidden or scrolled out of view; stop ingestion after one minute without requests. Metrics never wake the engine. Presets export for a reviewed release; the public service has no studio mutation routes.

Source timing is verified for the current constant-frame-rate source. First source timestamps detect segment gaps and reset trails. Variable-frame-rate source support is not claimed. HLS uses the source's advertised fixed 13-second upper bound and actual EXTINF durations; segments longer than that are rejected. Browser hls.js uses an explicit 18-second live-sync distance.

## Verification

98 Rust tests; 91 portfolio tests; studio and portfolio builds passed. Local browser playback advanced; artwork filled the initial viewport behind navigation; scrolling removed the entire hero. Local studio rendered and played a draft using the native renderer, without production credentials. The earlier build-contention timing samples were discarded in favor of the isolated measurements above.
