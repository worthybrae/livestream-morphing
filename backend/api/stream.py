from fastapi import APIRouter
from fastapi.responses import StreamingResponse, HTMLResponse, FileResponse, Response
import os
from pathlib import Path
import httpx
import time

router = APIRouter()

# Base paths
BASE_DIR = Path(__file__).parent.parent.parent
DATA_DIR = BASE_DIR / "data"
PROCESSED_DIR = DATA_DIR / "processed"
RAW_DIR = DATA_DIR / "raw"

# Abbey Road stream headers
EARTHCAM_HEADERS = {
    'Accept': '*/*',
    'Accept-Language': 'en-US,en;q=0.9',
    'Connection': 'keep-alive',
    'Origin': 'https://www.abbeyroad.com',
    'Referer': 'https://www.abbeyroad.com/',
    'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36',
}

@router.get("/stream")
async def stream():
    """Serve the M3U8 playlist from local filesystem"""
    try:
        playlist_path = DATA_DIR / "current_playlist.m3u8"

        if not playlist_path.exists():
            empty_playlist = "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:6\n"
            return StreamingResponse(
                iter([empty_playlist]),
                media_type='application/x-mpegURL'
            )

        with open(playlist_path, 'r') as f:
            playlist_content = f.read()

        return StreamingResponse(
            iter([playlist_content]),
            media_type='application/x-mpegURL'
        )
    except Exception as e:
        print(f"Error fetching the playlist: {e}")
        import traceback
        traceback.print_exc()
        empty_playlist = "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:6\n"
        return StreamingResponse(
            iter([empty_playlist]),
            media_type='application/x-mpegURL',
            status_code=500
        )

@router.get("/segments/{segment_id}.ts")
async def get_segment(segment_id: str):
    """Serve individual video segments from local filesystem"""
    try:
        segment_path = PROCESSED_DIR / f"{segment_id}.ts"

        if not segment_path.exists():
            return Response(content=b"Segment not found", status_code=404)

        return FileResponse(
            segment_path,
            media_type='video/mp2t',
            headers={
                'Cache-Control': 'max-age=3600',
                'Access-Control-Allow-Origin': '*'
            }
        )
    except Exception as e:
        print(f"Error serving segment {segment_id}: {e}")
        return Response(content=b"Error serving segment", status_code=500)

@router.get("/raw")
async def raw_stream():
    """Serve the raw stream M3U8 playlist using locally saved segments"""
    try:
        # Import here to get the current state
        from backend.core.processor import ready_segments

        if len(ready_segments) < 3:
            empty_playlist = "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:6\n"
            return Response(
                content=empty_playlist,
                media_type='application/x-mpegURL'
            )

        # Sort segments (oldest to newest)
        sorted_segments = sorted(ready_segments)

        # Use a sliding window of segments
        num_segments = min(10, len(sorted_segments))
        playlist_segments = sorted_segments[:num_segments]

        # Build M3U8 content for raw stream
        m3u8_content = (
            f"#EXTM3U\n"
            f"#EXT-X-VERSION:3\n"
            f"#EXT-X-TARGETDURATION:6\n"
            f"#EXT-X-MEDIA-SEQUENCE:{playlist_segments[0]}\n"
        )

        for segment in playlist_segments:
            # Use local raw segment endpoint
            segment_url = f"http://localhost:8000/api/raw-segments/{segment}.ts"
            m3u8_content += f"#EXTINF:6.0,\n{segment_url}\n"

        return Response(
            content=m3u8_content,
            media_type='application/x-mpegURL',
            headers={
                'Access-Control-Allow-Origin': '*',
                'Cache-Control': 'no-cache'
            }
        )
    except Exception as e:
        print(f"Error generating raw stream playlist: {e}")
        import traceback
        traceback.print_exc()
        empty_playlist = "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:6\n"
        return Response(
            content=empty_playlist,
            media_type='application/x-mpegURL',
            status_code=500
        )

@router.get("/raw-segments/{segment_id}.ts")
async def raw_segment(segment_id: str):
    """Serve individual raw video segments from local filesystem"""
    try:
        segment_path = RAW_DIR / f"{segment_id}.ts"

        if not segment_path.exists():
            return Response(content=b"Segment not found", status_code=404)

        return FileResponse(
            segment_path,
            media_type='video/mp2t',
            headers={
                'Cache-Control': 'max-age=3600',
                'Access-Control-Allow-Origin': '*'
            }
        )
    except Exception as e:
        print(f"Error serving raw segment {segment_id}: {e}")
        return Response(content=b"Error serving segment", status_code=500)

@router.get("/", response_class=HTMLResponse)
async def main():
    content = """
    <!DOCTYPE html>
    <html>
    <head>
    <style>
        body {
        margin: 0;
        height: 100vh;
        overflow: hidden;
        }
        video {
        object-fit: cover;
        width: 100vw;
        height: 100vh;
        position: fixed;
        top: 0;
        left: 0;
        }
    </style>
    </head>
    <body>
    <video autoplay loop muted playsinline>
        <source src="/stream" type="application/x-mpegURL">
        Your browser does not support the video tag.
    </video>
    </body>
    </html>
    """
    return HTMLResponse(content=content)