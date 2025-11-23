"""
Main FastAPI application entry point.
Serves both the stream API and admin API.
Starts the background stream processor.
"""
import asyncio
from contextlib import asynccontextmanager

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from backend.api import stream, admin
from backend.core.processor import stream_processor


@asynccontextmanager
async def lifespan(app: FastAPI):
    """
    Lifecycle manager for the FastAPI app.
    Starts the stream processor on startup, stops it on shutdown.
    """
    # Startup: Start the background processor
    processor_task = asyncio.create_task(stream_processor())
    print("✅ Stream processor started in background")

    yield

    # Shutdown: Cancel the processor
    processor_task.cancel()
    try:
        await processor_task
    except asyncio.CancelledError:
        print("✅ Stream processor stopped")


app = FastAPI(
    title="Livestream Morphing API",
    lifespan=lifespan
)

# CORS for frontend/admin UIs
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],  # Configure based on deployment
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Include routers
app.include_router(stream.router, prefix="/api", tags=["stream"])
app.include_router(admin.router, prefix="/api/admin", tags=["admin"])


@app.get("/health")
async def health_check():
    """Health check endpoint"""
    return {"status": "ok"}


@app.get("/")
async def root():
    """Root endpoint with API info"""
    return {
        "name": "Livestream Morphing API",
        "version": "2.0.0",
        "docs": "/docs",
        "endpoints": {
            "stream": "/api/stream",
            "admin_config": "/api/admin/config",
            "admin_status": "/api/admin/status",
            "health": "/health"
        }
    }


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
