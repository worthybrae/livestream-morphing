# Livestream Morphing

A real-time video processing system that transforms the Abbey Road webcam livestream with artistic effects. Features a public viewer and admin interface for live stylization control.

The portfolio stream uses the Rust service in `livestream-morphing-rs` and
Railway Serverless. Production CDN delivery is pending (see below). It does not
use the legacy Python, Celery, Redis, S3, or R2 deployment instructions below.
See [Production deployment](#production-deployment-railway) for the current runtime
contract and rollout status.

## 🎯 What It Does

Captures the Abbey Road webcam stream, applies artistic "carbonized" blob effects (blur, quantization, psychedelic distortion), and serves it back as an HLS stream with time-based color schemes that change throughout the day.

## 📁 Project Structure

```
livestream-morphing/
├── backend/                    # FastAPI + Video Processing
│   ├── api/
│   │   ├── stream.py          # Stream endpoints (HLS playlist serving)
│   │   └── admin.py           # Admin API (config management)
│   ├── core/
│   │   ├── processor.py       # Main stream processor (replaces old tasks.py)
│   │   ├── image_processing.py # OpenCV effects (blur, quantize, morph, distortion)
│   │   └── config.json        # Runtime stylization config (gitignored)
│   ├── utils/
│   │   └── video_creation.py  # Batch video generation utilities
│   ├── main.py                # FastAPI app entry point
│   └── requirements.txt       # Python dependencies
│
├── frontend/                   # Public Viewer UI (TODO)
│   ├── src/
│   └── index.html
│
├── admin/                      # Admin UI for Stylization (TODO)
│   └── src/
│
├── data/                       # Runtime data (gitignored)
│   ├── frames/                # Processed frames per segment
│   ├── segments/              # Downloaded .ts segments
│   └── outputs/               # Final compiled videos
│
├── research/                   # Experimental iterations
└── [old root files]           # Legacy files (to be removed after migration)
```

## 🚀 How to Run

### Option 1: Legacy Python Setup (Uses Celery; not the portfolio deployment)

**1. Setup Virtual Environment:**
```bash
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

**2. Start Redis (required for Celery):**
```bash
redis-server
```

**3. Start Celery Workers (in separate terminals):**
```bash
# Terminal 1 - Beat scheduler (runs every 2 seconds)
celery -A tasks beat --loglevel=info

# Terminal 2 - Fetch new segments queue
celery -A tasks worker --loglevel=info --concurrency=1 -Q fns

# Terminal 3 - Download segments queue
celery -A tasks worker --loglevel=info --concurrency=2 -Q ds

# Terminal 4 - Process segments queue (CPU intensive)
celery -A tasks worker --loglevel=info --concurrency=3 -Q ps

# Terminal 5 - Generate M3U8 queue
celery -A tasks worker --loglevel=info --concurrency=2 -Q gm

# Optional - Flower monitoring UI
celery -A tasks flower
```

**4. Start FastAPI:**
```bash
uvicorn api:app --reload
```

**5. Access the stream:**
- Viewer: http://localhost:8000
- Stream endpoint: http://localhost:8000/stream
- Flower (if running): http://localhost:5555

---

### Option 2: Legacy Python Backend Structure (WIP)

**Note:** These are historical Python setup notes. Backend code needs import
updates to work. Do not provision AWS or R2 credentials for the Rust portfolio
deployment.

**1. Setup:**
```bash
cd backend
pip install -r requirements.txt
```

**2. Configure environment:**
```bash
# Create .env file in project root
cat > ../.env << EOF
AWS_PUB_KEY=your_access_key
AWS_SECRET_KEY=your_secret_key
# Or for Cloudflare R2:
R2_ACCESS_KEY=your_r2_key
R2_SECRET_KEY=your_r2_secret
R2_ACCOUNT_ID=your_account_id
EOF
```

**3. Run backend:**
```bash
cd backend
uvicorn main:app --reload --port 8000
```

**API Endpoints:**
- `GET /api/stream` - HLS playlist
- `GET /api/admin/config` - Get current stylization config
- `POST /api/admin/config` - Update config
- `POST /api/admin/config/reset` - Reset to defaults
- `GET /api/admin/status` - Processor status
- `GET /health` - Health check

---

## Production deployment (Railway)

Railway builds the Rust service from the repository root using `Dockerfile.railway`
and checks `GET /health` before serving traffic. Configure the production service
with these settings:

```text
PUBLIC_MODE=true
ALLOWED_ORIGIN=https://worthyrae.com
PORT is supplied by Railway
STREAM_URL is optional and defaults to the Abbey Road source
Serverless must be enabled for the livestream-morphing service
```

The portfolio uses the generated Railway HTTPS origin below. The local studio is
mounted only when `PUBLIC_MODE` is `false`.

### Deployment targets and controls (2026-09-09)

- Existing Railway project: `lavish-energy`, production environment, in the same
  Hobby workspace as `personal` (`worthyrae.com`).
- Stream service: `livestream-morphing` (`ec75829f-b72a-49ff-b7c8-879afb9dfb4b`).
- Origin: `https://livestream-morphing-production.up.railway.app`.
- Optional future branded domain: `https://live.worthyrae.com`. Its CNAME target is
  `fk1nlch4.up.railway.app`.
- Serverless is enabled (`sleepApplication: true`). The pipeline releases its
  source work after five minutes without viewer requests.
- The shared workspace has a $5 soft usage alert and no hard limit. Railway
  rejected the planned $4 alert because its minimum nonzero threshold is $5.
  A soft alert sends email; it does not enforce a $5 service budget or stop the
  portfolio. Other services consume the same included Hobby usage allowance.

### Delivery decision

The tuned direct-origin cost gate passes. The portfolio build-time variable is
`VITE_LIVE_STREAM_URL=https://livestream-morphing-production.up.railway.app`.
This launch does not require AWS, R2, or a Cloudflare CDN cache.

Custom-domain branding remains optional and pending DNS access.
The authenticated Cloudflare account contains `streamclout.io`, not
`worthyrae.com`. Public DNS for `worthyrae.com` uses
`dns1.registrar-servers.com` and `dns2.registrar-servers.com`, so its DNS is not
currently hosted by Cloudflare. The intended hostname needs access to the
current DNS provider. No nameserver migration has been attempted.

The original plain Cloudflare proxy/cache proposal also needs revision.
[Cloudflare's video-delivery policy](https://developers.cloudflare.com/fundamentals/reference/policies-compliances/delivering-videos-with-cloudflare/)
does not permit externally hosted video delivery on an ordinary Free, Pro, or
Business CDN plan without an appropriate video service. Do not enable a generic
orange-cloud HLS cache as a substitute. Direct Railway HTTPS delivery was selected
after measuring the tuned five-viewer workload below.
[Cloudflare's explanation of its terms](https://blog.cloudflare.com/updated-tos/)
allows video hosted by its Stream or R2 services. No R2 resources or credentials
have been created by this rollout.

When deploying from an isolated Git worktree, pass its path explicitly:

```bash
railway up "$PWD" --path-as-root --service livestream-morphing --environment production --detach
```

The service also has `dockerfilePath: Dockerfile.railway` set in Railway. The first
CLI upload without the explicit root selected the legacy Python Dockerfile and
failed before serving traffic. Confirm the build log names `Dockerfile.railway`.
Railway currently accepts `railway.json` but warns that Config as Code support
ends on 2026-12-01; migrate the configuration before that date.

### Rollback

Remove `VITE_LIVE_STREAM_URL` from the existing `personal` Railway service and
redeploy the portfolio. Verify that `/art/livestream-art` plays its recording.
Only then stop the `livestream-morphing` service's deployment. Keep this order so
the art page remains usable throughout rollback. A deployment restart or a new
deployment can restore the stream service; no persistent stream data is needed.

### Verification status

The tuned origin serves H.264 at 640 × 360, 30 FPS. Public status and playlist
responses are `no-store`; successful MPEG-TS responses are immutable for one
hour; missing segments are `no-store`. The configured origin is exactly
`https://worthyrae.com`, and `PUT /api/source` returns 404 in public mode.

Local verification on 2026-09-09 passed 87 Rust tests, 82 portfolio tests, the
portfolio production build, and the fallback-only build plus 12 focused media
tests. Focused ESLint has no errors and one pre-existing `Home.tsx` hook warning.
Strict Rust clippy (with baseline `dead_code` allowed) reports 53 existing lint
errors on both the original checkout and feature worktree; it is not a passing
gate. No effect-code cleanup was included in this deployment task.

The single cost-tuning pass caps decoded RGB frames at 1280 × 720 before retaining
them in memory, so the existing 2x stage produces 640 × 360 for Abbey Road. Smaller
sources keep their original dimensions; larger sources retain their aspect ratio
within rounding required for H.264. A real encode/decode regression test failed
on the original 1920 × 1080 behavior, then passed with the cap. The tuned service
passes all 87 Rust tests; strict clippy still reports the same 53 baseline errors.

### Measured cost gate

The one authorized tuning pass reduced Abbey Road output from 960 × 540 to
640 × 360. The tuned direct Railway deployment passes the $5 incremental
resource target for 50 active hours in 50 one-hour sessions:

| Tuned measurement / projection | 1 continuous viewer | 5 continuous viewers |
| --- | ---: | ---: |
| Decoded test duration per client | 5:30 | 10:30 |
| Average CPU | 0.5254 vCPU | 0.5249 vCPU |
| Average RAM | 0.6958 GB | 0.6981 GB |
| Generated video | 0.2200 GB/hour | 0.2085 GB/hour |
| Mean / maximum processing per 6-second segment | 2.638 / 2.898 s | 2.662 / 2.897 s |
| 50-hour viewer egress | 11.00 GB | 52.13 GB |
| Cost with 10-minute session overhead | $1.95 | $4.00 |
| Cost with 15-minute session overhead | $2.05 | $4.10 |

The tuned one-viewer run was 19:24:18–19:29:40 UTC, and the five-viewer run was
19:29:40–19:40:00 UTC on 2026-09-09. The 54 and 104 segments respectively had
zero cadence overruns. Each decoder exited successfully with zero dropped or
duplicated frames. All five clients decoded 18,900 frames; each recovered from
two TLS EOF warnings. The one-client run decoded 9,902 frames and recovered from
one TLS EOF plus two initial null-muxer non-monotonic DTS warnings. These are
FFmpeg transport/decode results, not substitutes for browser acceptance.

Railway's interior five-viewer metrics window was 19:30:30–19:39:30 UTC. Memory
peaked at 0.7028 GB. Missing buckets where the CPU limit was zero were excluded;
zero CPU buckets followed by accumulated CPU samples were retained. Segment
sizes conservatively add one KiB to the logged integer KiB value. A sensitivity
using 0.65 vCPU, 0.75 GB RAM, the higher 0.2200 GB/hour payload rate, and 15-minute
overhead yields **$4.51 for five viewers**. This is not a guaranteed spending cap:
longer viewing, more sessions, retries, or more complex source video can cost more.

Post-deployment activation reached first status in 114 ms and three fresh
segments in 12.248 seconds. This is not an asleep-container cold-start measurement.
After all five clients closed at 19:40:00 UTC, production logged pipeline idle at
19:45:03 UTC, confirming the five-minute viewer lease stops processing.
Railway reported `SLEEPING` at 19:52:00 UTC (the previous 19:50:26 check was still
running). Actual sleep occurred within 10:26–12:00 after the final viewer, so the
15-minute overhead sensitivity covers the observed wake/idle lifecycle.

The subsequent production Chrome cold-start showed the recording and starting
label immediately. A transient retry label appeared around 18.5 seconds; the
page recovered automatically and was confirmed `LIVE` by 89.6 seconds, with the
live video playing (`readyState=4`, no media error) above the still-playing
recording. Exact first `canplay` time was not captured. Origin logs show activation
at 19:52:30.890 UTC, first segment at 19:52:33.816, and third at 19:52:41.211,
without source errors. This is a self-recovering cold start, not a flawless direct
transition; the transient browser failure has not been reproduced or attributed.
Safari also passed warm-service native HLS acceptance on the production page:
the transformed Abbey Road video was visible with the `LIVE` badge. Both browser
pages were closed after verification.

### Baseline, cost method, and supported alternatives

The first origin session ran on 2026-09-09 from 18:47:54 to 19:02:54 UTC.
One FFmpeg client decoded 27,000 frames (15 minutes) successfully, with no dropped
or duplicated frames. Four transient TLS EOF warnings recovered automatically.
Across 150 processed segments, mean processing time was 4.240 seconds and maximum
was 5.125 seconds; none exceeded the six-second cadence. Mean segment size was
approximately 637 KB, or 0.3824 GB/hour (rounded upward from KiB log values).
Railway reported average 0.8504 vCPU and 1.5356 GB RAM over the interior metrics
window (18:48:00–19:02:30 UTC). These are direct-origin measurements, not browser
or Cloudflare-cache acceptance results.

The original five-client session ran from 19:02:54 to 19:18:13 UTC. Each client
decoded 27,900 frames (15:30 video), exited successfully, and had zero dropped or
duplicated frames; recoverable TLS EOF warnings occurred. Its 153 processed
segments averaged 4.301 seconds (maximum 5.668 seconds), with no cadence overruns.
Average segment size was about 650 KB / 0.3902 GB/hour. Railway averaged 0.8841
vCPU and 2.5134 GB RAM (peak RAM 3.1520 GB). Direct five-viewer delivery at this
original resolution projects to about $8.30 for 50 one-hour sessions plus ten
minutes overhead each, exceeding the $5 gate and triggering the one tuning pass.

Use [Railway's current rates](https://docs.railway.com/pricing): $20/vCPU-month,
$10/GB-month RAM, and $0.05/GB egress. For 50 active hours in `N` sessions,
reserve `H = 50 + N / 6` compute hours (ten minutes overhead per session).
With average CPU `C`, average memory `M` GB, and generated video `G` GB/hour:

```text
compute = H * (C * 20 / 730 + M * 10 / 730)
direct Railway total = compute + 50 * simultaneous_viewers * G * 0.05
Railway + R2 estimate = compute + H * G * 0.05 + R2 usage
```

Ten minutes is a planning assumption, not a maximum.
[Railway's Serverless documentation](https://docs.railway.com/deployments/serverless)
allows 5–10 minutes after the last outbound packet before sleep, following the
pipeline's own five-minute idle period. Also calculate `H = 50 + N / 4` for a
15-minute overhead sensitivity, plus startup time if material.

The R2 estimate conservatively uploads during all overhead hours. Direct
delivery only sends video while viewers are present. These are incremental
resource estimates, not a promise that all services in the shared workspace
will total $5. Output complexity, session count, retries, and viewing hours can
change the bill.

[R2 Standard pricing](https://developers.cloudflare.com/r2/pricing/) includes
1 million writes, 10 million reads, and 10 GB-month storage per month, with free
egress. For 50 one-hour sessions, a six-second segment cadence means about
35,000 writes including overhead and at most 150,000 segment reads for five
continuous viewers. With short retention and unused shared R2 allowances, the
additional R2 usage is expected to fit the free tier. This estimate needs a
small R2 delivery design and validation; R2 has not been implemented. Apply an
[object lifecycle](https://developers.cloudflare.com/r2/buckets/object-lifecycles/)
and account for deletion potentially lagging expiry by 24 hours.

[Cloudflare Stream pricing](https://developers.cloudflare.com/stream/pricing/)
charges $1 per 1,000 delivered minutes. Fifty viewing hours therefore cost $3
for one continuous viewer or $15 for five, before this custom processor and
Stream storage. Storage capacity is sold in $5/1,000-minute monthly increments.
Stream does not fit the five-viewer $5 target.

---

## 🎨 Stylization Configuration

The admin API (`/api/admin/config`) allows real-time adjustments:

```json
{
  "bilateral_diameter": 15,        // Edge-preserving blur diameter
  "bilateral_sigma_color": 80,     // Color similarity
  "bilateral_sigma_space": 80,     // Spatial distance
  "gaussian_blur_size": 9,         // Blob smoothing kernel
  "quantization_levels": 8,        // Posterization levels (4-16)
  "morph_kernel_size": 3,          // Morphological operation size
  "edge_blend_factor": 0.2,        // Edge enhancement blend (0-1)
  "psychedelic_amplitude": 0.01,   // Distortion strength
  "psychedelic_frequency": 20.0,   // Distortion frequency
  "process_every_nth_frame": 3     // Skip frames for speed
}
```

### Example Presets

**Heavy Blobs (minimal detail):**
```bash
curl -X POST http://localhost:8000/api/admin/config \
  -H "Content-Type: application/json" \
  -d '{
    "bilateral_diameter": 25,
    "gaussian_blur_size": 15,
    "quantization_levels": 4,
    "edge_blend_factor": 0.0
  }'
```

**Detailed Posterization:**
```bash
curl -X POST http://localhost:8000/api/admin/config \
  -H "Content-Type: application/json" \
  -d '{
    "bilateral_diameter": 9,
    "gaussian_blur_size": 5,
    "quantization_levels": 16,
    "edge_blend_factor": 0.4
  }'
```

**Extreme Psychedelic:**
```bash
curl -X POST http://localhost:8000/api/admin/config \
  -H "Content-Type: application/json" \
  -d '{
    "psychedelic_amplitude": 0.05,
    "psychedelic_frequency": 50.0
  }'
```

---

## 🔧 Architecture Deep Dive

### Processing Pipeline

```
Abbey Road Stream (HLS)
    ↓
Fetch new segment every 2s
    ↓
Download .ts segment
    ↓
Extract frames with PyAV
    ↓
Apply effects (OpenCV):
  - Psychedelic sine-wave distortion
  - Bilateral filter (edge-preserving blur)
  - Gaussian blur (blob smoothing)
  - Quantization (posterization)
  - Morphological operations (blob shapes)
  - Edge enhancement
  - Time-based color scheme
    ↓
Encode with FFmpeg (H.264)
    ↓
Upload to S3/R2
    ↓
Generate M3U8 playlist
    ↓
Serve to clients via HLS
```

### Time-Based Color Scheme

Colors change based on London time:
- **Noon (12:00)** → Lightest grey (175)
- **Midnight (00:00)** → Darkest grey (25)
- Linear interpolation throughout the day
- Edge color contrasts with background (black/white)

### Performance Optimizations

Currently processes **every 3rd frame** and copies the rest for speed:
- Segment duration: ~6 seconds
- Frame rate: 30 FPS
- Actual frames processed: 10 FPS
- Output: 30 FPS (duplicated frames)

---

## 💰 Legacy Deployment Options (historical estimates)

These older Python/S3/R2 notes are not current pricing guidance and do not apply
to the Rust portfolio deployment. Use the measured production cost decision
above when selecting infrastructure.

### Ultra-Cheap (~$0-2/month)

- **Compute:** Oracle Cloud Free Tier (4 ARM cores, 24GB RAM - permanent free)
- **Storage:** Cloudflare R2 (10GB free, **FREE egress** - huge savings vs S3)
- **Frontend:** Vercel/Netlify/Cloudflare Pages (free)
- **Admin:** Vercel/Netlify (free)
- **Domain:** Cloudflare (free) or .xyz (~$1/month)

### Budget (~$5-10/month)

- **Compute:** Hetzner CX21 (€4.5/mo) or DigitalOcean Basic ($6/mo)
- **Storage:** Cloudflare R2
- **Frontend/Admin:** Same as above (free)

### Key Cost Killer: S3 Egress

With 10 concurrent viewers 24/7:
- **AWS S3:** ~$3,240/month (36TB × $0.09/GB egress)
- **Cloudflare R2:** ~$0/month (free egress)

---

## 🔮 Roadmap

### Current Migration (In Progress)

- [x] Reorganize into monorepo structure
- [x] Create backend API structure
- [x] Add admin API for config management
- [ ] Update import paths in backend
- [ ] Test new backend structure
- [ ] Remove old root files

### Near-term Optimizations

- [ ] **Remove Celery/Redis** - Replace with asyncio (simpler, lighter)
- [ ] **Direct FFmpeg piping** - Eliminate disk I/O for frames
- [ ] **Cloudflare R2 migration** - Free egress bandwidth
- [ ] **Config hot-reloading** - Live updates without restart

### Frontend Development

- [ ] Build React video player (frontend/)
- [ ] Build admin UI with stylization controls
- [ ] Real-time preview in admin
- [ ] WebSocket for live processor status
- [ ] Preset management UI

### Advanced Features

- [ ] Multiple preset slots
- [ ] A/B comparison in admin
- [ ] Time-based preset scheduling
- [ ] ML-based effects (style transfer)
- [ ] Multi-camera support

---

## 📝 Development Notes

### Adding New Effects

**1. Add processing logic:**
```python
# backend/core/image_processing.py
def apply_new_effect(frame, param1, param2):
    # OpenCV processing here
    return processed_frame
```

**2. Expose in config:**
```python
# backend/api/admin.py
class StylizationConfig(BaseModel):
    # ... existing params
    new_effect_param1: float = 1.0
    new_effect_param2: int = 10
```

**3. Add UI controls (when admin is built):**
```jsx
// admin/src/components/StyleEditor.jsx
<Slider
  label="New Effect Param"
  value={config.new_effect_param1}
  onChange={handleChange}
/>
```

### Changing Stream Source

Edit `backend/core/processor.py`:
```python
# Line 52 - Update M3U8 URL
m3u8_response = requests.get("YOUR_NEW_STREAM_URL")
```

### Performance Tuning

- **Faster processing:** Reduce `bilateral_diameter` (15 → 9)
- **More detail:** Increase `quantization_levels` (8 → 12)
- **Speed vs quality:** Adjust `process_every_nth_frame` (3 → 2 for better quality, 3 → 5 for speed)

---

## 🐛 Troubleshooting

**Celery workers not processing:**
```bash
# Check Redis is running
redis-cli ping
# Should return: PONG

# Check queues
celery -A tasks inspect active_queues
```

**FFmpeg encoding errors:**
```bash
# Verify FFmpeg is installed
ffmpeg -version

# Check frames directory has images
ls -la frames/[segment_number]/
```

**S3 upload failures:**
```bash
# Verify AWS credentials in .env
aws s3 ls s3://abbey-road

# Check IAM permissions (need PutObject, GetObject)
```

**Import errors in new backend:**
```bash
# Make sure you're in the right directory
cd backend
# And Python can find modules
export PYTHONPATH="${PYTHONPATH}:$(pwd)/.."
```

---

## 📚 Tech Stack

**Backend:**
- FastAPI 0.110.0 - Web framework
- Celery 5.3.6 - Task queue (to be removed)
- Redis 5.0.3 - Message broker (to be removed)
- PyAV 11.0.0 - Video decoding
- OpenCV 4.9.0 - Image processing
- FFmpeg - Video encoding
- Boto3 1.34.67 - AWS S3/R2 client

**Frontend (Planned):**
- React - UI framework
- HLS.js - HLS playback
- Vite - Build tool

**Infrastructure:**
- Docker + Docker Compose
- AWS S3 (migrating to Cloudflare R2)
- Uvicorn - ASGI server

---

## 📄 License

[Your license here]

## 🤝 Contributing

[Contributing guidelines here]
