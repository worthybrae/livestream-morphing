# Livestream Morphing

A real-time video processing system that transforms the Abbey Road webcam livestream with artistic effects. Features a public viewer and admin interface for live stylization control.

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

### Option 1: Legacy Setup (Current - Uses Celery)

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

### Option 2: New Backend Structure (WIP - Simpler!)

**Note:** This is the future setup. Backend code needs import updates to work.

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

## 💰 Deployment Options

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
