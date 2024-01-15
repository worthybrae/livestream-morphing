from concurrent.futures import ThreadPoolExecutor
from celery.schedules import timedelta
from celery import Celery
import numpy as np
import lz4.frame
import datetime
import requests
import pickle
import shutil
import base64
import m3u8
import time
import pytz
import json
import cv2
import os
import av


app = Celery('test_app',
             broker='redis://localhost:6379/0',
             backend='redis://localhost:6379/0')

app.conf.update(
    task_serializer='pickle',
    accept_content=['pickle'],
    result_serializer='pickle',
)

app.conf.beat_schedule = {
    'fetch-new-segments-every-2-seconds': {
        'task': 'app.fetch_new_segments_task',
        'schedule': timedelta(seconds=2),
    },
}

app.conf.task_routes = {
    'app.fetch_new_segments_task': {'queue': 'new_segments_queue'},
    'app.download_segment': {'queue': 'download_queue'},
    'app.process_segment': {'queue': 'process_segment_queue'},
    'app.process_frame': {'queue': 'process_frame_queue'},
}

@app.task
def fetch_new_segments_task():
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
        max_segment_number = max(int(segment.uri.split('_')[-1].split('.')[0]) for segment in playlist.segments)
        min_segment_number = max(max_segment_number - buffer_size, 0)
        existing_segments = [int(x) for x in os.listdir('segments') if x != '.DS_Store']
        existing_segments.sort(reverse=True)
        existing_fsegments = [int(x) for x in os.listdir('frames') if x != '.DS_Store']
        existing_fsegments.sort(reverse=True)
        new_segments = [i for i in range(min_segment_number, max_segment_number) if i not in existing_segments]
        for ns in new_segments:
            download_segment.delay(ns)
        for inc in existing_segments[5:]:
            shutil.rmtree(f"segments/{inc}")
        for inc in existing_fsegments[5:]:
            shutil.rmtree(f"frames/{inc}")
        return new_segments
    
@app.task
def download_segment(segment, max_retries=3, delay=.5):
    try:
        os.mkdir(f"segments/{segment}")
        flag = True
    except:
        flag = False
    if flag == True:
        for attempt in range(3):
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
    else:
        print('another worker is already downloading this segment...')
        return False
    
@app.task
def process_segment(segment):
    london_time = datetime.datetime.now(pytz.timezone('Europe/London'))
    edge_color, background_color = get_colors(london_time.hour, london_time.minute)
    container = av.open(f"segments/{segment}/{segment}.ts")
    for frame_number, frame in enumerate(container.decode(container.streams.video[0])):
        f = frame.to_ndarray(format='gray').tolist()

        # Manually serializing with Pickle
        serialized_frame = pickle.dumps(f)

        print(len(f), f[:5])
        process_frame.delay((int(segment), int(frame_number), serialized_frame, edge_color, background_color, london_time.year, london_time.month, london_time.day, london_time.hour, london_time.minute))
    container.close()

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

def adjust_contrast(frame, hour, minute):
    """
    Adjusts the contrast of the frame based on the time of day.
    The adjustment is linearly scaled: minimal around noon and maximal around midnight.
    """
    total_minutes = hour * 60 + minute
    if total_minutes > 720:  # Normalize for 24-hour cycle
        total_minutes = 1440 - total_minutes

    # Scale factor for contrast
    contrast_scale = total_minutes / 720.0  # Ranges from 0 (noon) to 1 (midnight)

    # Base value for contrast adjustment
    base_contrast = 50  # Maximal contrast adjustment

    # Calculate actual contrast adjustment
    contrast_adjustment = int(base_contrast * contrast_scale)

    # Adjust contrast
    f = 259 * (contrast_adjustment + 255) / (255 * (259 - contrast_adjustment))
    adjusted = cv2.convertScaleAbs(frame, alpha=f, beta=-128*f + 128)

    return adjusted


@app.task
def process_frame(frame_data):
    # Unpack frame data
    try:
        segment_number = frame_data[0]
        frame_number = frame_data[1]
        frame = frame_data[2]
        edge_color = frame_data[3]
        background_color = frame_data[4]
        lty = frame_data[5]
        ltmnth = frame_data[6]
        ltd = frame_data[7]
        lth = frame_data[8]
        ltm = frame_data[9]
        f = pickle.loads(frame)
        print(len(f), f[:5])
        frame = np.array(f)

        adjusted = adjust_contrast(frame, lth, ltm)

        # Apply Gaussian blur
        blurred = cv2.GaussianBlur(adjusted, (15, 15), 0)

        # Edge detection
        edges = cv2.Canny(blurred, 300, 400, apertureSize=5)

        # Optional: Smooth edges
        kernel = np.ones((2, 2), np.uint8)  # Adjust kernel size as needed
        smoothed_edges = cv2.morphologyEx(edges, cv2.MORPH_CLOSE, kernel)

        # Create a background and apply edge color
        background = np.full_like(frame, background_color)
        background[smoothed_edges > 0] = edge_color

        original_height, original_width = frame.shape[:2]

        aspect_ratio = original_width / original_height

        # Reduce frame size by 10%
        scale_factor = 0.90
        new_width = int(original_width * scale_factor)
        new_height = int(original_height * scale_factor)
        resized_frame = cv2.resize(background, (new_width, new_height), interpolation=cv2.INTER_AREA)

        # Calculate border sizes
        border_top = border_bottom = (original_height - new_height) // 2
        border_left = border_right = (original_width - new_width) // 2

        # Additional space for text
        text_space = 60  # Pixels
        border_bottom += text_space

        # Adjust left and right borders to maintain aspect ratio
        additional_width = text_space / 2 * aspect_ratio
        border_left += int(additional_width / 2)
        border_right += int(additional_width / 2)

        # Add border
        frame_with_border = cv2.copyMakeBorder(resized_frame, border_top, border_bottom, border_left, border_right, cv2.BORDER_CONSTANT, value=background_color)

        # Add text
        font = cv2.FONT_HERSHEY_SIMPLEX
        font_scale = 1
        line_type = 2

        # Current time
        current_time = datetime.datetime(year=lty, month=ltmnth, day=ltd, hour=lth, minute=ltm).strftime("%I:%M %p")
        (text_width, text_height), _ = cv2.getTextSize(current_time, font, font_scale, line_type)

        cv2.putText(frame_with_border, 'Abbey Road', (border_left, frame_with_border.shape[0] - int(text_space / 2) - int(text_height / 2)), font, font_scale, edge_color, line_type)
        cv2.putText(frame_with_border, current_time, (frame_with_border.shape[1] - text_width - border_right, frame_with_border.shape[0] - int(text_space / 2) - int(text_height / 2)), font, font_scale, edge_color, line_type)

        # Save the processed frame
        cv2.imwrite(f"frames/{segment_number}/{frame_number}.jpg", frame_with_border)
        return True
    except Exception as e:
        return e

