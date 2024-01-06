import av
import cv2
import numpy as np
import pytz
import time
import datetime
from concurrent.futures import ThreadPoolExecutor


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
    frame = adjust_contrast_and_hue(frame, london_time.hour, london_time.minute)
    blurred_array = cv2.GaussianBlur(frame, (11, 11), 0)

    # Convert to grayscale
    gray = cv2.cvtColor(blurred_array, cv2.COLOR_BGR2GRAY)

    # Edge detection
    edges = cv2.Canny(gray, 100, 200, apertureSize=5)

    # Create a background and apply edge color
    background = np.full_like(frame, background_color)
    background[edges > 0] = edge_color
    cv2.imwrite(f"frames/{segment_number}/{frame_number}.jpg", background)

def process_video(segment):
    start = time.time()
    container = av.open(f"segments/{segment}.ts")
    frame_data = [(segment, frame_number, frame.to_ndarray(format='bgr24')) for frame_number, frame in enumerate(container.decode(container.streams.video[0]))]
    with ThreadPoolExecutor() as executor:
        results = list(executor.map(process_frame, frame_data))
    container.close()
    end = time.time()
    print(f"{end - start:.1f}")
    return results


london_time = datetime.datetime.now(pytz.timezone('Europe/London'))
edge_color, background_color = get_colors(london_time.hour, london_time.minute)
# Replace with your TS file path
segment = "145540"
frames = process_video(segment)

print(f"Total Frames: {len(frames)}")
