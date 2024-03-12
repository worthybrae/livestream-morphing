from celery import Celery
import numpy as np
from concurrent.futures import ThreadPoolExecutor
import os
import requests
import m3u8
import shutil
import time
import datetime
import pytz
import cv2
import av
import random


app = Celery('tasks')
app.config_from_object('celery_config')

@app.task()
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
        max_segment_number = max(int(segment.uri.split('_')[-1].split('.')[0]) for segment in playlist.segments)
        min_segment_number = max(max_segment_number - buffer_size, 0)
        existing_segments = [int(x) for x in os.listdir('segments') if x != '.DS_Store']
        existing_segments.sort(reverse=True)
        existing_fsegments = [int(x) for x in os.listdir('frames') if x != '.DS_Store']
        existing_fsegments.sort(reverse=True)
        new_segments = [i for i in range(min_segment_number, max_segment_number) if i not in existing_segments]
        for ns in new_segments:
            download_segment.apply_async(args=[ns], expires=30, queue='ds')
        for inc in existing_segments[5:]:
            shutil.rmtree(f"segments/{inc}")
        for inc in existing_fsegments[5:]:
            shutil.rmtree(f"frames/{inc}")
        return new_segments
    
@app.task
def download_segment(segment, max_retries=3, delay=.2):
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
                    process_segment.apply_async(args=[segment], expires=30, queue='ps')
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
    # edge_color = get_random_rgb()
    # background_color = get_random_rgb()
    container = av.open(f"segments/{segment}/{segment}.ts")
    frame_data = [(segment, frame_number, frame.to_ndarray(format='bgr24'), edge_color, background_color, london_time.year, london_time.month, london_time.day, london_time.hour, london_time.minute) for frame_number, frame in enumerate(container.decode(container.streams.video[0]))]
    with ThreadPoolExecutor() as executor:
        executor.map(process_frame, frame_data)
    container.close()
    return segment

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

def get_random_rgb():
    """Generate a random RGB color."""
    return (random.randint(0, 255), random.randint(0, 255), random.randint(0, 255))

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
    # Unpack frame data
    segment_number, frame_number, frame, edge_color, background_color, lty, ltmnth, ltd, lth, ltm = frame_data
    try:
        adjusted = adjust_contrast_and_hue(frame, lth, ltm)

        blurred_array = cv2.GaussianBlur(adjusted, (11, 11), 0)

        # Convert to grayscale
        gray = cv2.cvtColor(blurred_array, cv2.COLOR_BGR2GRAY)

        # Edge detection
        edges = cv2.Canny(gray, 300, 400, apertureSize=5)

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
