use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Two-pass box blur (horizontal then vertical).
#[derive(Default)]
pub struct Blur {
    scratch: Vec<u8>,
}

impl Effect for Blur {
    fn id(&self) -> &'static str {
        "blur"
    }

    fn name(&self) -> &'static str {
        "Box Blur"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("radius", "Radius", 0.0, 20.0, 3.0, 1.0)]
    }

    fn init(&mut self, width: u32, height: u32) {
        self.scratch = vec![0u8; (width * height * 3) as usize];
    }

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let radius = params.get("radius").copied().unwrap_or(3.0) as i32;
        if radius <= 0 {
            return;
        }

        let w = frame.width as i32;
        let h = frame.height as i32;

        // Ensure scratch buffer is large enough (in case init was not called).
        let needed = (w * h * 3) as usize;
        if self.scratch.len() < needed {
            self.scratch.resize(needed, 0);
        }

        // Horizontal pass: frame → scratch
        for y in 0..h {
            for x in 0..w {
                let mut sum = [0u32; 3];
                let mut count = 0u32;
                for dx in -radius..=radius {
                    let nx = (x + dx).clamp(0, w - 1);
                    let idx = ((y * w + nx) * 3) as usize;
                    sum[0] += frame.data[idx] as u32;
                    sum[1] += frame.data[idx + 1] as u32;
                    sum[2] += frame.data[idx + 2] as u32;
                    count += 1;
                }
                let dst = ((y * w + x) * 3) as usize;
                self.scratch[dst] = (sum[0] / count) as u8;
                self.scratch[dst + 1] = (sum[1] / count) as u8;
                self.scratch[dst + 2] = (sum[2] / count) as u8;
            }
        }

        // Vertical pass: scratch → frame
        for y in 0..h {
            for x in 0..w {
                let mut sum = [0u32; 3];
                let mut count = 0u32;
                for dy in -radius..=radius {
                    let ny = (y + dy).clamp(0, h - 1);
                    let idx = ((ny * w + x) * 3) as usize;
                    sum[0] += self.scratch[idx] as u32;
                    sum[1] += self.scratch[idx + 1] as u32;
                    sum[2] += self.scratch[idx + 2] as u32;
                    count += 1;
                }
                let dst = ((y * w + x) * 3) as usize;
                frame.data[dst] = (sum[0] / count) as u8;
                frame.data[dst + 1] = (sum[1] / count) as u8;
                frame.data[dst + 2] = (sum[2] / count) as u8;
            }
        }
    }
}

crate::register_effect!(Blur);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    /// A sharp black-to-white edge should be smoothed: the boundary pixel must
    /// land strictly between 0 and 255 after blurring.
    #[test]
    fn blur_smooths_sharp_edge() {
        let w = 10u32;
        let h = 1u32;
        let mut effect = Blur::default();
        effect.init(w, h);

        // Left half black, right half white.
        let mut frame = RawFrame::new(w, h);
        for x in 0..w {
            let v = if x < w / 2 { 0u8 } else { 255u8 };
            let idx = (x * 3) as usize;
            frame.data[idx] = v;
            frame.data[idx + 1] = v;
            frame.data[idx + 2] = v;
        }

        let params = default_params(&effect.params()); // radius = 3
        let ctx = FrameCtx { frame_number: 0, width: w, height: h };
        effect.apply(&mut frame, &params, &ctx);

        // The boundary pixel (x = w/2 - 1 = 4) should be between 0 and 255.
        let boundary_idx = ((w / 2 - 1) * 3) as usize;
        let val = frame.data[boundary_idx];
        assert!(val > 0 && val < 255, "boundary pixel {} should be between 0 and 255", val);
    }

    #[test]
    fn blur_zero_radius_is_identity() {
        let mut effect = Blur::default();
        effect.init(4, 4);
        let mut frame = RawFrame::filled(4, 4, 100, 150, 200);
        let original = frame.data.clone();
        let mut params = default_params(&effect.params());
        params.insert("radius".into(), 0.0);
        let ctx = FrameCtx { frame_number: 0, width: 4, height: 4 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original);
    }
}
