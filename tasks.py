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
from redis import Redis
import boto3
from dotenv import load_dotenv
import ffmpeg
from io import BytesIO


load_dotenv(override=True)

app = Celery('tasks')
app.config_from_object('celery_config')
app.conf.beat_schedule = {
    'fetch-new-segment-task': {
        'task': 'tasks.fetch_new_segment',
        'schedule': 2.0
    }
}

@app.task()
def fetch_new_segment():
    redis_client = Redis.from_url('redis://127.0.0.1:6379/0')
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
        new_segment = playlist.segments[0].uri.split('_')[-1].split('.')[0]
        # Retrieve the current list of recent segments
        current_segments = redis_client.lrange('recent_segments', 0, -1)
        current_segments = [segment.decode('utf-8') for segment in current_segments]
        if len(current_segments) == 0:
            setup()
        new = str(new_segment) not in current_segments
        print(f"New Segment #: {new_segment}\nCurrent Segments: {current_segments}\nNew?: {new}\n")

        # Check if the segment is already in the list
        if new:
            print(f"new segment {new_segment} found")
            # Add the new segment to the start of the list
            redis_client.lpush('recent_segments', str(new_segment))

            download_segment.apply_async(args=[str(new_segment)], expires=30, queue='ds')

            # If the list exceeds 5 elements, remove the oldest one
            if len(current_segments) >= 5:
                removed_segment = redis_client.rpop('recent_segments')
                if removed_segment:
                    removed_segment = removed_segment.decode('utf-8')
                    try:
                        shutil.rmtree(f"frames/{removed_segment}")
                        shutil.rmtree(f"segments/{removed_segment}")
                        shutil.rmtree(f"modified_segments/{removed_segment}")
                    except Exception as e:
                        print(e)
            return new_segment
        else:
            return current_segments
    
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
                    process_segment.apply_async(args=[segment], expires=40, queue='ps')
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
    rounded_seconds = round_seconds(london_time.second)
    if not os.path.exists(f"modified_segments/{london_time.hour}/{london_time.minute}/{rounded_seconds}"):
        os.makedirs(f"modified_segments/{london_time.hour}/{london_time.minute}/{rounded_seconds}", exist_ok=True)
        
        edge_color, background_color = get_colors(london_time.hour, london_time.minute)
        # edge_color = get_random_rgb()
        # background_color = get_random_rgb()
        container = av.open(f"segments/{segment}/{segment}.ts")
        frame_data = [(segment, frame_number, frame.to_ndarray(format='bgr24'), edge_color, background_color, london_time.year, london_time.month, london_time.day, london_time.hour, london_time.minute) for frame_number, frame in enumerate(container.decode(container.streams.video[0]))]
        with ThreadPoolExecutor() as executor:
            executor.map(process_frame, frame_data)
        container.close()
        # Construct the ffmpeg command for creating a .ts file
        out, _ = (
            ffmpeg
            .input(f'frames/{segment}/%d.jpg', r=30, f='image2', s='1972x1140')
            .output('pipe:', vcodec='libx264', crf=25, pix_fmt='yuv420p', format='mpegts')
            .run(capture_stdout=True, capture_stderr=True)
        )

        upload_to_s3(
            BytesIO(out),
            f"segments/{london_time.hour}/{london_time.minute}/{rounded_seconds}.ts"
        )        
        print(rounded_seconds)
        if rounded_seconds == 60:
            generate_m3u8_file.apply_async(args=[london_time.hour, london_time.minute], expires=30, queue='gm')
            
    else:
        return "Duplicate Process"

@app.task
def generate_m3u8_file(hr, min):
    s3_client = boto3.client(
        's3',
        aws_access_key_id=os.getenv('AWS_PUB_KEY'),
        aws_secret_access_key=os.getenv('AWS_SECRET_KEY'),
        region_name='us-east-1'
    )

    bucket_name = 'abbey-road'
    prefix = f'segments/{hr}/{min}'

     # Construct the M3U8 content with signed URLs
    m3u8_content = "#EXTM3U\n#EXT-X-VERSION:3\n"

    # List objects within a specified bucket and prefix
    response = s3_client.list_objects_v2(Bucket=bucket_name, Prefix=prefix)

    # Iterate over the objects and print the keys
    if 'Contents' in response:
        for item in response['Contents']:
            alt_path = f"https://{bucket_name}.s3.amazonaws.com/{item['Key']}"
            m3u8_content += f"#EXTINF:6,\n{alt_path}\n"
        m3u8_content += "#EXT-X-ENDLIST"
        bytes_file = BytesIO(m3u8_content.encode('utf-8'))
        upload_to_s3(bytes_file, f"current_playlist.m3u8") 
        print('uploaded')
    else:
        print("No items found.")

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

def round_seconds(seconds, interval=6):
    rounded_seconds = round(seconds / interval) * interval
    # Handle the case where rounding leads to 60
    if rounded_seconds == 0:
        rounded_seconds = 6
    
    return rounded_seconds

def upload_to_s3(file_data, object_name):
    bucket_name = "abbey-road"

    # Create a session using the specified AWS credentials and region
    session = boto3.Session(
        aws_access_key_id=os.getenv('AWS_PUB_KEY'),
        aws_secret_access_key=os.getenv('AWS_SECRET_KEY'),
        region_name='us-east-1'
    )
    s3 = session.client("s3")

    try:
        # Upload the file
        response = s3.upload_fileobj(file_data, bucket_name, object_name)
        print(f"File {object_name} uploaded to {bucket_name}")
        return response
    except Exception as e:
        # Output the error if something goes wrong
        print(f"Failed to upload {object_name} to {bucket_name}: {e}")
        return None
    
def clear_folder_contents(folder_path):
    # Check if the given path is an actual directory
    if not os.path.isdir(folder_path):
        print("The specified path is not a directory")
        return

    # List all the contents of the directory
    for item_name in os.listdir(folder_path):
        item_path = os.path.join(folder_path, item_name)
        try:
            # Check if it's a file or directory
            if os.path.isfile(item_path) or os.path.islink(item_path):
                os.unlink(item_path)  # Remove files and links
            elif os.path.isdir(item_path):
                shutil.rmtree(item_path)  # Remove directories
        except Exception as e:
            print(f"Failed to delete {item_path}. Reason: {e}")

def setup():
    redis_client = Redis.from_url('redis://127.0.0.1:6379/0')
    redis_client.delete('recent_segments')
    clear_folder_contents('frames')
    clear_folder_contents('segments')
    clear_folder_contents('modified_segments')