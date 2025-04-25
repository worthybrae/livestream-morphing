import cv2
import numpy as np
import os
import math
import shutil

def get_grey_level(hour, minute):
    """
    Interpolates grey level based on hour and minute.
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
    """
    Determines background and edge colors based on time of day.
    """
    # Get grey level for background
    grey_level = get_grey_level(hour, minute)
    background_color = (grey_level, grey_level, grey_level)

    # Determine the most contrasting edge color
    if grey_level > 127:  # If background is light
        edge_color = (0, 0, 0)  # Use black
    else:  # If background is dark or mid-tone
        edge_color = (255, 255, 255)  # Use white

    return edge_color, background_color

def apply_psychedelic_distortion(image, frame_number, amplitude=0.02, frequency=30.0, total_frames=180):
    """
    Apply a psychedelic distortion effect to an image using a sine wave distortion
    """
    # Create normalized time variable (0.0 to 2π) for full cycle
    time = (frame_number % total_frames) * (2 * math.pi / total_frames)

    # Get image dimensions
    height, width = image.shape[:2]

    # Create meshgrid for pixel coordinates
    y_coords, x_coords = np.mgrid[0:height, 0:width]

    # Convert to float32 for OpenCV remap function
    x_coords = x_coords.astype(np.float32)
    y_coords = y_coords.astype(np.float32)

    # Apply sine wave distortion (similar to the shader code)
    x_distorted = x_coords + np.sin(time + x_coords * frequency / width) * (width * amplitude)
    y_distorted = y_coords + np.sin(time + y_coords * frequency / height) * (height * amplitude)

    # Remap the image using the distorted coordinates
    distorted_image = cv2.remap(image, x_distorted, y_distorted, cv2.INTER_LINEAR, borderMode=cv2.BORDER_REPLICATE)

    return distorted_image

def process_frame_fast_blobs(frame_data):
    """
    Creates a blobby, highly quantized effect with less detail and more prominent blobs
    """
    # Unpack frame data
    segment_number, frame_number, frame, edge_color, background_color, lty, ltmnth, ltd, lth, ltm = frame_data

    try:
        # Convert segment_number to int if it's a string
        if isinstance(segment_number, str):
            segment_number = int(segment_number)

        # Only process every 3rd frame for max speed
        if frame_number % 3 != 0:
            base_frame = (frame_number // 3) * 3
            if os.path.exists(f"frames/{segment_number}/{base_frame}.jpg"):
                shutil.copy(f"frames/{segment_number}/{base_frame}.jpg",
                           f"frames/{segment_number}/{frame_number}.jpg")
                return True

        # Apply moderate distortion effect
        distorted_frame = apply_psychedelic_distortion(
            frame, frame_number, amplitude=0.01, frequency=20.0, total_frames=180
        )

        # Convert to grayscale
        gray = cv2.cvtColor(distorted_frame, cv2.COLOR_BGR2GRAY)

        # Apply moderate bilateral filter with parameters tuned for slightly more detail
        smooth = cv2.bilateralFilter(gray, 15, 80, 80)  # Reduced diameter to preserve more edges

        # Apply lighter Gaussian blur to retain more detail
        smooth = cv2.GaussianBlur(smooth, (9, 9), 0)  # Smaller kernel to preserve more texture

        # Increase quantization levels slightly for more nuanced detail
        num_levels = 8  # Increased from 4 to 5 for slightly more detail
        level_step = 255.0 / (num_levels - 1)
        quantized = np.floor(smooth / level_step + 0.5) * level_step
        quantized = quantized.astype(np.uint8)

        # Apply gentler morphological operations to preserve more detail
        kernel = np.ones((3, 3), np.uint8)  # Smaller kernel to preserve finer details

        # Apply light morphological closing to maintain more texture
        quantized = cv2.morphologyEx(quantized, cv2.MORPH_CLOSE, kernel)

        # Optional: Selective edge enhancement to bring out some details
        # Create an edge mask for selective detail enhancement
        edges = cv2.Canny(quantized, 30, 100)
        edges = cv2.dilate(edges, kernel, iterations=1)

        # Add a small amount of the edges back to the quantized image
        # Scale down the edges to just hint at details without overwhelming the blob effect
        edge_blend_factor = 0.2
        edges_scaled = (edges * edge_blend_factor).astype(np.uint8)
        quantized = cv2.add(quantized, edges_scaled)

        # Create output frame - pure blobs without edge overlays
        carbonized_bgr = np.zeros_like(frame)
        carbonized_bgr[:, :, 0] = quantized
        carbonized_bgr[:, :, 1] = quantized
        carbonized_bgr[:, :, 2] = quantized

        # Save the processed frame
        cv2.imwrite(f"frames/{segment_number}/{frame_number}.jpg", carbonized_bgr)

        return True
    except Exception as e:
        print(f"Error processing fast blob frame: {e}")
        return e