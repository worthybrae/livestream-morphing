#!/bin/bash

# Style Transfer Environment Setup Script

echo "========================================"
echo "Neural Style Transfer Setup"
echo "========================================"
echo ""

# Check if poetry is installed
if ! command -v poetry &> /dev/null; then
    echo "❌ Poetry not found. Please install poetry first:"
    echo "   curl -sSL https://install.python-poetry.org | python3 -"
    exit 1
fi

echo "✅ Poetry found"

# Navigate to project root
cd ..

echo ""
echo "Installing dependencies with poetry..."
poetry install

echo ""
echo "✅ Dependencies installed!"
echo ""
echo "========================================"
echo "Next Steps:"
echo "========================================"
echo ""
echo "1. Start Jupyter notebook:"
echo "   cd style-transfer/notebooks"
echo "   poetry run jupyter notebook"
echo ""
echo "2. Open: 01_setup_and_quicktest.ipynb"
echo ""
echo "3. Run all cells to test pre-trained model"
echo ""
echo "Expected results:"
echo "  - Inference time: 50-200ms per frame"
echo "  - Speedup: 30-120x faster than cv::stylization"
echo ""
echo "========================================"
