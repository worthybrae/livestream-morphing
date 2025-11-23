#!/bin/bash

# Quick start script for the new backend
# No Celery, no Redis - just one command!

echo "🚀 Starting Livestream Morphing Backend..."
echo ""

# Check if we're in the backend directory
if [[ $(basename $(pwd)) == "backend" ]]; then
    echo "📁 Detected backend directory, moving to project root..."
    cd ..
fi

echo "📦 Installing dependencies..."
pip install -r backend/requirements.txt

echo ""
echo "✅ Starting server on http://localhost:8000"
echo "📝 API docs: http://localhost:8000/docs"
echo "💚 Health check: http://localhost:8000/health"
echo "🎬 Stream: http://localhost:8000/api/stream"
echo "⚙️  Admin API: http://localhost:8000/api/admin/config"
echo ""
echo "Press CTRL+C to stop"
echo ""

# Run from project root so imports work
python -m uvicorn backend.main:app --reload --host 0.0.0.0 --port 8000
