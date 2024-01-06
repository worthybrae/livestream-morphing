import aiohttp
import asyncio
import requests
import m3u8
import time
import subprocess
import cv2
import numpy as np
import pytz
import datetime
import os
import threading
import csv
import shutil
import aiofiles

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
    edges = cv2.Canny(gray, 100, 200, apertureSize=5)
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

async def download_segment(session, ts_url, headers, max_retries=3, delay=2):
    segment_number = get_segment_number(ts_url)
    file_name = f"segments/{segment_number}.ts"

    for attempt in range(max_retries):
        try:
            async with session.get(f"https://videos-3.earthcam.com/fecnetwork/AbbeyRoadHD1.flv/{ts_url}", headers=headers) as response:
                if response.status == 200:
                    content = await response.read()
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
            epoch_time = int(time.time())
            m3u8_url = generate_m3u8_url(epoch_time)
            m3u8_response = requests.get(m3u8_url, headers=headers)
            playlist = m3u8.loads(m3u8_response.text)
            existing_segments = [int(x.split('.')[0]) for x in os.listdir('segments')]
            existing_segments.sort(reverse=True)
            for s in existing_segments[11:]:
                os.remove(f"segments/{s}.ts")
                shutil.rmtree(f"frames/{s}")
            if playlist.segments:
                max_segment_number = max(get_segment_number(segment.uri) for segment in playlist.segments)
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



def get_sorted_frame_paths(segment_dir):
    """Retrieve frame file paths in sorted order based on integer file names."""
    frame_files = os.listdir(segment_dir)
    frame_files.sort(key=lambda f: int(f.split('.')[0]))
    return [os.path.join(segment_dir, f) for f in frame_files if f.endswith('.jpg')]

def play_frames(fps=30):
    """Play the frames in the 'frames' directory as a video."""
    time.sleep(30)
    wait_time = int(1000 / fps)  # milliseconds between frames
    last_segment = 0

    while True:
        segment_dirs = [d for d in os.listdir('frames') if os.path.isdir(os.path.join('frames', d))]
        segment_dirs.sort(key=int)
        
        for segment_dir in segment_dirs:
            segment_number = int(segment_dir)

            # Only process new segments
            if segment_number > last_segment:
                frame_paths = get_sorted_frame_paths(os.path.join('frames', segment_dir))

                for frame_path in frame_paths:
                    frame = cv2.imread(frame_path)
                    if frame is not None:
                        cv2.imshow('Frame', frame)
                        if cv2.waitKey(wait_time) & 0xFF == ord('q'):
                            return  # Exit if 'q' is pressed

                last_segment = segment_number


def run_asyncio_forever(loop):
    """Run the asyncio event loop forever."""
    asyncio.set_event_loop(loop)
    loop.run_forever()

async def main_async(headers):
    """Main asynchronous function."""
    await process_stream(headers)


async def main():
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
    try:
        for segment in os.listdir('segments'):
            os.remove(f'segments/{segment}')
        for frame in os.listdir:
            shutil.rmtree(f"frames/{frame}")
    except:
        pass
    loop = asyncio.new_event_loop()
    asyncio_thread = threading.Thread(target=run_asyncio_forever, args=(loop,))
    asyncio_thread.start()

    # Run the asynchronous stream processing in the background
    asyncio.run_coroutine_threadsafe(main_async(headers), loop)

    # Run play_frames on the main thread
    play_frames()

    # Wait for the asyncio thread to finish (if needed)
    asyncio_thread.join()

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
        

if __name__ == "__main__":
    asyncio.run(main())
