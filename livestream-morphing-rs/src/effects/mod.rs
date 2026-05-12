// Image processing effects — shared types and utility functions

pub mod blur;
pub mod canvas_texture;
pub mod color_shift;
pub mod distortion;
pub mod edges;
pub mod levels;
pub mod pixelate;
pub mod quantize;
pub mod sharpen;
pub mod solarize;
pub mod threshold;
pub mod vignette;

pub mod chromatic_aberration;
pub mod denoise;
pub mod gradient_map;
pub mod halftone;
pub mod mirror;

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
