.PHONY: help install run dev clean test deps check status config screenshot screenshot-auto admin-dev

# Default target
help:
	@echo "🎨 Livestream Morphing - Available Commands"
	@echo ""
	@echo "Setup:"
	@echo "  make install          Install all dependencies"
	@echo "  make deps             Install only backend dependencies"
	@echo ""
	@echo "Running:"
	@echo "  make run              Start the backend server"
	@echo "  make dev              Start with auto-reload (development)"
	@echo "  make dev URL='...'    Start with custom stream URL"
	@echo "  make build-cpp        Build C++ extension (auto-run with run/dev)"
	@echo ""
	@echo "Example:"
	@echo "  make dev URL='https://videos-3.earthcam.com/fecnetwork/hdtimes10.flv/chunklist_w'"
	@echo ""
	@echo "Development:"
	@echo "  make admin-dev        Start admin app dev server"
	@echo "  make screenshot       Interactive browser screenshot tool"
	@echo "  make screenshot-auto  Auto-capture screenshots"
	@echo ""
	@echo "Testing:"
	@echo "  make check            Check if server is running"
	@echo "  make status           Get processor status"
	@echo "  make config           Get current stylization config"
	@echo ""
	@echo "Cleanup:"
	@echo "  make clean            Remove generated files and cache"
	@echo "  make clean-data       Remove processed frames and segments"
	@echo ""
	@echo "Examples:"
	@echo "  make install && make dev"

# Install all dependencies with Poetry
install:
	@echo "📦 Installing dependencies with Poetry..."
	@command -v poetry >/dev/null 2>&1 || { echo "❌ Poetry not found. Install it with: curl -sSL https://install.python-poetry.org | python3 -"; exit 1; }
	poetry install
	@echo "✅ Installation complete!"

# Install only backend dependencies (alias)
deps: install

# Install Poetry if not present
install-poetry:
	@echo "📦 Installing Poetry..."
	curl -sSL https://install.python-poetry.org | python3 -
	@echo "✅ Poetry installed! Run: make install"

# Build C++ extension for fast processing
build-cpp:
	@echo "🔨 Building C++ extension for fast image processing..."
	@if command -v pkg-config >/dev/null 2>&1 && (pkg-config --exists opencv4 || pkg-config --exists opencv); then \
		poetry run python -c "import pybind11" 2>/dev/null || poetry run pip install pybind11; \
		rm -rf build dist *.egg-info fast_processor*.so; \
		poetry run python setup.py build_ext --inplace && echo "✅ C++ extension built successfully!" || echo "⚠️  C++ build failed, will use Python fallback"; \
	else \
		echo "⚠️  OpenCV not found. Install with: brew install opencv"; \
		echo "⚠️  Will use Python fallback (slower)"; \
	fi

# Run the server (production mode)
run: build-cpp
	@echo "🚀 Starting backend server..."
	@if [ -n "$(URL)" ]; then \
		echo "📹 Setting stream URL: $(URL)"; \
		poetry run python -m uvicorn backend.main:app --host 0.0.0.0 --port 8000 & \
		sleep 3 && \
		curl -X POST http://localhost:8000/api/admin/stream-url \
			-H "Content-Type: application/json" \
			-d '{"url": "$(URL)"}' && \
		wait; \
	else \
		poetry run python -m uvicorn backend.main:app --host 0.0.0.0 --port 8000; \
	fi

# Run with auto-reload (development mode)
dev: build-cpp
	@echo "🚀 Starting backend in development mode..."
	@echo "📝 API docs: http://localhost:8000/docs"
	@echo "💚 Health: http://localhost:8000/health"
	@echo "🎬 Stream: http://localhost:8000/api/stream"
	@echo "⚙️  Admin: http://localhost:8000/api/admin/config"
	@echo ""
	@if [ -n "$(URL)" ]; then \
		echo "📹 Setting stream URL: $(URL)"; \
		poetry run python -m uvicorn backend.main:app --reload --host 0.0.0.0 --port 8000 & \
		sleep 3 && \
		curl -X POST http://localhost:8000/api/admin/stream-url \
			-H "Content-Type: application/json" \
			-d '{"url": "$(URL)"}' && \
		fg; \
	else \
		poetry run python -m uvicorn backend.main:app --reload --host 0.0.0.0 --port 8000; \
	fi

# Check if server is running
check:
	@echo "🔍 Checking server health..."
	@curl -s http://localhost:8000/health | python -m json.tool || echo "❌ Server not running"

# Get processor status
status:
	@echo "📊 Processor Status:"
	@curl -s http://localhost:8000/api/admin/status | python -m json.tool || echo "❌ Server not running"

# Get current config
config:
	@echo "⚙️  Current Configuration:"
	@curl -s http://localhost:8000/api/admin/config | python -m json.tool || echo "❌ Server not running"

# Update config (example)
config-blobs:
	@echo "🎨 Setting heavy blob effect..."
	@curl -X POST http://localhost:8000/api/admin/config \
		-H "Content-Type: application/json" \
		-d '{"bilateral_diameter": 25, "gaussian_blur_size": 15, "quantization_levels": 4, "edge_blend_factor": 0.0}' \
		| python -m json.tool

config-detail:
	@echo "🎨 Setting detailed posterization..."
	@curl -X POST http://localhost:8000/api/admin/config \
		-H "Content-Type: application/json" \
		-d '{"bilateral_diameter": 9, "gaussian_blur_size": 5, "quantization_levels": 16, "edge_blend_factor": 0.4}' \
		| python -m json.tool

config-psychedelic:
	@echo "🎨 Setting psychedelic effect..."
	@curl -X POST http://localhost:8000/api/admin/config \
		-H "Content-Type: application/json" \
		-d '{"psychedelic_amplitude": 0.05, "psychedelic_frequency": 50.0}' \
		| python -m json.tool

config-reset:
	@echo "🔄 Resetting to default config..."
	@curl -X POST http://localhost:8000/api/admin/config/reset | python -m json.tool

# Clean Python cache and generated files
clean:
	@echo "🧹 Cleaning up..."
	find . -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
	find . -type f -name "*.pyc" -delete 2>/dev/null || true
	find . -type f -name "*.pyo" -delete 2>/dev/null || true
	find . -type d -name "*.egg-info" -exec rm -rf {} + 2>/dev/null || true
	rm -f celerybeat-schedule.db 2>/dev/null || true
	rm -rf build dist fast_processor*.so 2>/dev/null || true
	@echo "✅ Cleanup complete!"

# Clean processed data (frames, segments)
clean-data:
	@echo "🧹 Cleaning processed data..."
	rm -rf data/frames/* 2>/dev/null || true
	rm -rf data/segments/* 2>/dev/null || true
	rm -rf frames/* 2>/dev/null || true
	rm -rf segments/* 2>/dev/null || true
	@echo "✅ Data cleanup complete!"

# Test imports
test-imports:
	@echo "🧪 Testing imports..."
	@poetry run python -c "from backend.core import processor; print('✅ Processor imports OK')"
	@poetry run python -c "from backend.api import stream, admin; print('✅ API imports OK')"
	@poetry run python -c "from backend.utils import s3; print('✅ Utils imports OK')"
	@echo "✅ All imports successful!"

# Show logs (if running)
logs:
	@echo "📋 Recent logs (if available)..."
	@tail -n 50 logs/*.log 2>/dev/null || echo "No log files found"

# Quick setup for first time users
setup: install
	@echo ""
	@echo "✅ Setup complete!"
	@echo ""
	@echo "Next steps:"
	@echo "  1. Create .env file with your AWS/R2 credentials"
	@echo "  2. Run: make dev"
	@echo "  3. Open: http://localhost:8000/docs"
	@echo ""

# Full clean (including venv - use with caution!)
clean-all: clean clean-data
	@echo "⚠️  Removing virtual environment..."
	rm -rf venv/ 2>/dev/null || true
	@echo "✅ Full cleanup complete!"

# Admin app development server
admin-dev:
	@echo "🎨 Starting admin app dev server..."
	@echo "📱 Admin UI: http://localhost:5173"
	@echo ""
	@cd admin-app && npm run dev

# Interactive browser screenshot tool
screenshot:
	@echo "📸 Starting interactive screenshot tool..."
	@echo "Make sure admin-dev is running on http://localhost:5173"
	@echo ""
	poetry run python screenshot_browser.py --no-headless

# Automated screenshot capture
screenshot-auto:
	@echo "📸 Capturing screenshots automatically..."
	@echo "Make sure admin-dev is running on http://localhost:5173"
	@echo ""
	poetry run python screenshot_browser.py --auto --headless

# Install Playwright browsers (needed after adding playwright dependency)
install-playwright:
	@echo "📦 Installing Playwright browsers..."
	poetry run playwright install
	@echo "✅ Playwright browsers installed!"
