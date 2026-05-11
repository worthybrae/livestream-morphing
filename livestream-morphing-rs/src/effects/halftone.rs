use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Halftone — simulates a halftone printing effect with variable-size dots
/// whose radius is proportional to the average brightness of each cell.
#[derive(Default)]
pub struct Halftone;

impl Effect for Halftone {
    fn id(&self) -> &'static str {
        "halftone"
    }

    fn name(&self) -> &'static str {
        "Halftone"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("dot_size", "Dot Size", 3.0, 20.0, 6.0, 1.0)]
    }

    fn init(&mut self, _width: u32, _height: u32) {}

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let dot_size = params.get("dot_size").copied().unwrap_or(6.0).max(3.0) as usize;
        let w = frame.width as usize;
        let h = frame.height as usize;
        let half = dot_size as f32 / 2.0;

        let mut by = 0;
        while by < h {
            let mut bx = 0;
            while bx < w {
                let bw = (bx + dot_size).min(w) - bx;
                let bh = (by + dot_size).min(h) - by;
                let count = (bw * bh) as u32;

                // Compute average brightness for this cell
                let mut sum: u32 = 0;
                for dy in 0..bh {
                    for dx in 0..bw {
                        let idx = ((by + dy) * w + (bx + dx)) * 3;
                        let r = frame.data[idx] as u32;
                        let g = frame.data[idx + 1] as u32;
                        let b = frame.data[idx + 2] as u32;
                        sum += (r + g + b) / 3;
                    }
                }
                let brightness = sum as f32 / count as f32;
                let radius = half * (brightness / 255.0);

                // Cell center relative to the cell origin
                let cx = bw as f32 / 2.0;
                let cy = bh as f32 / 2.0;

                // Set pixels based on distance from cell center
                for dy in 0..bh {
                    for dx in 0..bw {
                        let dist_x = dx as f32 + 0.5 - cx;
                        let dist_y = dy as f32 + 0.5 - cy;
                        let dist = (dist_x * dist_x + dist_y * dist_y).sqrt();
                        let idx = ((by + dy) * w + (bx + dx)) * 3;
                        if dist <= radius {
                            frame.data[idx] = 255;
                            frame.data[idx + 1] = 255;
                            frame.data[idx + 2] = 255;
                        } else {
                            frame.data[idx] = 0;
                            frame.data[idx + 1] = 0;
                            frame.data[idx + 2] = 0;
                        }
                    }
                }
                bx += dot_size;
            }
            by += dot_size;
        }
    }
}

crate::register_effect!(Halftone);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn halftone_black_image_all_black() {
        let mut effect = Halftone::default();
        let mut frame = RawFrame::filled(6, 6, 0, 0, 0);
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 6, height: 6 };
        effect.apply(&mut frame, &params, &ctx);
        // Brightness 0 -> radius 0 -> all pixels black
        for &b in &frame.data {
            assert_eq!(b, 0);
        }
    }

    #[test]
    fn halftone_white_image_has_white_center() {
        let mut effect = Halftone::default();
        let mut frame = RawFrame::filled(6, 6, 255, 255, 255);
        let params = default_params(&effect.params()); // dot_size=6
        let ctx = FrameCtx { frame_number: 0, width: 6, height: 6 };
        effect.apply(&mut frame, &params, &ctx);
        // Center pixel of 6x6 cell (pixel 3,3) should be white
        let center_idx = (3 * 6 + 3) * 3;
        assert_eq!(frame.data[center_idx], 255);
        assert_eq!(frame.data[center_idx + 1], 255);
        assert_eq!(frame.data[center_idx + 2], 255);
    }

    #[test]
    fn halftone_output_is_binary() {
        let mut effect = Halftone::default();
        let mut frame = RawFrame::filled(6, 6, 128, 128, 128);
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 6, height: 6 };
        effect.apply(&mut frame, &params, &ctx);
        // Every pixel should be either 0 or 255
        for &b in &frame.data {
            assert!(b == 0 || b == 255, "expected 0 or 255, got {}", b);
        }
    }
}
