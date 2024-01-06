import m3u8
import requests
import subprocess
import cv2
import numpy as np
import tempfile
import time
import asyncio
import aiohttp
import pytz
import datetime

# Function to get the daytime factor (0 for night, 1 for day)
def get_daytime_factor():
    london_tz = pytz.timezone('Europe/London')
    london_time = datetime.datetime.now(london_tz)
    normalized_time = (london_time.hour + london_time.minute / 60) / 24

    sunrise = 0.25  # 6:00 AM normalized
    sunset = 0.75   # 6:00 PM normalized

    if sunrise <= normalized_time < sunset:
        daytime_factor = (normalized_time - sunrise) / (sunset - sunrise)
    else:
        if normalized_time >= sunset:
            daytime_factor = 1 - (normalized_time - sunset) / (1 - sunset + sunrise)
        else:
            daytime_factor = 1 - (sunrise - normalized_time) / (sunrise + 1 - sunset)

    return 1 - daytime_factor

# Function to interpolate colors
def interpolate_color(color1, color2, factor, min_contrast=30):
    factor = max(min_contrast / 255, min(factor, 1 - min_contrast / 255))
    return [int(c1 + (c2 - c1) * factor) for c1, c2 in zip(color1, color2)]

# Function to generate the M3U8 URL dynamically
def generate_m3u8_url():
    epoch_time = int(time.time())
    return f"https://videos-3.earthcam.com/fecnetwork/AbbeyRoadHD1.flv/chunklist_w{epoch_time}.m3u8"

# Define headers for requests
headers = {
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

# Async function to download TS file
async def download_ts_file(session, ts_url, headers):
    print(ts_url)
    async with session.get(f"https://videos-3.earthcam.com/fecnetwork/AbbeyRoadHD1.flv/{ts_url}", headers=headers) as response:
        if response.status == 200:
            return await response.read()
        else:
            return None

# Function to process TS files with contour filtering
def process_ts_file(ts_content, frame_rate=30):
    frame_duration = 1.0 / frame_rate

    with tempfile.NamedTemporaryFile(suffix='.ts', delete=False) as temp_ts:
        temp_ts.write(ts_content)
        temp_ts_path = temp_ts.name

    process = subprocess.Popen(['ffmpeg', '-i', temp_ts_path, '-f', 'image2pipe', '-pix_fmt', 'bgr24', '-vcodec', 'rawvideo', '-'], stdout=subprocess.PIPE)

    last_frame_time = time.time()
    while True:
        raw_frame = process.stdout.read(1920 * 1080 * 3)
        if not raw_frame:
            break

        frame = np.frombuffer(raw_frame, np.uint8).reshape((1080, 1920, 3))
        gray = cv2.cvtColor(frame, cv2.COLOR_BGR2GRAY)
        edges = cv2.Canny(gray, 100, 200)

        kernel = np.ones((3, 3), np.uint8)
        edges_dilated = cv2.dilate(edges, kernel, iterations=1)
        edges_processed = cv2.erode(edges_dilated, kernel, iterations=1)

        # Contour filtering
        contours, _ = cv2.findContours(edges_processed, cv2.RETR_TREE, cv2.CHAIN_APPROX_SIMPLE)
        large_contours = [cnt for cnt in contours if cv2.contourArea(cnt) > 100]  # Adjust threshold
        large_edges = np.zeros_like(edges_processed)
        cv2.drawContours(large_edges, large_contours, -1, (255), thickness=cv2.FILLED)

        daytime_factor = get_daytime_factor()
        background_color = interpolate_color([0, 0, 0], [255, 255, 255], daytime_factor)
        edge_color = interpolate_color([255, 255, 255], [0, 0, 0], daytime_factor)

        background = np.full_like(frame, background_color)
        background[large_edges == 255] = edge_color

        cv2.imshow('Processed Frame', background)

        while time.time() - last_frame_time < frame_duration:
            time.sleep(0.001)
        last_frame_time = time.time()

        if cv2.waitKey(1) & 0xFF == ord('q'):
            break

    process.stdout.close()
    process.terminate()

# Main loop
async def main():
    last_segment_uri = None
    while True:
        await asyncio.sleep(1)  # Prefetching the next M3U8 file

        m3u8_url = generate_m3u8_url()
        m3u8_response = requests.get(m3u8_url, headers=headers)

        playlist = m3u8.loads(m3u8_response.text)

        async with aiohttp.ClientSession() as session:
            ts_futures = [download_ts_file(session, segment.uri, headers) for segment in playlist.segments if segment.uri != last_segment_uri]
            ts_contents = await asyncio.gather(*ts_futures)

        for ts_content in ts_contents:
            if ts_content:
                process_ts_file(ts_content)

        if playlist.segments:
            last_segment_uri = playlist.segments[-1].uri

asyncio.run(main())




