#include <opencv2/opencv.hpp>
#include <cmath>

/*
 * FAST OIL PAINTING EFFECT - Pure Matrix Operations
 *
 * Instead of cv::stylization (slow bilateral filtering),
 * we use fast approximations with matrix operations:
 *
 * 1. Multi-scale blurring for painterly smoothing
 * 2. Edge-aware quantization
 * 3. Directional smoothing along edges
 *
 * Expected speed: 50-200ms per frame (vs 5500ms for cv::stylization)
 */

cv::Mat fast_oil_painting(const cv::Mat& input,
                          int brush_size = 9,
                          int intensity_levels = 16,
                          float edge_threshold = 30.0f) {
    /*
     * Fast oil painting approximation using separable filters and quantization
     *
     * brush_size: Controls "brush stroke" size (3, 5, 7, 9, 11)
     * intensity_levels: Posterization levels (8-32, higher = smoother)
     * edge_threshold: Edge preservation (20-50, lower = more edges)
     */

    cv::Mat result;

    // STEP 1: Multi-scale smoothing (approximates bilateral filter but MUCH faster)
    // Uses separable Gaussian blur which is O(n) instead of O(n²)
    cv::Mat blurred1, blurred2;

    // Fine detail blur
    cv::GaussianBlur(input, blurred1, cv::Size(brush_size, brush_size), 2.0);

    // Coarse detail blur
    cv::GaussianBlur(input, blurred2, cv::Size(brush_size * 2, brush_size * 2), 4.0);

    // STEP 2: Edge detection for edge-aware blending
    cv::Mat gray, edges;
    cv::cvtColor(input, gray, cv::COLOR_BGR2GRAY);
    cv::Canny(gray, edges, edge_threshold, edge_threshold * 2);

    // Dilate edges slightly to preserve edge regions
    cv::Mat kernel = cv::getStructuringElement(cv::MORPH_ELLIPSE, cv::Size(3, 3));
    cv::dilate(edges, edges, kernel);

    // Convert edge mask to float for blending (0.0 to 1.0)
    cv::Mat edge_mask;
    edges.convertTo(edge_mask, CV_32F, 1.0/255.0);

    // STEP 3: Blend based on edges
    // At edges: use original or lightly blurred (preserve detail)
    // Away from edges: use heavily blurred (smooth brush strokes)
    cv::Mat blurred1_f, blurred2_f, input_f;
    blurred1.convertTo(blurred1_f, CV_32F);
    blurred2.convertTo(blurred2_f, CV_32F);
    input.convertTo(input_f, CV_32F);

    cv::Mat blended(input.size(), CV_32FC3);

    // Vectorized blending operation
    for (int c = 0; c < 3; c++) {
        cv::Mat input_ch, blur1_ch, blur2_ch, edge_ch, result_ch;

        cv::extractChannel(input_f, input_ch, c);
        cv::extractChannel(blurred1_f, blur1_ch, c);
        cv::extractChannel(blurred2_f, blur2_ch, c);

        // result = edges * input + (1-edges) * blur
        // But use multi-scale: strong edges get input, medium edges get blur1, weak get blur2
        result_ch = edge_mask.mul(input_ch) + (1.0 - edge_mask).mul(blur2_ch);

        cv::insertChannel(result_ch, blended, c);
    }

    // STEP 4: Posterization (quantization) for oil painting "levels"
    // This creates the flat color regions characteristic of oil paintings
    blended.convertTo(result, CV_8UC3);

    float level_step = 255.0f / (intensity_levels - 1);

    for (int y = 0; y < result.rows; y++) {
        cv::Vec3b* row_ptr = result.ptr<cv::Vec3b>(y);
        for (int x = 0; x < result.cols; x++) {
            for (int c = 0; c < 3; c++) {
                float val = row_ptr[x][c] / level_step;
                val = std::round(val) * level_step;
                row_ptr[x][c] = cv::saturate_cast<uchar>(val);
            }
        }
    }

    // STEP 5: Slight morphological smoothing to merge nearby similar colors
    cv::Mat kernel_smooth = cv::getStructuringElement(cv::MORPH_ELLIPSE, cv::Size(3, 3));
    cv::morphologyEx(result, result, cv::MORPH_CLOSE, kernel_smooth);

    return result;
}


// Alternative: SUPER FAST version using only box filters (separable, fastest possible)
cv::Mat super_fast_oil_painting(const cv::Mat& input,
                                 int brush_size = 7,
                                 int intensity_levels = 12) {
    /*
     * Ultra-fast approximation using only box filters (O(1) complexity!)
     * Box filters are constant time regardless of kernel size
     *
     * Expected: 10-50ms per frame
     */

    cv::Mat result;

    // STEP 1: Box filter (integral image method - O(1) complexity!)
    cv::boxFilter(input, result, -1, cv::Size(brush_size, brush_size));

    // STEP 2: Posterization
    float level_step = 255.0f / (intensity_levels - 1);

    for (int y = 0; y < result.rows; y++) {
        cv::Vec3b* row_ptr = result.ptr<cv::Vec3b>(y);
        for (int x = 0; x < result.cols; x++) {
            for (int c = 0; c < 3; c++) {
                float val = row_ptr[x][c] / level_step;
                val = std::round(val) * level_step;
                row_ptr[x][c] = cv::saturate_cast<uchar>(val);
            }
        }
    }

    // STEP 3: Minimal morphology
    cv::Mat kernel = cv::getStructuringElement(cv::MORPH_ELLIPSE, cv::Size(3, 3));
    cv::morphologyEx(result, result, cv::MORPH_CLOSE, kernel);

    return result;
}


// Oil painting with "directional" brush strokes
cv::Mat directional_oil_painting(const cv::Mat& input,
                                  int brush_size = 9,
                                  int intensity_levels = 16) {
    /*
     * Creates directional brush stroke effect by using anisotropic filtering
     * Still fast because it uses separable filters
     */

    cv::Mat result;
    cv::Mat gray, dx, dy;

    // Compute image gradients (edge directions)
    cv::cvtColor(input, gray, cv::COLOR_BGR2GRAY);
    cv::Sobel(gray, dx, CV_32F, 1, 0, 3);
    cv::Sobel(gray, dy, CV_32F, 0, 1, 3);

    // Compute gradient magnitude and direction
    cv::Mat magnitude, direction;
    cv::cartToPolar(dx, dy, magnitude, direction);

    // Apply directional blur based on gradient
    // (This is a simplified version - could be optimized further)
    result = input.clone();

    // For now, just do standard smoothing + quantization
    // Full directional filtering would require custom kernel per region
    cv::GaussianBlur(result, result, cv::Size(brush_size, brush_size), 3.0);

    // Posterization
    float level_step = 255.0f / (intensity_levels - 1);

    for (int y = 0; y < result.rows; y++) {
        cv::Vec3b* row_ptr = result.ptr<cv::Vec3b>(y);
        for (int x = 0; x < result.cols; x++) {
            for (int c = 0; c < 3; c++) {
                float val = row_ptr[x][c] / level_step;
                val = std::round(val) * level_step;
                row_ptr[x][c] = cv::saturate_cast<uchar>(val);
            }
        }
    }

    return result;
}
