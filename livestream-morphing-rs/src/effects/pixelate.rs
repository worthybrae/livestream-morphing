use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Pixelate — divides the frame into blocks and fills each with the average color.
#[derive(Default)]
pub struct Pixelate;

impl Effect for Pixelate {
    fn id(&self) -> &'static str {
        "pixelate"
    }

    fn name(&self) -> &'static str {
        "Pixelate"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("block_size", "Block Size", 2.0, 32.0, 8.0, 1.0)]
    }

    fn init(&mut self, _width: u32, _height: u32) {}

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let block = params.get("block_size").copied().unwrap_or(8.0).max(2.0) as usize;
        let w = frame.width as usize;
        let h = frame.height as usize;

        let mut by = 0;
        while by < h {
            let mut bx = 0;
            while bx < w {
                let bw = (bx + block).min(w) - bx;
                let bh = (by + block).min(h) - by;
                let count = (bw * bh) as u32;

                // Compute average color for this block
                let mut sum_r: u32 = 0;
                let mut sum_g: u32 = 0;
                let mut sum_b: u32 = 0;
                for dy in 0..bh {
                    for dx in 0..bw {
                        let idx = ((by + dy) * w + (bx + dx)) * 3;
                        sum_r += frame.data[idx] as u32;
                        sum_g += frame.data[idx + 1] as u32;
                        sum_b += frame.data[idx + 2] as u32;
                    }
                }
                let avg_r = (sum_r / count) as u8;
                let avg_g = (sum_g / count) as u8;
                let avg_b = (sum_b / count) as u8;

                // Fill block with average
                for dy in 0..bh {
                    for dx in 0..bw {
                        let idx = ((by + dy) * w + (bx + dx)) * 3;
                        frame.data[idx] = avg_r;
                        frame.data[idx + 1] = avg_g;
                        frame.data[idx + 2] = avg_b;
                    }
                }
                bx += block;
            }
            by += block;
        }
    }
}

crate::register_effect!(Pixelate);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn pixelate_uniform_unchanged() {
        let mut effect = Pixelate::default();
        let mut frame = RawFrame::filled(8, 8, 100, 150, 200);
        let original = frame.data.clone();
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 8, height: 8 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original);
    }

    #[test]
    fn pixelate_averages_block() {
        let mut effect = Pixelate::default();
        // 2x2 image with different pixels
        let mut frame = RawFrame::new(2, 2);
        frame.data = vec![
            0, 0, 0,     200, 200, 200,
            100, 100, 100, 100, 100, 100,
        ];
        let mut params = default_params(&effect.params());
        params.insert("block_size".into(), 2.0);
        let ctx = FrameCtx { frame_number: 0, width: 2, height: 2 };
        effect.apply(&mut frame, &params, &ctx);
        // Average: (0+200+100+100)/4 = 100
        assert_eq!(frame.data, vec![100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100]);
    }

    #[test]
    fn pixelate_non_divisible_size() {
        let mut effect = Pixelate::default();
        // 3x3 image with block_size 2 — last column and row are partial blocks
        let mut frame = RawFrame::filled(3, 3, 60, 60, 60);
        let mut params = default_params(&effect.params());
        params.insert("block_size".into(), 2.0);
        let ctx = FrameCtx { frame_number: 0, width: 3, height: 3 };
        effect.apply(&mut frame, &params, &ctx);
        // Uniform input, so output should still be uniform
        for &b in &frame.data {
            assert_eq!(b, 60);
        }
    }
}
