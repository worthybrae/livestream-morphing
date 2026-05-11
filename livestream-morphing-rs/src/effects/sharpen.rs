use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Sharpen (Unsharp Mask) — enhances edges by amplifying the difference between
/// each pixel and the average of its 4-connected neighbors.
#[derive(Default)]
pub struct Sharpen;

impl Effect for Sharpen {
    fn id(&self) -> &'static str {
        "sharpen"
    }

    fn name(&self) -> &'static str {
        "Sharpen"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("amount", "Amount", 0.0, 5.0, 1.0, 0.1)]
    }

    fn init(&mut self, _width: u32, _height: u32) {}

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let amount = params.get("amount").copied().unwrap_or(1.0);
        if amount <= 0.0 {
            return;
        }
        let w = frame.width as usize;
        let h = frame.height as usize;
        let original = frame.data.clone();

        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 3;
                for c in 0..3 {
                    let center = original[idx + c] as f32;
                    // Average of 4 neighbors (clamp at edges)
                    let top = if y > 0 { original[((y - 1) * w + x) * 3 + c] } else { original[idx + c] } as f32;
                    let bot = if y + 1 < h { original[((y + 1) * w + x) * 3 + c] } else { original[idx + c] } as f32;
                    let left = if x > 0 { original[(y * w + x - 1) * 3 + c] } else { original[idx + c] } as f32;
                    let right = if x + 1 < w { original[(y * w + x + 1) * 3 + c] } else { original[idx + c] } as f32;
                    let avg = (top + bot + left + right) / 4.0;
                    let sharpened = center + amount * (center - avg);
                    frame.data[idx + c] = sharpened.clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}

crate::register_effect!(Sharpen);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn sharpen_uniform_image_unchanged() {
        let mut effect = Sharpen::default();
        let mut frame = RawFrame::filled(4, 4, 100, 100, 100);
        let original = frame.data.clone();
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 4, height: 4 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original);
    }

    #[test]
    fn sharpen_amplifies_edge() {
        let mut effect = Sharpen::default();
        // 3x1 image: dark-bright-dark
        let mut frame = RawFrame::new(3, 1);
        frame.data = vec![50, 50, 50, 200, 200, 200, 50, 50, 50];
        let mut params = default_params(&effect.params());
        params.insert("amount".into(), 1.0);
        let ctx = FrameCtx { frame_number: 0, width: 3, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        // Center pixel should become brighter (pushed away from neighbors)
        assert!(frame.data[3] > 200, "center should be amplified, got {}", frame.data[3]);
    }

    #[test]
    fn sharpen_zero_amount_is_noop() {
        let mut effect = Sharpen::default();
        let mut frame = RawFrame::new(2, 2);
        frame.data = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120];
        let original = frame.data.clone();
        let mut params = default_params(&effect.params());
        params.insert("amount".into(), 0.0);
        let ctx = FrameCtx { frame_number: 0, width: 2, height: 2 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original);
    }
}
