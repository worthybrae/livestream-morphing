// Image processing effects pipeline

use std::f32::consts::PI;

/// RGB24 pixel buffer.
#[derive(Clone)]
pub struct RawFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl RawFrame {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            data: vec![0u8; (width * height * 3) as usize],
            width,
            height,
        }
    }

    pub fn filled(width: u32, height: u32, r: u8, g: u8, b: u8) -> Self {
        let mut frame = Self::new(width, height);
        for pixel in frame.data.chunks_exact_mut(3) {
            pixel[0] = r;
            pixel[1] = g;
            pixel[2] = b;
        }
        frame
    }
}

/// Snap each color channel to N discrete levels.
pub fn quantize(frame: &mut RawFrame, levels: u8) {
    let step = 255.0 / (levels - 1) as f32;
    for byte in frame.data.iter_mut() {
        let val = *byte as f32;
        *byte = ((val / step).round() * step).clamp(0.0, 255.0) as u8;
    }
}

/// Sine-wave coordinate remapping for psychedelic melting effect.
/// Writes from `src` into `dst` with bilinear interpolation.
pub fn apply_distortion(
    src: &RawFrame,
    dst: &mut RawFrame,
    frame_number: u32,
    amplitude: f32,
    frequency: f32,
    cycle_length: u32,
) {
    let w = src.width;
    let h = src.height;
    let wf = w as f32;
    let hf = h as f32;
    let time = (frame_number % cycle_length) as f32 * (2.0 * PI / cycle_length as f32);

    for y in 0..h {
        let y_offset = (time + y as f32 * frequency / hf).sin() * hf * amplitude;
        for x in 0..w {
            let x_offset = (time + x as f32 * frequency / wf).sin() * wf * amplitude;

            let src_x = (x as f32 + x_offset).clamp(0.0, wf - 1.0);
            let src_y = (y as f32 + y_offset).clamp(0.0, hf - 1.0);

            // Bilinear interpolation
            let x0 = src_x.floor() as u32;
            let y0 = src_y.floor() as u32;
            let x1 = (x0 + 1).min(w - 1);
            let y1 = (y0 + 1).min(h - 1);
            let fx = src_x.fract();
            let fy = src_y.fract();

            let dst_idx = ((y * w + x) * 3) as usize;
            for c in 0..3 {
                let p00 = src.data[((y0 * w + x0) * 3) as usize + c] as f32;
                let p10 = src.data[((y0 * w + x1) * 3) as usize + c] as f32;
                let p01 = src.data[((y1 * w + x0) * 3) as usize + c] as f32;
                let p11 = src.data[((y1 * w + x1) * 3) as usize + c] as f32;
                let val = p00 * (1.0 - fx) * (1.0 - fy)
                    + p10 * fx * (1.0 - fy)
                    + p01 * (1.0 - fx) * fy
                    + p11 * fx * fy;
                dst.data[dst_idx + c] = val.clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// Sobel edge detection + dark overlay on the frame.
/// `gray` and `edges` are pre-allocated scratch buffers (width * height each).
pub fn detect_and_overlay_edges(
    frame: &mut RawFrame,
    gray: &mut [u8],
    edges: &mut [u8],
    width: u32,
    height: u32,
    threshold: u8,
    darkness: u8,
) {
    let w = width as usize;
    let h = height as usize;

    // RGB → grayscale (BT.601 weights)
    for i in 0..(w * h) {
        let r = frame.data[i * 3] as u16;
        let g = frame.data[i * 3 + 1] as u16;
        let b = frame.data[i * 3 + 2] as u16;
        gray[i] = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
    }

    // Clear edges
    edges.iter_mut().for_each(|e| *e = 0);

    // Sobel (skip border pixels)
    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let g = |dy: i32, dx: i32| -> i16 {
                gray[((y as i32 + dy) as usize) * w + (x as i32 + dx) as usize] as i16
            };
            let gx = -g(-1, -1) + g(-1, 1) - 2 * g(0, -1) + 2 * g(0, 1) - g(1, -1) + g(1, 1);
            let gy = -g(-1, -1) - 2 * g(-1, 0) - g(-1, 1) + g(1, -1) + 2 * g(1, 0) + g(1, 1);
            let mag = ((gx.unsigned_abs() + gy.unsigned_abs()) / 2).min(255) as u8;
            edges[y * w + x] = if mag > threshold { 255 } else { 0 };
        }
    }

    // Overlay dark edges
    for i in 0..(w * h) {
        if edges[i] > 0 {
            frame.data[i * 3] = frame.data[i * 3].saturating_sub(darkness);
            frame.data[i * 3 + 1] = frame.data[i * 3 + 1].saturating_sub(darkness);
            frame.data[i * 3 + 2] = frame.data[i * 3 + 2].saturating_sub(darkness);
        }
    }
}

/// Multiply-blend a grayscale texture over the frame.
pub fn blend_texture(frame: &mut RawFrame, texture: &[u8], strength: f32) {
    let pixel_count = (frame.width * frame.height) as usize;
    for i in 0..pixel_count {
        let tex = texture[i % texture.len()] as f32 / 255.0;
        let factor = 1.0 - strength + strength * tex;
        for c in 0..3 {
            let idx = i * 3 + c;
            frame.data[idx] = (frame.data[idx] as f32 * factor).clamp(0.0, 255.0) as u8;
        }
    }
}

/// Generate a deterministic canvas-weave texture (grayscale).
pub fn generate_canvas_texture(width: u32, height: u32) -> Vec<u8> {
    let mut texture = vec![0u8; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let h = x.wrapping_mul(374761393)
                .wrapping_add(y.wrapping_mul(668265263))
                .wrapping_mul(1274126177);
            let noise = ((h >> 24) & 0x1F) as u8;
            let weave: u8 = if (x % 4 < 2) ^ (y % 4 < 2) { 10 } else { 0 };
            texture[(y * width + x) as usize] = 200u8.wrapping_add(noise).wrapping_add(weave);
        }
    }
    texture
}

/// Downsample a frame by 2x using nearest-neighbor sampling.
pub fn downsample_2x(src: &RawFrame) -> RawFrame {
    let dw = src.width / 2;
    let dh = src.height / 2;
    let mut dst = RawFrame::new(dw, dh);
    for y in 0..dh {
        for x in 0..dw {
            let si = ((y * 2 * src.width + x * 2) * 3) as usize;
            let di = ((y * dw + x) * 3) as usize;
            dst.data[di..di + 3].copy_from_slice(&src.data[si..si + 3]);
        }
    }
    dst
}

/// Upsample a frame by 2x using nearest-neighbor (preserves hard painterly edges).
pub fn upsample_2x(src: &RawFrame, dst_w: u32, dst_h: u32) -> RawFrame {
    let mut dst = RawFrame::new(dst_w, dst_h);
    for y in 0..dst_h {
        for x in 0..dst_w {
            let sx = (x / 2).min(src.width - 1);
            let sy = (y / 2).min(src.height - 1);
            let si = ((sy * src.width + sx) * 3) as usize;
            let di = ((y * dst_w + x) * 3) as usize;
            dst.data[di..di + 3].copy_from_slice(&src.data[si..si + 3]);
        }
    }
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_snaps_to_levels() {
        let mut frame = RawFrame::new(2, 1);
        frame.data = vec![50, 50, 50, 200, 200, 200];
        quantize(&mut frame, 2);
        assert_eq!(frame.data, vec![0, 0, 0, 255, 255, 255]);
    }

    #[test]
    fn quantize_with_more_levels() {
        let mut frame = RawFrame::new(1, 1);
        frame.data = vec![100, 100, 100];
        quantize(&mut frame, 4);
        assert_eq!(frame.data, vec![85, 85, 85]);
    }

    #[test]
    fn distortion_zero_amplitude_is_identity() {
        let src = RawFrame::filled(4, 4, 128, 64, 32);
        let mut dst = RawFrame::new(4, 4);
        apply_distortion(&src, &mut dst, 0, 0.0, 12.0, 180);
        assert_eq!(src.data, dst.data);
    }

    #[test]
    fn distortion_nonzero_amplitude_changes_pixels() {
        let mut src = RawFrame::new(8, 8);
        for y in 0..8u32 {
            for x in 0..8u32 {
                let idx = ((y * 8 + x) * 3) as usize;
                src.data[idx] = (x * 32) as u8;
                src.data[idx + 1] = (y * 32) as u8;
                src.data[idx + 2] = 0;
            }
        }
        let mut dst = RawFrame::new(8, 8);
        apply_distortion(&src, &mut dst, 10, 0.05, 12.0, 180);
        assert_ne!(src.data, dst.data);
    }

    #[test]
    fn edges_detected_at_sharp_boundary() {
        let mut frame = RawFrame::new(8, 4);
        for y in 0..4u32 {
            for x in 0..8u32 {
                let idx = ((y * 8 + x) * 3) as usize;
                let val = if x < 4 { 255 } else { 0 };
                frame.data[idx] = val;
                frame.data[idx + 1] = val;
                frame.data[idx + 2] = val;
            }
        }
        let original = frame.data.clone();
        let mut gray = vec![0u8; 32];
        let mut edges = vec![0u8; 32];
        detect_and_overlay_edges(&mut frame, &mut gray, &mut edges, 8, 4, 30, 80);
        let mid_pixel = frame.data[((1 * 8 + 4) * 3) as usize];
        assert!(mid_pixel < original[((1 * 8 + 4) * 3) as usize], "Edge pixel should be darkened");
    }

    #[test]
    fn no_edges_on_uniform_frame() {
        let mut frame = RawFrame::filled(8, 8, 128, 128, 128);
        let original = frame.data.clone();
        let mut gray = vec![0u8; 64];
        let mut edges = vec![0u8; 64];
        detect_and_overlay_edges(&mut frame, &mut gray, &mut edges, 8, 8, 30, 80);
        assert_eq!(frame.data, original, "Uniform frame should have no edges");
    }

    #[test]
    fn texture_blend_darkens_pixels() {
        let mut frame = RawFrame::filled(4, 4, 200, 200, 200);
        let texture = vec![128u8; 16];
        blend_texture(&mut frame, &texture, 0.5);
        assert!(frame.data[0] < 200, "Should be darkened, got {}", frame.data[0]);
        assert!(frame.data[0] > 100, "Shouldn't be too dark, got {}", frame.data[0]);
    }

    #[test]
    fn texture_blend_zero_strength_is_identity() {
        let mut frame = RawFrame::filled(2, 2, 100, 100, 100);
        let texture = vec![0u8; 4];
        let original = frame.data.clone();
        blend_texture(&mut frame, &texture, 0.0);
        assert_eq!(frame.data, original);
    }

    #[test]
    fn downsample_halves_dimensions() {
        let src = RawFrame::filled(8, 6, 100, 100, 100);
        let dst = downsample_2x(&src);
        assert_eq!(dst.width, 4);
        assert_eq!(dst.height, 3);
        assert_eq!(dst.data.len(), (4 * 3 * 3) as usize);
    }

    #[test]
    fn upsample_doubles_dimensions() {
        let src = RawFrame::filled(4, 3, 50, 50, 50);
        let dst = upsample_2x(&src, 8, 6);
        assert_eq!(dst.width, 8);
        assert_eq!(dst.height, 6);
        assert_eq!(dst.data[0], 50);
    }
}
