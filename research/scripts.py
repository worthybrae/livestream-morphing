import av
import cv2
import numpy as np
import pytz
import time
import datetime
from concurrent.futures import ThreadPoolExecutor
import requests
import time
import os
import time
import m3u8
import requests
import shutil
from research.app import app

def get_segment_number(ts_url):
    try:
        return int(ts_url.split('_')[-1].split('.')[0])
    except ValueError:
        return float('inf')

def fetch_new_segments():
    buffer_size = 5
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
    m3u8_response = requests.get(f"https://videos-3.earthcam.com/fecnetwork/AbbeyRoadHD1.flv/chunklist_w{int(time.time())}.m3u8", headers=headers)
    playlist = m3u8.loads(m3u8_response.text)
    if playlist.segments:
        max_segment_number = max(get_segment_number(segment.uri) for segment in playlist.segments)
        min_segment_number = max(max_segment_number - buffer_size, 0)
        existing_segments = [x for x in os.listdir('segments')]
        new_segments = [i for i in range(min_segment_number, max_segment_number) if i not in existing_segments]
        return new_segments

@app.task
def fetch_new_segments_task():
    new_segments = fetch_new_segments()  # Your function to fetch new segments
    for segment in new_segments:
        download_segment.delay(segment)   

def get_segment_number(ts_url):
    try:
        return int(ts_url.split('_')[-1].split('.')[0])
    except ValueError:
        return float('inf')

@app.task
def download_segment(segment, max_retries=3, delay=2):
    try:
        os.mkdir(f"segments/{segment}")
        for attempt in range(max_retries):
            try:
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
                response = requests.get(
                    f"https://videos-3.earthcam.com/fecnetwork/AbbeyRoadHD1.flv/media_w{int(time.time())}_{segment}.ts", 
                    headers=headers
                )
                if response.status_code == 200:
                    content = response.content
                    with open(f"segments/{segment}/{segment}.ts", 'wb') as file:
                        file.write(content)
                    os.makedirs(f"frames/{segment}", exist_ok=True)
                    process_segment.delay(segment)
                    return True
                else:
                    print(f'Error downloading segment {segment}, attempt {attempt + 1} of {max_retries}')
            except Exception as e:
                print(f'Error downloading segment {segment}, attempt {attempt + 1} of {max_retries}: {e}')
            
            # Wait for a short delay before retrying
            if attempt < max_retries - 1:
                time.sleep(delay)

        # If all retries failed
        print(f'Failed to download segment {segment} after {max_retries} attempts')
        return False
    except:
        print('another worker is already downloading this segment...')
        return False
   
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

def get_colors(hour, minute):
    # Get grey level for background
    grey_level = get_grey_level(hour, minute)
    background_color = (grey_level, grey_level, grey_level)
    # Determine the most contrasting edge color
    if grey_level > 127:  # If background is light
        edge_color = (0, 0, 0)  # Use black
    else:  # If background is dark or mid-tone
        edge_color = (255, 255, 255)
    return edge_color, background_color

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

def process_frame(frame_data):
    segment_number = frame_data[0]
    frame_number = frame_data[1]
    frame = frame_data[2]
    edge_color = frame_data[3] 
    background_color = frame_data[4]  
    lth = frame_data[5]
    ltm = frame_data[6]
    frame = adjust_contrast_and_hue(frame, lth, ltm)
    blurred_array = cv2.GaussianBlur(frame, (11, 11), 0)

    # Convert to grayscale
    gray = cv2.cvtColor(blurred_array, cv2.COLOR_BGR2GRAY)

    # Edge detection
    edges = cv2.Canny(gray, 100, 200, apertureSize=5)

    # Create a background and apply edge color
    background = np.full_like(frame, background_color)
    background[edges > 0] = edge_color
    cv2.imwrite(f"frames/{segment_number}/{frame_number}.jpg", background)

@app.task
def process_segment(segment):
    start = time.time()
    london_time = datetime.datetime.now(pytz.timezone('Europe/London'))
    edge_color, background_color = get_colors(london_time.hour, london_time.minute)
    container = av.open(f"segments/{segment}/{segment}.ts")
    frame_data = [(segment, frame_number, frame.to_ndarray(format='bgr24'), edge_color, background_color, london_time.hour, london_time.minute) for frame_number, frame in enumerate(container.decode(container.streams.video[0]))]
    with ThreadPoolExecutor() as executor:
        results = list(executor.map(process_frame, frame_data))
    container.close()
    end = time.time()
    print(f"{end - start:.1f}")
    return results



