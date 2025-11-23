# Quick Start - New Asyncio Backend

## ✅ What Changed

**Removed:**
- ❌ Celery (5 terminal windows)
- ❌ Redis (separate server)
- ❌ Flower (monitoring UI)
- ❌ Complex worker queues

**Replaced with:**
- ✅ Pure asyncio (built-in Python)
- ✅ In-memory state (deque instead of Redis)
- ✅ Single process
- ✅ One command to run everything!

## 🚀 How to Run (New Way)

### 1. Install Dependencies

```bash
cd backend
pip install -r requirements.txt
```

### 2. Configure Environment

Create `.env` file in project root:
```bash
# AWS S3 (current)
AWS_PUB_KEY=your_key
AWS_SECRET_KEY=your_secret
S3_BUCKET=abbey-road

# Or Cloudflare R2 (recommended - free egress!)
R2_ACCESS_KEY=your_r2_key
R2_SECRET_KEY=your_r2_secret
R2_ACCOUNT_ID=your_account_id
S3_BUCKET=your-bucket-name
```

### 3. Run!

```bash
# From project root:
python -m uvicorn backend.main:app --reload

# Or use the run script:
cd backend
./run.sh
```

That's it! No Redis, no Celery workers, just one command!

## 🎯 What Happens

When you start the server:

1. **FastAPI starts** on `http://localhost:8000`
2. **Background processor automatically starts** (via lifespan manager)
3. **Every 2 seconds**, it checks for new Abbey Road segments
4. **Downloads** → **Processes** → **Uploads to S3** → **Generates playlist**
5. **API serves the stream** at `/api/stream`

## 📡 API Endpoints

- **Stream**: `GET /api/stream` - HLS playlist
- **Config**: `GET /api/admin/config` - Current stylization settings
- **Update Config**: `POST /api/admin/config` - Change effects in real-time
- **Status**: `GET /api/admin/status` - Processor status
- **Health**: `GET /health` - Health check
- **Docs**: `GET /docs` - Interactive API documentation

## 🔍 Testing

### Check if it's running:
```bash
curl http://localhost:8000/health
# Should return: {"status":"ok"}
```

### Get processor status:
```bash
curl http://localhost:8000/api/admin/status
# Shows recent/ready segments
```

### Update stylization:
```bash
curl -X POST http://localhost:8000/api/admin/config \
  -H "Content-Type: application/json" \
  -d '{
    "quantization_levels": 12,
    "bilateral_diameter": 20
  }'
```

## 📊 Comparison

### Before (Celery):
```bash
# Terminal 1
redis-server

# Terminal 2
celery -A tasks beat --loglevel=info

# Terminal 3
celery -A tasks worker --loglevel=info --concurrency=1 -Q fns

# Terminal 4
celery -A tasks worker --loglevel=info --concurrency=2 -Q ds

# Terminal 5
celery -A tasks worker --loglevel=info --concurrency=3 -Q ps

# Terminal 6
celery -A tasks worker --loglevel=info --concurrency=2 -Q gm

# Terminal 7
uvicorn api:app --reload

# Memory usage: ~500MB
# Dependencies: 23 packages
```

### After (Asyncio):
```bash
# Just one terminal!
python -m uvicorn backend.main:app --reload

# Memory usage: ~100MB
# Dependencies: 12 packages
```

## 🐛 Troubleshooting

**Import errors:**
```bash
# Make sure you're running from project root
cd /Users/worthy/TestCode/FunCode/livestream-morphing
python -m uvicorn backend.main:app --reload
```

**Module not found:**
```bash
# Install dependencies
cd backend
pip install -r requirements.txt
```

**S3 upload errors:**
```bash
# Check your .env file has AWS credentials
cat ../.env

# Test AWS connection
aws s3 ls s3://abbey-road
```

## 💡 Next Steps

1. ✅ Backend is now Celery-free!
2. 🔜 Build admin UI for real-time stylization control
3. 🔜 Build frontend video player
4. 🔜 Deploy to Oracle Cloud free tier ($0/month)
5. 🔜 Migrate to Cloudflare R2 (free egress)

## 🎨 Try Different Effects

```bash
# Heavy blobs
curl -X POST http://localhost:8000/api/admin/config \
  -H "Content-Type: application/json" \
  -d '{"bilateral_diameter": 25, "quantization_levels": 4}'

# More detail
curl -X POST http://localhost:8000/api/admin/config \
  -H "Content-Type: application/json" \
  -d '{"bilateral_diameter": 9, "quantization_levels": 16, "edge_blend_factor": 0.4}'

# Psychedelic
curl -X POST http://localhost:8000/api/admin/config \
  -H "Content-Type: application/json" \
  -d '{"psychedelic_amplitude": 0.05, "psychedelic_frequency": 50.0}'

# Reset to defaults
curl -X POST http://localhost:8000/api/admin/config/reset
```
