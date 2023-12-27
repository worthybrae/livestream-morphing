import aiohttp
import asyncio
import requests
import m3u8
import time
import cv2
import numpy as np
import pytz
import datetime
import os
import threading
import csv
import shutil
import aiofiles
from fastapi import FastAPI, Request
from fastapi.responses import StreamingResponse, HTMLResponse
from fastapi.templating import Jinja2Templates
import os


def get_grey_level(hour, minute):
    """ Interpolates grey level based on hour and minute. 
        Noon (12:00) is lightest grey, and midnight (00:00) is darkest grey.
    """
    total_minutes = hour * 60 + minute
    if total_minutes > 720:  # Normalize for 24-hour cycle
        total_minutes = 1440 - total_minutes

    # Linear interpolation between lightest and darkest grey
    lightest_grey = 175
    darkest_grey = 25
    grey_level = int(darkest_grey + (lightest_grey - darkest_grey) * (total_minutes / 720.0))
    return grey_level

def adjust_contrast_and_hue(frame, hour, minute):
    """
    Adjusts the contrast and hue of the frame based on the time of day.
    The adjustments are linearly scaled: minimal around noon and maximal around midnight.
    """
    total_minutes = hour * 60 + minute
    if total_minutes > 720:  # Normalize for 24-hour cycle
        total_minutes = 1440 - total_minutes

    # Scale factors for contrast and hue
    contrast_scale = total_minutes / 720.0  # Ranges from 0 (noon) to 1 (midnight)
    hue_scale = total_minutes / 720.0  # Same range

    # Base values for contrast and hue adjustments
    base_contrast = 50  # Maximal contrast adjustment
    base_hue = 10  # Maximal hue shift

    # Calculate actual contrast and hue adjustments
    contrast_adjustment = int(base_contrast * contrast_scale)
    hue_adjustment = int(base_hue * hue_scale)

    # Adjust contrast
    f = 259 * (contrast_adjustment + 255) / (255 * (259 - contrast_adjustment))
    adjusted = cv2.convertScaleAbs(frame, alpha=f, beta=-128*f + 128)

    # Convert to HSV and shift hue
    hsv = cv2.cvtColor(adjusted, cv2.COLOR_BGR2HSV)
    hsv[..., 0] = (hsv[..., 0] + hue_adjustment) % 180  # Adding hue shift
    adjusted = cv2.cvtColor(hsv, cv2.COLOR_HSV2BGR)

    return adjusted
       
async def write_to_csv_async(file_path, data):
    async with aiofiles.open(file_path, 'w', newline='') as csvfile:
        writer = csv.DictWriter(csvfile, fieldnames=data.keys())
        await writer.writeheader()
        await writer.writerow(data)

def generate_m3u8_url(epoch_time):
    return f"https://videos-3.earthcam.com/fecnetwork/AbbeyRoadHD1.flv/chunklist_w{epoch_time}.m3u8"

def process_frame(frame, frame_number, segment_number, background_color, edge_color, london_time):
    timings = {}

    start_ach = time.time()
    frame = adjust_contrast_and_hue(frame, london_time.hour, london_time.minute)
    timings['adjust_contrast'] = time.time() - start_ach

    # Apply Gaussian blur to reduce noise
    start_blur = time.time()
    blurred_frame = cv2.GaussianBlur(frame, (11, 11), 0)
    timings['gaussian_blur'] = time.time() - start_blur

    # Edge detection with higher thresholds
    start_gray = time.time()
    gray = cv2.cvtColor(blurred_frame, cv2.COLOR_BGR2GRAY)
    timings['gray'] = time.time() - start_gray

    start_edges = time.time()
    edges = cv2.Canny(gray, 400, 400, apertureSize=5)
    timings['canny_edge'] = time.time() - start_edges

    start_bck = time.time()
    background = np.full_like(frame, background_color)
    timings['create_background'] = time.time() - start_bck   

    start_ceg = time.time()
    background[edges > 0] = edge_color
    timings['color_edges'] = time.time() - start_ceg
    
    timings['total'] = timings['adjust_contrast'] + timings['gaussian_blur'] + timings['gray'] + timings['canny_edge'] + timings['create_background'] + timings['color_edges']
    background_image_filename = f"frames/{segment_number}/{frame_number}.jpg"
    cv2.imwrite(background_image_filename, background)

def process_segment(segment_number):
    cap = cv2.VideoCapture(f"segments/{segment_number}.ts")
    frame_number = 0

    london_tz = pytz.timezone('Europe/London')
    london_time = datetime.datetime.now(london_tz)

    # Get grey level for background
    grey_level = get_grey_level(london_time.hour, london_time.minute)
    background_color = (grey_level, grey_level, grey_level)

    # Determine the most contrasting edge color
    if grey_level > 127:  # If background is light
        edge_color = (0, 0, 0)  # Use black
    else:  # If background is dark or mid-tone
        edge_color = (255, 255, 255)
    while cap.isOpened():
        ret, frame = cap.read()
        if not ret:
            break
        process_frame(frame, frame_number, segment_number, background_color, edge_color, london_time)

        frame_number += 1

    # Release the video capture object and close OpenCV window
    cap.release()

async def download_segment(session, ts_url, headers, max_retries=3, delay=.1):
    segment_number = get_segment_number(ts_url)

    for attempt in range(max_retries):
        try:
            async with session.get(f"https://videos-3.earthcam.com/fecnetwork/AbbeyRoadHD1.flv/{ts_url}", headers=headers) as response:
                if response.status == 200:
                    content = await response.read()
                    file_name = f"segments/{segment_number}.ts"
                    with open(file_name, 'wb') as file:
                        file.write(content)
                    os.makedirs(f"frames/{segment_number}", exist_ok=True)
                    return True
                else:
                    print(f'Error downloading segment {segment_number}, attempt {attempt + 1} of {max_retries}')
        except aiohttp.ClientError as e:
            print(f'Error downloading segment {segment_number}, attempt {attempt + 1} of {max_retries}: {e}')
        
        # Wait for a short delay before retrying
        if attempt < max_retries - 1:
            await asyncio.sleep(delay)

    # If all retries failed
    print(f'Failed to download segment {segment_number} after {max_retries} attempts')
    return False

def get_segment_number(ts_url):
    try:
        return int(ts_url.split('_')[-1].split('.')[0])
    except ValueError:
        return float('inf')

def process_segment_wrapper(segment_number):
    # Wrapper function to run process_segment in a thread
    process_segment(segment_number)

async def process_stream(headers, max_segments=10):
    async with aiohttp.ClientSession() as session:
        while True:

            for s in existing_segments[7:]:
                try:
                    os.remove(f"segments/{s}.ts")
                except:
                    pass
            for f in existing_frames[7:]:
                try:
                    shutil.rmtree(f"frames/{f}")
                except:
                    pass
            if playlist.segments:
                
                min_segment_number = max(max_segment_number - max_segments, 0)

                for seg_number in range(min_segment_number, max_segment_number + 1):
                    if seg_number in existing_segments:
                        pass
                    elif seg_number < min_segment_number:
                        pass
                    else:
                        print(f"New Segment Found: {seg_number}")
                        seg_uri = f"media_w{epoch_time}_{seg_number}.ts"
                        download_successful = await download_segment(session, seg_uri, headers)
                        if download_successful:
                            print(f'processing segment {seg_number}')
                            threading.Thread(target=process_segment_wrapper, args=(seg_number,)).start()

            await asyncio.sleep(.1)

def run_asyncio_loop():
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
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    loop.run_until_complete(process_stream(headers))
    loop.close()

class FrameFetcher:
    def __init__(self):
        self.current_frame = 0
        self.current_segment = 0
        self.current_frame_index = 0

    def clear_cache(self):
        try:
            for s in os.listdir('frames'):
                try:
                    shutil.rmtree(f"frames/{s}")
                    os.remove(f"segments/{s}.ts")
                except Exception as e:
                    print(f"Error: {e}")
        except Exception as e:
            print(f"Error: {e}")

        # Get all ordered segments
        existing_segments = [int(x.split('.')[0]) for x in os.listdir('segments')]
        existing_segments.sort(reverse=True)
        # Get all ordered existing frames
        existing_frames = [int(x) for x in os.listdir('frames')]
        existing_frames.sort(reverse=True)
        

    def _get_segments(self):
        # Get and sort segment directories
        segments = [d for d in os.listdir(self.frames_dir) if os.path.isdir(os.path.join(self.frames_dir, d))]
        segments.sort(key=int)
        return segments
    
    def _set_current_segment(self, headers):
        epoch_time = int(time.time())
        m3u8_response = requests.get(
            f"https://videos-3.earthcam.com/fecnetwork/AbbeyRoadHD1.flv/chunklist_w{epoch_time}.m3u8", 
            headers=headers
        )
        playlist = m3u8.loads(m3u8_response.text)

        if playlist.segments:
            max_segment_number = max(get_segment_number(segment.uri) for segment in playlist.segments)
            return max_segment_number
        else:
            print("incorrect playlist url used. is your epoch_timestamp in the wrong format?")
            return None

    def _set_next_segment(self, segments):
        if len(segments) < 5:
            return False

        # Set the fifth oldest segment
        self.current_segment += 1
        self.current_frame_index = 0
        return True
    
    def _check_if_exists(self):
        segment_path = os.path.join(self.frames_dir, self.current_segment, self.current_frame_index)
        frame_files = [f for f in os.listdir(segment_path) if f.endswith('.jpg')]
        frame_files.sort(key=lambda f: int(f.split('.')[0]))

    def get_next_frame(self):
        segments = self._get_segments()
        if self.current_segment is None or self.current_segment not in segments:
            if not self._set_next_segment(segments):
                return None

        segment_path = os.path.join(self.frames_dir, self.current_segment)
        frame_files = [f for f in os.listdir(segment_path) if f.endswith('.jpg')]
        frame_files.sort(key=lambda f: int(f.split('.')[0]))

        if self.current_frame_index >= len(frame_files):
            # Move to the next segment
            current_index = segments.index(self.current_segment)
            if current_index + 1 < len(segments):
                self.current_segment = segments[current_index + 1]
                self.current_frame_index = 0
                # Return the first frame of the new segment
                segment_path = os.path.join(self.frames_dir, self.current_segment)
                frame_files = [f for f in os.listdir(segment_path) if f.endswith('.jpg')]
                frame_files.sort(key=lambda f: int(f.split('.')[0]))

        frame_path = os.path.join(segment_path, frame_files[self.current_frame_index])
        self.current_frame_index += 1
        return frame_path

frame_fetcher = FrameFetcher()

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

app = FastAPI()
templates = Jinja2Templates(directory="templates")

@app.get("/stream")
def stream_frames():
    frame_interval = 1.0 / 30  # Approximately 33.33 milliseconds for 30 FPS
    frame_fetcher._set_current_segment(headers=)
    def generate():
        while True:
            start_time = time.time()


            frame_path = frame_fetcher.get_next_frame()
            print(frame_path)
            if frame_path and os.path.exists(frame_path):
                with open(frame_path, "rb") as f:
                    yield (b'--frame\r\n'
                           b'Content-Type: image/jpeg\r\n\r\n' + f.read() + b'\r\n')

            # elapsed_time = time.time() - start_time
            # wait_time = max(frame_interval - elapsed_time, 0) ajust to -> max(frame_interval - elapsed_time, 0) - tt_get_next_frame()
            
            # time.sleep(wait_time) adjust so that we wait the remainder of seconds...
            # between the start time and how long it takes to run tt_get_next_frame()

    return StreamingResponse(generate(), media_type="multipart/x-mixed-replace; boundary=frame")

@app.get("/", response_class=HTMLResponse)
async def read_root(request: Request):
    london_time = datetime.datetime.now(pytz.timezone('Europe/London'))
    current_time = london_time.strftime("%I:%M %p")  # Format time as HH:MM AM/PM
    return templates.TemplateResponse("main.html", {"request": request, "current_time": current_time})

if __name__ == "__main__":
    # The code below serves as a purge of frames / segments from the previous run
    for s in os.listdir('frames'):
        try:
            shutil.rmtree(f"frames/{s}")
            os.remove(f"segments/{s}.ts")
        except:
            pass
    asyncio_thread = threading.Thread(target=run_asyncio_loop)
    asyncio_thread.start()

    # Run FastAPI server with Uvicorn
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)