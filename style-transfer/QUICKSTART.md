# Neural Style Transfer - Quick Start Guide

## 🎯 Goal

Replace the slow `cv::stylization()` (180s per segment) with fast neural style transfer (<6s per segment).

## 📁 What's Been Set Up

```
style-transfer/
├── notebooks/
│   └── 01_setup_and_quicktest.ipynb  # Start here!
├── models/                            # Trained models will go here
├── style_images/                      # Put Dali paintings here
├── test_images/                       # Test frames from Abbey Road
├── outputs/                           # Results will be saved here
└── requirements.txt                   # Dependencies
```

## 🚀 Getting Started

### Step 1: Install Dependencies

The dependencies have already been added to your `pyproject.toml`. To install:

```bash
poetry install
```

### Step 2: Launch Jupyter Notebook

```bash
cd style-transfer/notebooks
poetry run jupyter notebook
```

This will open Jupyter in your browser.

### Step 3: Run the Quick Test

1. Open `01_setup_and_quicktest.ipynb`
2. Run all cells (Cell → Run All)
3. This will:
   - Download a pre-trained model (~20MB)
   - Test on one of your Abbey Road frames
   - Benchmark inference speed
   - Show you the speedup vs cv::stylization

**Expected Results:**
- Inference time: 50-200ms per frame (vs 5500ms with cv::stylization)
- Per segment: 1.5-6s (vs 180s currently)
- **Speedup: 30-120x faster!** ✅

## 📊 What to Look For

After running the notebook, you'll see:

1. **Benchmark Results**:
   ```
   Average inference time: ~100ms per frame
   Estimated time per 30-frame segment: ~3s

   Comparison:
     Current cv::stylization: ~180s per segment
     Neural style transfer:   ~3s per segment
     Speedup:                 ~60x faster!
   ```

2. **Visual Comparison**:
   - Original frame vs styled frame side-by-side
   - Check if the artistic style is acceptable

## 🎨 Next Steps

If the speed looks good (which it should!):

### Option A: Use Pre-trained Model
- Export the pre-trained model to ONNX (last cell in notebook)
- Skip to C++ integration

### Option B: Train on Dali Style (Recommended for exact look)
1. Collect 5-10 Dali painting images
2. Put them in `style_images/dali/`
3. Run training notebook (coming next)
4. Export trained model to ONNX
5. Integrate into C++

## 🔧 Troubleshooting

### PyTorch Installation Issues

If you don't have a CUDA GPU:
```bash
# CPU-only PyTorch (slower but works)
poetry add torch torchvision --source https://download.pytorch.org/whl/cpu
```

If you have NVIDIA GPU:
```bash
# Check CUDA version
nvidia-smi

# Install appropriate PyTorch version
poetry add torch torchvision --source https://download.pytorch.org/whl/cu118
```

### Model Download Fails

The notebook will try to download a pre-trained model. If it fails:
1. Download manually from: https://www.dropbox.com/s/lrvwfehqdcxoza8/mosaic.pth
2. Save to `style-transfer/models/mosaic.pth`
3. Re-run notebook cells

## 📈 Performance Targets

| Metric | Current | Target | Expected with Neural ST |
|--------|---------|--------|------------------------|
| Time per frame | 5500ms | <200ms | 50-200ms ✅ |
| Time per segment | 180s | <6s | 1.5-6s ✅ |
| Quality | Perfect Dali | Same | Trainable to match |

## 💡 Key Insight

The pre-trained model is just for testing speed. To get the **exact Dali oil painting look** you love, you'll train a custom model on actual Dali paintings in the next notebook.

---

**Ready?** Start Jupyter and open `01_setup_and_quicktest.ipynb`!
