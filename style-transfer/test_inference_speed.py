"""
Test neural style transfer inference speed on your hardware.
This will give us a baseline for expected performance.
"""

import torch
import time
import numpy as np
from pathlib import Path
import sys

# Add current directory to path
sys.path.append(str(Path(__file__).parent))
from train import TransformerNet


def benchmark_model(image_size=(1080, 1920), num_warmup=5, num_runs=20):
    """
    Benchmark the neural style transfer model.

    Args:
        image_size: (height, width) tuple for input size
        num_warmup: Number of warmup runs
        num_runs: Number of benchmark runs
    """
    print("=" * 60)
    print("NEURAL STYLE TRANSFER INFERENCE BENCHMARK")
    print("=" * 60)
    print()

    # Device setup
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    print(f"Device: {device}")
    if torch.cuda.is_available():
        print(f"GPU: {torch.cuda.get_device_name(0)}")
    print()

    # Create model (random weights - just testing speed)
    print("Creating model...")
    model = TransformerNet()
    model = model.to(device)
    model.eval()

    total_params = sum(p.numel() for p in model.parameters())
    print(f"Model parameters: {total_params:,}")
    print()

    # Create dummy input (Abbey Road stream resolution)
    height, width = image_size
    print(f"Input size: {height}x{width}")
    dummy_input = torch.randn(1, 3, height, width).to(device)
    print(f"Input tensor shape: {dummy_input.shape}")
    print()

    # Warmup
    print(f"Warming up ({num_warmup} runs)...")
    with torch.no_grad():
        for _ in range(num_warmup):
            _ = model(dummy_input)
            if device.type == 'cuda':
                torch.cuda.synchronize()
    print("Warmup complete!")
    print()

    # Benchmark
    print(f"Running benchmark ({num_runs} runs)...")
    times = []
    with torch.no_grad():
        for i in range(num_runs):
            start = time.perf_counter()
            output = model(dummy_input)
            if device.type == 'cuda':
                torch.cuda.synchronize()
            end = time.perf_counter()
            elapsed_ms = (end - start) * 1000
            times.append(elapsed_ms)

            if (i + 1) % 5 == 0:
                print(f"  Run {i+1}/{num_runs}: {elapsed_ms:.2f}ms")

    print()

    # Calculate statistics
    times = np.array(times)
    mean_time = np.mean(times)
    std_time = np.std(times)
    min_time = np.min(times)
    max_time = np.max(times)

    # Calculate segment timing (30 frames per segment)
    frames_per_segment = 30
    segment_time = (mean_time * frames_per_segment) / 1000  # Convert to seconds

    # Current cv::stylization timing
    current_stylization_per_frame = 5500  # ms
    current_segment_time = (current_stylization_per_frame * frames_per_segment) / 1000  # seconds

    speedup = current_segment_time / segment_time

    # Print results
    print("=" * 60)
    print("RESULTS")
    print("=" * 60)
    print()
    print(f"Inference time per frame:")
    print(f"  Mean:   {mean_time:.2f}ms ± {std_time:.2f}ms")
    print(f"  Min:    {min_time:.2f}ms")
    print(f"  Max:    {max_time:.2f}ms")
    print()
    print(f"Time per 30-frame segment:")
    print(f"  Neural ST:  {segment_time:.2f}s")
    print(f"  cv::stylization: {current_segment_time:.0f}s")
    print()
    print(f"SPEEDUP: {speedup:.1f}x faster! 🚀")
    print()

    # Check if we meet the <6s target
    if segment_time < 6.0:
        print("✅ SUCCESS! Meets <6s per segment target!")
        print(f"   ({segment_time:.2f}s is well under the 6s goal)")
    else:
        print("⚠️  Does not meet <6s target")
        print(f"   ({segment_time:.2f}s is over the 6s goal)")
        print("   Consider GPU acceleration with TensorRT")

    print()
    print("=" * 60)
    print()

    return {
        'mean_ms': mean_time,
        'std_ms': std_time,
        'segment_time_s': segment_time,
        'speedup': speedup,
        'meets_target': segment_time < 6.0
    }


if __name__ == "__main__":
    print()
    print("This will benchmark neural style transfer inference speed")
    print("on your hardware using a random (untrained) model.")
    print()
    print("The goal: <6 seconds per 30-frame segment")
    print("Current cv::stylization: ~180 seconds per segment")
    print()
    print("Starting benchmark...")
    print()

    results = benchmark_model()

    print()
    print("Next steps:")
    print()
    if results['meets_target']:
        print("1. ✅ Speed is good! Now train on Dali paintings for quality")
        print("2. Export trained model to ONNX")
        print("3. Integrate into C++ with ONNX Runtime")
        print("4. Optional: Optimize further with TensorRT")
    else:
        print("1. The model architecture is fast, but CPU is too slow")
        print("2. Options:")
        print("   - Use GPU with CUDA")
        print("   - Optimize with TensorRT")
        print("   - Use smaller model architecture")
    print()
