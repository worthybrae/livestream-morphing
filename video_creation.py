import os
import subprocess
from pathlib import Path

import cv2


def create_segment_video(segment_number, output_path, crf=26, scale=0.7, fps=30):
    """Create a compressed video from frames in a segment folder"""
    frames_dir = f"frames/{segment_number}"
    temp_output = f"temp_segment_{segment_number}.mp4"

    # Get the first frame to determine dimensions
    first_frame = cv2.imread(os.path.join(frames_dir, "0.jpg"))
    if first_frame is None:
        print(f"Error: Could not read first frame in segment {segment_number}")
        return None

    height, width = first_frame.shape[:2]

    # Calculate new dimensions (70% of original for better quality)
    new_width = int(width * scale)
    new_height = int(height * scale)

    # Use ffmpeg directly instead of OpenCV VideoWriter for better compression
    temp_dir = Path(f"temp_frames_{segment_number}")
    temp_dir.mkdir(exist_ok=True)

    # Copy and resize frames to temp directory
    for i in range(180):  # 0-179 frames
        frame_path = os.path.join(frames_dir, f"{i}.jpg")
        if os.path.exists(frame_path):
            frame = cv2.imread(frame_path)
            if frame is not None:
                # Resize frame
                resized_frame = cv2.resize(frame, (new_width, new_height))
                # Save to temp directory with moderate JPEG compression
                cv2.imwrite(
                    str(temp_dir / f"{i:04d}.jpg"),
                    resized_frame,
                    [cv2.IMWRITE_JPEG_QUALITY, 90],
                )
        else:
            print(f"Warning: Missing frame {i} in segment {segment_number}")

    # Use ffmpeg to create video with H.265 encoding
    cmd = [
        "ffmpeg",
        "-framerate",
        str(fps),
        "-i",
        str(temp_dir / "%04d.jpg"),
        "-c:v",
        "libx265",  # H.265 codec for better compression
        "-crf",
        str(crf),  # Slightly better quality (lower CRF)
        "-preset",
        "medium",  # Balanced compression/speed
        "-pix_fmt",
        "yuv420p",  # Standard pixel format        # Optimize for film-like content
        temp_output,
    ]

    subprocess.run(cmd)

    # Clean up temp frames
    for file in temp_dir.glob("*.jpg"):
        os.remove(file)
    temp_dir.rmdir()

    return temp_output


def combine_segments(segment_files, output_path):
    """Combine multiple segment videos into one final video"""
    # Create a file listing all segments to concatenate
    with open("segments.txt", "w") as f:
        for segment in segment_files:
            f.write(f"file '{segment}'\n")

    # Use ffmpeg to concatenate segments with additional compression
    cmd = [
        "ffmpeg",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
        "segments.txt",
        "-c:v",
        "libx265",  # Use H.265 for final output
        "-crf",
        "5",  # Better quality for final output
        "-preset",
        "medium",  # Balance between compression and encoding speed      # Optimized for film-like content
        output_path,
    ]

    subprocess.run(cmd)


def main():
    # Create output directory if it doesn't exist
    output_dir = Path("public/assets/livestream-art")
    output_dir.mkdir(parents=True, exist_ok=True)

    output_path = output_dir / "abbey-road-stream.mp4"

    # Create videos for each segment
    segment_files = []
    for segment in range(56486, 56496):  # 9091 to 9094 segments
        print(f"Processing segment {segment}...")
        temp_file = create_segment_video(
            segment,
            output_path,
            crf=5,  # Lower CRF = better quality (range 0-51)
            scale=1,  # Reduce resolution to 70% (better quality)
            fps=30,  # Keep original framerate
        )
        if temp_file:
            segment_files.append(temp_file)

    # Combine all segments
    print("Combining segments...")
    combine_segments(segment_files, output_path)

    # Clean up temporary files
    print("Cleaning up...")
    for temp_file in segment_files:
        os.remove(temp_file)
    os.remove("segments.txt")

    print(f"Video created at {output_path}")


if __name__ == "__main__":
    main()
