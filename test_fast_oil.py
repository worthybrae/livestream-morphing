"""
Quick test of the new fast oil painting effect
"""

import fast_processor
import cv2
import numpy as np
import time
from pathlib import Path

# Find a test frame
frames_dir = Path("data/frames")
test_frames = list(frames_dir.glob("**/*.jpg"))

if not test_frames:
    print("No test frames found, creating dummy frame...")
    frame = np.random.randint(0, 255, (1080, 1920, 3), dtype=np.uint8)
else:
    frame_path = test_frames[0]
    print(f"Using frame: {frame_path}")
    frame = cv2.imread(str(frame_path))
    print(f"Frame shape: {frame.shape}")

print()
print("=" * 60)
print("TESTING FAST OIL PAINTING EFFECT")
print("=" * 60)
print()

# Warmup
print("Warming up (3 runs)...")
for _ in range(3):
    _ = fast_processor.process_frame(
        frame,
        frame_number=0,
        psychedelic_amplitude=0.035,
        psychedelic_frequency=8.0,
        psychedelic_total_frames=180,
        use_stylization=True,
        stylize_sigma_s=60.0,
        stylize_sigma_r=0.6,
        detail_enhance=False,
        detail_sigma_s=10.0,
        detail_sigma_r=0.15,
        bilateral_d=5,
        bilateral_sigma_color=50,
        bilateral_sigma_space=50,
        quantization_levels=16,
        use_adaptive_threshold=True,
        edge_blend_factor=0.0,
        downsample_factor=1,
        canny_threshold_1=50,
        canny_threshold_2=150,
        morph_kernel_size=3,
        apply_opening=False,
        apply_closing_iterations=1,
        edge_blur_amount=5
    )

print("Warmup complete!")
print()

# Benchmark
print("Running benchmark (20 iterations)...")
times = []

for i in range(20):
    start = time.time()

    result = fast_processor.process_frame(
        frame,
        frame_number=i,
        psychedelic_amplitude=0.035,
        psychedelic_frequency=8.0,
        psychedelic_total_frames=180,
        use_stylization=True,
        stylize_sigma_s=60.0,
        stylize_sigma_r=0.6,
        detail_enhance=False,
        detail_sigma_s=10.0,
        detail_sigma_r=0.15,
        bilateral_d=5,
        bilateral_sigma_color=50,
        bilateral_sigma_space=50,
        quantization_levels=16,
        use_adaptive_threshold=True,
        edge_blend_factor=0.0,
        downsample_factor=1,
        canny_threshold_1=50,
        canny_threshold_2=150,
        morph_kernel_size=3,
        apply_opening=False,
        apply_closing_iterations=1,
        edge_blur_amount=5
    )

    end = time.time()
    elapsed = (end - start) * 1000  # Convert to ms

    times.append(elapsed)

    if (i + 1) % 5 == 0:
        avg_so_far = np.mean(times)
        print(f"  After {i+1} runs: {avg_so_far:.2f}ms average")

print()

# Results
times = np.array(times)
mean_time = np.mean(times)
std_time = np.std(times)
min_time = np.min(times)
max_time = np.max(times)

segment_time = (mean_time * 30) / 1000  # 30 frames per segment, in seconds

print("=" * 60)
print("RESULTS")
print("=" * 60)
print()
print(f"Time per frame:")
print(f"  Mean:   {mean_time:.2f}ms ± {std_time:.2f}ms")
print(f"  Min:    {min_time:.2f}ms")
print(f"  Max:    {max_time:.2f}ms")
print()
print(f"Time per 30-frame segment:")
print(f"  Fast oil painting:  {segment_time:.2f}s")
print(f"  Old cv::stylization: ~180s")
print()
print(f"SPEEDUP: {180 / segment_time:.1f}x faster! 🚀")
print()

if segment_time < 6.0:
    print("✅ SUCCESS! Meets <6s per segment target!")
    print(f"   ({segment_time:.2f}s is well under the 6s goal)")
elif segment_time < 30.0:
    print("⚠️  Good speedup but doesn't quite meet <6s")
    print(f"   ({segment_time:.2f}s vs 6s target)")
    print("   But still MUCH better than 180s!")
else:
    print("❌ Still too slow")
    print(f"   ({segment_time:.2f}s)")

print()
print("=" * 60)
print()

# Save a sample
if result is not None:
    output_path = "fast_oil_sample.jpg"
    cv2.imwrite(output_path, result)
    print(f"✅ Sample output saved to: {output_path}")
    print("   Check the visual quality!")
