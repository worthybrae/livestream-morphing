"""
Main stream processor - replaces Celery tasks with asyncio.
Handles fetching, downloading, processing, and uploading video segments.
"""
import asyncio
import os
import shutil
import time
from collections import deque
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime
from io import BytesIO

import av
import ffmpeg
import httpx
import m3u8
import pytz
from dotenv import load_dotenv

from backend.core.image_processing import get_colors, process_frame_fast_blobs

load_dotenv(override=True)

# In-memory state (replaces Redis)
recent_segments = deque(maxlen=10)
ready_segments = deque(maxlen=10)
processing_lock = asyncio.Lock()

# Performance tracking
processing_times = deque(maxlen=10)  # Track last 10 segment processing times
download_times = deque(maxlen=10)    # Track last 10 download times

# Stream configuration (can be updated via API)
STREAM_BASE_URL = 'https://videos-3.earthcam.com/fecnetwork/AbbeyRoadHD1.flv/chunklist_w'

# Abbey Road stream headers
EARTHCAM_HEADERS = {
    'Accept': '*/*',
    'Accept-Language': 'en-US,en;q=0.9',
    'Connection': 'keep-alive',
    'Origin': 'https://www.abbeyroad.com',
    'Referer': 'https://www.abbeyroad.com/',
    'Sec-Fetch-Dest': 'empty',
    'Sec-Fetch-Mode': 'cors',
    'Sec-Fetch-Site': 'cross-site',
    'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36',
    'sec-ch-ua': '"Google Chrome";v="119", "Chromium";v="119", "Not?A_Brand";v="24"',
    'sec-ch-ua-mobile': '?0',
    'sec-ch-ua-platform': '"macOS"'
}

# Base paths
BASE_DIR = os.path.dirname(os.path.dirname(os.path.dirname(__file__)))
DATA_DIR = os.path.join(BASE_DIR, "data")
FRAMES_DIR = os.path.join(DATA_DIR, "frames")
SEGMENTS_DIR = os.path.join(DATA_DIR, "segments")  # Temporary: for processing
RAW_DIR = os.path.join(DATA_DIR, "raw")  # Persistent: raw .ts files for playback
PROCESSED_DIR = os.path.join(DATA_DIR, "processed")  # Persistent: processed .ts files for playback

# Ensure directories exist
os.makedirs(FRAMES_DIR, exist_ok=True)
os.makedirs(SEGMENTS_DIR, exist_ok=True)
os.makedirs(RAW_DIR, exist_ok=True)
os.makedirs(PROCESSED_DIR, exist_ok=True)


async def fetch_new_segment(client: httpx.AsyncClient) -> str | None:
    """
    Fetches the latest segment ID from the configured stream.
    Returns the segment ID if it's new, None otherwise.
    """
    try:
        url = f"{STREAM_BASE_URL}{int(time.time())}.m3u8"
        response = await client.get(url, headers=EARTHCAM_HEADERS, timeout=10.0)
        response.raise_for_status()

        playlist = m3u8.loads(response.text)
        if playlist.segments:
            # Extract segment ID from URI
            segment_id = playlist.segments[0].uri.split('_')[-1].split('.')[0]

            # Check if it's new
            if segment_id not in recent_segments:
                print(f"✨ New segment found: {segment_id}")
                return segment_id
            else:
                print(f"⏭️  Segment {segment_id} already processed")
                return None
    except Exception as e:
        print(f"❌ Error fetching segment: {e}")
        return None


async def download_segment(client: httpx.AsyncClient, segment_id: str) -> bytes | None:
    """
    Downloads a video segment from the Abbey Road stream.
    Returns the raw .ts file content.
    """
    download_start = time.time()

    segment_dir = os.path.join(SEGMENTS_DIR, segment_id)
    segment_file = os.path.join(segment_dir, f"{segment_id}.ts")

    # Check if already downloaded by another process
    if os.path.exists(segment_file):
        print(f"⏭️  Segment {segment_id} already downloaded")
        with open(segment_file, 'rb') as f:
            return f.read()

    # Create directory
    os.makedirs(segment_dir, exist_ok=True)

    # Download with retries
    for attempt in range(3):
        try:
            # Generate fresh timestamp for each attempt to avoid stale URLs
            timestamp = int(time.time())
            # Derive segment URL from STREAM_BASE_URL by replacing chunklist_w with media_w
            segment_base_url = STREAM_BASE_URL.replace('/chunklist_w', '/media_w')
            url = f"{segment_base_url}{timestamp}_{segment_id}.ts"
            print(f"📥 Downloading segment {segment_id} (attempt {attempt + 1}/3, ts={timestamp})...")

            response = await client.get(url, headers=EARTHCAM_HEADERS, timeout=30.0)
            response.raise_for_status()

            content = response.content

            # Save to disk
            with open(segment_file, 'wb') as f:
                f.write(content)

            download_time = time.time() - download_start
            download_times.append(download_time)
            print(f"✅ Downloaded segment {segment_id} ({len(content) / 1024 / 1024:.2f} MB) in {download_time:.2f}s")
            return content

        except Exception as e:
            print(f"⚠️  Download attempt {attempt + 1} failed: {e}")
            if attempt < 2:
                await asyncio.sleep(0.2)
            else:
                print(f"❌ Failed to download segment {segment_id} after 3 attempts")
                return None


def process_segment_sync(segment_id: str) -> bytes | None:
    """
    Processes a video segment synchronously (CPU-bound, runs in thread pool).
    Extracts frames, applies effects, encodes back to video.
    Returns the encoded .ts video content.
    """
    process_start = time.time()

    try:
        segment_file = os.path.join(SEGMENTS_DIR, segment_id, f"{segment_id}.ts")
        frames_dir = os.path.join(FRAMES_DIR, segment_id)
        os.makedirs(frames_dir, exist_ok=True)

        # Get London time for color scheme
        london_time = datetime.now(pytz.timezone('Europe/London'))
        edge_color, background_color = get_colors(london_time.hour, london_time.minute)

        print(f"🎨 Processing segment {segment_id}...")

        # Open video container
        container = av.open(segment_file)

        # Prepare frame data for processing
        frame_data = [
            (segment_id, frame_number, frame.to_ndarray(format='bgr24'),
             edge_color, background_color,
             london_time.year, london_time.month, london_time.day,
             london_time.hour, london_time.minute)
            for frame_number, frame in enumerate(container.decode(container.streams.video[0]))
        ]

        # Process frames in parallel
        import multiprocessing
        max_workers = min(multiprocessing.cpu_count(), 8)
        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            list(executor.map(process_frame_fast_blobs, frame_data))

        container.close()

        print(f"🎬 Encoding segment {segment_id}...")

        # Check if frames were actually created
        frame_files = sorted([f for f in os.listdir(frames_dir) if f.endswith('.jpg')])
        if not frame_files:
            print(f"❌ No frames found in {frames_dir}")
            return None

        print(f"📊 Found {len(frame_files)} frame files to encode")

        # Verify frame numbering is consecutive
        frame_numbers = sorted([int(f.replace('.jpg', '')) for f in frame_files])
        print(f"🎬 Frame range: {frame_numbers[0]} to {frame_numbers[-1]}")

        # Encode frames to video using FFmpeg with explicit start number
        out, err = (
            ffmpeg
            .input(f'{frames_dir}/%d.jpg',
                   framerate=30,
                   start_number=0,
                   f='image2')
            .output('pipe:',
                    vcodec='libx264',
                    crf=25,
                    pix_fmt='yuv420p',
                    format='mpegts',
                    **{'loglevel': 'warning'})
            .run(capture_stdout=True, capture_stderr=True)
        )

        if err:
            stderr_output = err.decode('utf-8')
            print(f"⚠️  FFmpeg stderr: {stderr_output[:500]}")
            # Check for actual errors (FFmpeg writes normal output to stderr too)
            if 'error' in stderr_output.lower() or 'invalid' in stderr_output.lower():
                print(f"❌ FFmpeg encountered errors")

        process_time = time.time() - process_start
        processing_times.append(process_time)
        print(f"✅ Processed segment {segment_id} ({len(out) / 1024 / 1024:.2f} MB) in {process_time:.2f}s")
        return out

    except Exception as e:
        print(f"❌ Error processing segment {segment_id}: {e}")
        import traceback
        traceback.print_exc()
        return None


async def process_segment_async(segment_id: str) -> bytes | None:
    """
    Wrapper to run CPU-bound processing in a thread pool.
    """
    loop = asyncio.get_event_loop()
    return await loop.run_in_executor(None, process_segment_sync, segment_id)


async def generate_m3u8_playlist() -> bool:
    """
    Generates an M3U8 playlist from processed segments.
    Uses a sliding window approach to ensure smooth continuous playback.
    """
    try:
        # Need at least 3 segments minimum to start
        if len(ready_segments) < 3:
            print(f"⏭️  Only {len(ready_segments)} segments ready, need at least 3 to start playback")
            return False

        # Sort segments (oldest to newest)
        sorted_segments = sorted(ready_segments)

        # Use a sliding window of segments - always keep 10 segments in the playlist
        # This mimics how the raw stream works
        num_segments = min(10, len(sorted_segments))

        # Start from oldest and take consecutive segments
        playlist_segments = sorted_segments[:num_segments]

        if not playlist_segments:
            print("⏭️  No segments available for playlist")
            return False

        # Build M3U8 content similar to live streaming
        m3u8_content = (
            f"#EXTM3U\n"
            f"#EXT-X-VERSION:3\n"
            f"#EXT-X-TARGETDURATION:6\n"
            f"#EXT-X-MEDIA-SEQUENCE:{playlist_segments[0]}\n"
        )

        for segment in playlist_segments:
            # Use local API endpoint instead of S3
            segment_url = f"http://localhost:8000/api/segments/{segment}.ts"
            m3u8_content += f"#EXTINF:6.0,\n{segment_url}\n"

        # Save playlist locally
        playlist_path = os.path.join(DATA_DIR, "current_playlist.m3u8")
        with open(playlist_path, 'w') as f:
            f.write(m3u8_content)

        print(f"📝 Playlist: segments {playlist_segments[0]}-{playlist_segments[-1]} ({len(playlist_segments)} segments)")
        return True

    except Exception as e:
        print(f"❌ Error generating playlist: {e}")
        return False


async def cleanup_old_segments(segment_id: str):
    """
    Removes old segment files to save disk space.
    Keeps only the most recent segments.
    """
    try:
        if len(recent_segments) >= 10:
            # Remove oldest segment (deque automatically keeps only last 10)
            oldest = recent_segments[-1]

            # Remove temporary directories (frames and segments used for processing)
            for base_dir in [FRAMES_DIR, SEGMENTS_DIR]:
                old_dir = os.path.join(base_dir, oldest)
                if os.path.exists(old_dir):
                    shutil.rmtree(old_dir)

            # Remove old raw and processed segment files
            raw_file = os.path.join(RAW_DIR, f"{oldest}.ts")
            if os.path.exists(raw_file):
                os.remove(raw_file)

            processed_file = os.path.join(PROCESSED_DIR, f"{oldest}.ts")
            if os.path.exists(processed_file):
                os.remove(processed_file)

            print(f"🗑️  Cleaned up old segment: {oldest}")
    except Exception as e:
        print(f"⚠️  Error cleaning up: {e}")


async def process_pipeline(client: httpx.AsyncClient, segment_id: str):
    """
    Main processing pipeline for a single segment.
    Downloads → Processes → Saves locally → Updates playlist.
    Processes segments in parallel for speed.
    """
    try:
        # Add to recent segments
        recent_segments.appendleft(segment_id)

        # Download
        ts_content = await download_segment(client, segment_id)
        if not ts_content:
            print(f"⏭️  Skipping segment {segment_id} - download failed (likely 404, stream moved on)")
            return

        # Save raw segment for playback
        raw_file = os.path.join(RAW_DIR, f"{segment_id}.ts")
        with open(raw_file, 'wb') as f:
            f.write(ts_content)
        print(f"💾 Saved raw segment {segment_id}")

        # Process (CPU-bound, runs in thread pool)
        processed_content = await process_segment_async(segment_id)
        if not processed_content:
            return

        # Save processed segment for playback
        processed_file = os.path.join(PROCESSED_DIR, f"{segment_id}.ts")
        with open(processed_file, 'wb') as f:
            f.write(processed_content)
        print(f"💾 Saved processed segment {segment_id}")

        # Add to ready segments (avoid duplicates)
        segment_int = int(segment_id)
        if segment_int not in ready_segments:
            ready_segments.appendleft(segment_int)
            print(f"✅ Added segment {segment_id} to ready queue (total: {len(ready_segments)})")
        else:
            print(f"⏭️  Segment {segment_id} already in ready queue")

        # Generate playlist
        await generate_m3u8_playlist()

        # Cleanup old files
        await cleanup_old_segments(segment_id)

        print(f"🎉 Completed segment {segment_id}\n")

    except Exception as e:
        print(f"❌ Pipeline error for segment {segment_id}: {e}")
        import traceback
        traceback.print_exc()


def cleanup_all_data():
    """
    Clears all frames, segments, raw, and processed files on startup.
    """
    print("🧹 Cleaning up old data on startup...")

    for directory in [FRAMES_DIR, SEGMENTS_DIR, RAW_DIR, PROCESSED_DIR]:
        if os.path.exists(directory):
            # Remove all subdirectories and files
            for item in os.listdir(directory):
                item_path = os.path.join(directory, item)
                try:
                    if os.path.isfile(item_path):
                        os.remove(item_path)
                    elif os.path.isdir(item_path):
                        shutil.rmtree(item_path)
                except Exception as e:
                    print(f"⚠️  Failed to delete {item_path}: {e}")

    # Clear the deques
    recent_segments.clear()
    ready_segments.clear()

    print("✅ Cleanup complete!\n")


async def stream_processor():
    """
    Main background loop that continuously processes the stream.
    Runs forever, checking for new segments every 2 seconds.
    """
    # Clean up old data on startup
    cleanup_all_data()

    print("🚀 Stream processor started!")
    print(f"📁 Data directory: {DATA_DIR}")
    print(f"🎬 Frames: {FRAMES_DIR}")
    print(f"📹 Segments: {SEGMENTS_DIR}\n")

    async with httpx.AsyncClient() as client:
        while True:
            try:
                # Fetch new segment
                segment_id = await fetch_new_segment(client)

                if segment_id:
                    # Process in background (don't block polling)
                    asyncio.create_task(process_pipeline(client, segment_id))

                # Wait 2 seconds before next check
                await asyncio.sleep(2.0)

            except Exception as e:
                print(f"❌ Error in main loop: {e}")
                await asyncio.sleep(5.0)  # Wait longer on error


def get_processor_status():
    """
    Returns current processor status for admin API.
    """
    avg_processing_time = sum(processing_times) / len(processing_times) if processing_times else 0
    avg_download_time = sum(download_times) / len(download_times) if download_times else 0

    return {
        "running": True,
        "recent_segments": list(recent_segments)[:5],
        "ready_segments": list(ready_segments)[:5],
        "total_processed": len(recent_segments),
        "total_ready": len(ready_segments),
        "avg_processing_time": round(avg_processing_time, 2),
        "avg_download_time": round(avg_download_time, 2),
        "avg_total_time": round(avg_processing_time + avg_download_time, 2)
    }
