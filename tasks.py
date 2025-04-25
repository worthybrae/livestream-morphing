from celery import Celery
from concurrent.futures import ThreadPoolExecutor
import os
import requests
import m3u8
import shutil
import time
import datetime
import pytz
import av
import boto3
from redis import Redis
from dotenv import load_dotenv
import ffmpeg
from io import BytesIO

# Import processing functions from separate file
from image_processing import get_colors, process_frame_fast_blobs

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
    """
    Task that fetches new video segments from the Abbey Road webcam feed.
    Runs every 2 seconds via Celery Beat.
    """
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

            # If the list exceeds 10 elements, remove the oldest one
            if len(current_segments) >= 10:
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
    """
    Task that downloads a video segment from the Abbey Road webcam feed.

    Args:
        segment (str): Segment identifier
        max_retries (int): Maximum number of download attempts
        delay (float): Delay between retry attempts
    """
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
    """
    Task that processes a video segment, extracting frames and applying the carbonized effects.

    Args:
        segment (str): Segment identifier
    """
    redis_client = Redis.from_url('redis://127.0.0.1:6379/0')
    london_time = datetime.datetime.now(pytz.timezone('Europe/London'))

    # Get colors for the current time of day
    edge_color, background_color = get_colors(london_time.hour, london_time.minute)

    # Open the video container
    container = av.open(f"segments/{segment}/{segment}.ts")

    # Prepare frame data for processing
    frame_data = [
        (segment, frame_number, frame.to_ndarray(format='bgr24'),
         edge_color, background_color,
         london_time.year, london_time.month, london_time.day,
         london_time.hour, london_time.minute)
        for frame_number, frame in enumerate(container.decode(container.streams.video[0]))
    ]

    # Process frames in parallel using the carbonized processing function
    with ThreadPoolExecutor() as executor:
        executor.map(process_frame_fast_blobs, frame_data)  # Changed from process_frame_enhanced_edges
    container.close()

    # Combine processed frames into a video
    out, err = (
        ffmpeg
        .input(f'frames/{segment}/%d.jpg', r=30, f='image2', s='1972x1140')
        .output('pipe:', vcodec='libx264', crf=25, pix_fmt='yuv420p', format='mpegts')
        .run(capture_stdout=True, capture_stderr=True)
    )
    print("FFmpeg stderr:", err.decode('utf-8'))

    # Upload processed video to S3
    upload_to_s3(
        BytesIO(out),
        f"segments/{segment}.ts"
    )

    # Update the ready_segments queue
    ready_segment_count = len(redis_client.lrange('ready_segments', 0, -1))
    if ready_segment_count >= 10:
        redis_client.rpop('ready_segments')
    redis_client.lpush('ready_segments', f"{segment}")

    # Generate playlist
    generate_m3u8_file.apply_async(args=[], expires=30, queue='gm')

@app.task
def generate_m3u8_file():
    """
    Task that generates an M3U8 playlist file from processed segments.
    """
    redis_client = Redis.from_url('redis://127.0.0.1:6379/0')
    bucket_name = 'abbey-road'

    # Get the most recent segments
    ready_segments = redis_client.lrange('ready_segments', 0, -1)
    ready_segments = sorted([int(s.decode('utf-8')) for s in ready_segments])[:3]

    # Construct the M3U8 content
    m3u8_content = (
        f"#EXTM3U\n"
        f"#EXT-X-VERSION:3\n"
        f"#EXT-X-TARGETDURATION:6\n"
        f"#EXT-X-MEDIA-SEQUENCE:{ready_segments[0]}\n"
    )

    # Add each segment to the playlist
    for segment in ready_segments:
        alt_path = f"https://{bucket_name}.s3.amazonaws.com/segments/{segment}.ts"
        m3u8_content += f"#EXTINF:6,\n{alt_path}\n"

    # Upload the playlist to S3
    bytes_file = BytesIO(m3u8_content.encode('utf-8'))
    upload_to_s3(bytes_file, f"current_playlist.m3u8")
    return True

def upload_to_s3(file_data, object_name):
    """
    Uploads a file to an S3 bucket.

    Args:
        file_data: File data to upload
        object_name: Name of the object in the S3 bucket
    """
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
    """
    Clears all contents of a folder.

    Args:
        folder_path: Path to the folder to clear
    """
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
    """
    Initial setup function to clear old data and initialize Redis queues.
    """
    redis_client = Redis.from_url('redis://127.0.0.1:6379/0')
    redis_client.delete('recent_segments')
    redis_client.delete('ready_segments')
    clear_folder_contents('frames')
    clear_folder_contents('segments')
    clear_folder_contents('modified_segments')