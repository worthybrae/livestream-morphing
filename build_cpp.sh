#!/bin/bash

# Script to build the C++ extension for fast image processing

echo "🔨 Building C++ extension..."

# Check if OpenCV is installed
if ! pkg-config --exists opencv4 && ! pkg-config --exists opencv; then
    echo "❌ OpenCV not found. Install it with:"
    echo "   brew install opencv"
    exit 1
fi

# Check if pybind11 is installed
if ! python3 -c "import pybind11" 2>/dev/null; then
    echo "📦 Installing pybind11..."
    pip install pybind11
fi

# Clean previous builds
echo "🧹 Cleaning previous builds..."
rm -rf build dist *.egg-info
rm -f fast_processor*.so

# Build the extension
echo "⚙️  Compiling..."
python3 setup.py build_ext --inplace

if [ $? -eq 0 ]; then
    echo "✅ C++ extension built successfully!"
    echo "🚀 The fast processor will be used automatically"
else
    echo "❌ Build failed. Using Python fallback."
    exit 1
fi
