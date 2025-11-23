import cv2
import numpy as np
import os
import math
import shutil

# Try to import the fast C++ processor
try:
    import fast_processor
    USE_CPP = True
    print("✨ Using fast C++ processor")
except ImportError:
    USE_CPP = False
    fast_processor = None  # Set to None if not available
    print("⚠️  C++ processor not available, using Python implementation")

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
    Creates a Salvador Dali-inspired surrealist oil painting effect with melting forms,
    dream-like atmosphere, and painterly textures
    """
    # ========== SALVADOR DALI STYLE - ORIGINAL QUALITY, PARALLEL PROCESSING ==========
    # DOWNSAMPLING - Process at lower resolution for SPEED
    downsample_factor = 2            # 50% resolution = 4x fewer pixels = MUCH faster!
    process_every_nth_frame = 1      # Process ALL frames

    # SUBTLE MELTING EFFECT - Less psychedelic
    psychedelic_amplitude = 0.01     # Subtle warping (was 0.035)
    psychedelic_frequency = 20.0     # Higher frequency = smaller waves (was 8.0)
    psychedelic_total_frames = 180   # Animation cycle length

    # TRUE DALI OIL PAINTING: Use original cv2.stylization
    use_stylization = True           # The REAL oil painting effect
    stylize_sigma_s = 60             # Spatial range (original value)
    stylize_sigma_r = 0.6            # Color range (original value)

    # DETAIL ENHANCEMENT: Skip - adds time with minimal benefit
    detail_enhance = False           # Disabled to save ~30s per segment
    detail_sigma_s = 10
    detail_sigma_r = 0.15

    # ATMOSPHERIC SMOOTHING: Faster bilateral settings
    bilateral_d = 5                  # REDUCED for speed (was 7, smaller=faster)
    bilateral_sigma_color = 50       # Moderate smoothing
    bilateral_sigma_space = 50       # Preserve local details

    # TONAL MAPPING: Smooth gradients like oil paint
    quantization_levels = 16         # HIGH for smooth oil-paint transitions
    use_adaptive_threshold = True    # Adaptive toning for depth

    # PAINTERLY EDGES: DISABLED - too pixelated and slow
    canny_threshold_1 = 50           # (not used when disabled)
    canny_threshold_2 = 150          # (not used when disabled)
    edge_blend_factor = 0.0          # DISABLED - no edges!
    edge_blur_amount = 5             # (not used when disabled)

    # TEXTURE PRESERVATION: Minimal morphology to keep paint texture
    morph_kernel_size = 3            # Small kernel
    apply_opening = False            # Keep texture detail
    apply_closing_iterations = 1     # Minimal smoothing

    # ===================================================================

    # Unpack frame data
    segment_number, frame_number, frame, edge_color, background_color, lty, ltmnth, ltd, lth, ltm = frame_data

    try:
        # Convert segment_number to int if it's a string
        if isinstance(segment_number, str):
            segment_number = int(segment_number)

        # Only process every Nth frame for performance
        if frame_number % process_every_nth_frame != 0:
            base_frame = (frame_number // process_every_nth_frame) * process_every_nth_frame
            base_dir = os.path.dirname(os.path.dirname(os.path.dirname(__file__)))
            frames_dir = os.path.join(base_dir, "data", "frames", str(segment_number))
            base_path = os.path.join(frames_dir, f"{base_frame}.jpg")
            dest_path = os.path.join(frames_dir, f"{frame_number}.jpg")

            # Wait a bit for base frame to be created if needed
            import time
            max_wait = 5  # seconds
            wait_count = 0
            while not os.path.exists(base_path) and wait_count < max_wait * 10:
                time.sleep(0.1)
                wait_count += 1

            if os.path.exists(base_path):
                shutil.copy2(base_path, dest_path)
                return True
            else:
                # Base frame doesn't exist, skip this frame
                return False

        # ALWAYS use C++ implementation - no Python fallback!
        if not USE_CPP:
            raise RuntimeError("C++ processor not available! Run ./build_cpp.sh to build it.")

        carbonized_bgr = fast_processor.process_frame(
            frame,
            frame_number,
            psychedelic_amplitude,
            psychedelic_frequency,
            psychedelic_total_frames,
            use_stylization,
            stylize_sigma_s,
            stylize_sigma_r,
            detail_enhance,
            detail_sigma_s,
            detail_sigma_r,
            bilateral_d,
            bilateral_sigma_color,
            bilateral_sigma_space,
            quantization_levels,
            use_adaptive_threshold,
            edge_blend_factor,
            downsample_factor,
            canny_threshold_1,
            canny_threshold_2,
            morph_kernel_size,
            apply_opening,
            apply_closing_iterations,
            edge_blur_amount
        )

        # No Python fallback - deleted for performance
        if False:
            # Python fallback implementation
            # Store original dimensions
            original_height, original_width = frame.shape[:2]

            # Downsample for faster processing
            if downsample_factor > 1:
                small_width = original_width // downsample_factor
                small_height = original_height // downsample_factor
                frame_small = cv2.resize(frame, (small_width, small_height), interpolation=cv2.INTER_AREA)
            else:
                frame_small = frame

            # SURREALIST TECHNIQUE: Apply enhanced psychedelic distortion for melting effect
            distorted_frame = apply_psychedelic_distortion(
                frame_small, frame_number,
                amplitude=psychedelic_amplitude,
                frequency=psychedelic_frequency,
                total_frames=psychedelic_total_frames
            )

            # OIL PAINTING EFFECT: Try faster edge-preserving filter
            if use_edge_preserving:
                # Edge preserving filter - MUCH faster than stylization
                distorted_frame = cv2.edgePreservingFilter(
                    distorted_frame,
                    flags=edge_preserve_flags,
                    sigma_s=edge_preserve_sigma_s,
                    sigma_r=edge_preserve_sigma_r
                )
            elif use_oil_painting:
                # Try xphoto oil painting filter if available
                try:
                    distorted_frame = cv2.xphoto.oilPainting(
                        distorted_frame,
                        oil_painting_size,
                        oil_painting_dynRatio
                    )
                except:
                    pass  # Fall back if not available

            # Convert to grayscale
            gray = cv2.cvtColor(distorted_frame, cv2.COLOR_BGR2GRAY)

            # ATMOSPHERIC DEPTH: Gentle bilateral smoothing
            smooth = cv2.bilateralFilter(gray, bilateral_d, bilateral_sigma_color, bilateral_sigma_space)

            # TONAL MAPPING: Smooth gradients like oil paint
            if use_adaptive_threshold:
                # Adaptive histogram equalization for depth and atmosphere
                clahe = cv2.createCLAHE(clipLimit=2.0, tileGridSize=(8, 8))
                smooth = clahe.apply(smooth)

            # Gentle quantization for tonal variation
            level_step = 255.0 / (quantization_levels - 1)
            quantized = np.floor(smooth / level_step + 0.5) * level_step
            quantized = quantized.astype(np.uint8)

            # MINIMAL MORPHOLOGY: Preserve painterly texture
            kernel = np.ones((morph_kernel_size, morph_kernel_size), np.uint8)

            if apply_opening:
                quantized = cv2.morphologyEx(quantized, cv2.MORPH_OPEN, kernel)

            for _ in range(apply_closing_iterations):
                quantized = cv2.morphologyEx(quantized, cv2.MORPH_CLOSE, kernel)

            # PAINTERLY EDGES: Skip entirely if disabled
            if edge_blend_factor > 0:
                edges = cv2.Canny(quantized, canny_threshold_1, canny_threshold_2)
                edges = cv2.GaussianBlur(edges, (edge_blur_amount, edge_blur_amount), 0)
                edges_scaled = (edges * edge_blend_factor).astype(np.uint8)
                quantized = cv2.add(quantized, edges_scaled)

            # Upsample back to original size if we downsampled
            if downsample_factor > 1:
                quantized = cv2.resize(quantized, (original_width, original_height), interpolation=cv2.INTER_NEAREST)

            # Create output frame - pure blobs without edge overlays
            carbonized_bgr = np.zeros((original_height, original_width, 3), dtype=np.uint8)
            carbonized_bgr[:, :, 0] = quantized
            carbonized_bgr[:, :, 1] = quantized
            carbonized_bgr[:, :, 2] = quantized

        # Save the processed frame
        # Use absolute path from BASE_DIR
        base_dir = os.path.dirname(os.path.dirname(os.path.dirname(__file__)))
        frames_dir = os.path.join(base_dir, "data", "frames", str(segment_number))
        os.makedirs(frames_dir, exist_ok=True)
        frame_path = os.path.join(frames_dir, f"{frame_number}.jpg")

        cv2.imwrite(frame_path, carbonized_bgr)

        return True
    except Exception as e:
        print(f"Error processing fast blob frame: {e}")
        import traceback
        traceback.print_exc()
        return e