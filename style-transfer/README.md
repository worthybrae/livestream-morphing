# Neural Style Transfer for Dali Oil Painting Effect

This directory contains the neural style transfer model training, testing, and export pipeline for the livestream morphing project.

## Directory Structure

```
style-transfer/
├── notebooks/          # Jupyter notebooks for experimentation and training
├── models/            # Trained models and exported ONNX/TensorRT files
├── style_images/      # Dali paintings and other artistic reference images
├── test_images/       # Test frames from Abbey Road stream
├── outputs/           # Sample outputs during training/testing
├── requirements.txt   # Python dependencies
└── README.md         # This file
```

## Goal

Replace the slow `cv::stylization()` (180s per segment) with fast neural style transfer (<6s per segment).

## Workflow

1. **Experiment** - Use Jupyter notebook to test pre-trained models
2. **Train** - Fine-tune on Dali painting style images
3. **Export** - Convert PyTorch model to ONNX format
4. **Optimize** - Convert ONNX to TensorRT for maximum speed
5. **Integrate** - Add TensorRT inference to C++ pipeline

## Expected Performance

- Current: cv::stylization @ 5500ms/frame → 180s per 30-frame segment
- Target: Neural style transfer @ 50-200ms/frame → 1.5-6s per segment
- **Speedup**: 30-120x faster!

## Setup

See notebooks/setup_and_training.ipynb for detailed setup instructions.
