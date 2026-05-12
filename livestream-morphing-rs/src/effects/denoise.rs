use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Denoise — edge-preserving noise reduction using a bilateral-like filter.
/// Averages a 5x5 neighborhood weighted by color similarity.
#[derive(Default)]
pub struct Denoise;

impl Effect for Denoise {
    fn id(&self) -> &'static str {
        "denoise"
    }

    fn name(&self) -> &'static str {
        "Denoise"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("strength", "Strength", 1.0, 20.0, 5.0, 1.0)]
    }

    fn init(&mut self, _width: u32, _height: u32) {}

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let strength = params.get("strength").copied().unwrap_or(5.0).max(1.0);
        let threshold = (strength * 3.0) as i32;
        let w = frame.width as i32;
        let h = frame.height as i32;
        let original = frame.data.clone();

        for y in 0..h {
            for x in 0..w {
                let center_idx = (y * w + x) as usize * 3;
                let cr = original[center_idx] as i32;
                let cg = original[center_idx + 1] as i32;
                let cb = original[center_idx + 2] as i32;

                let mut sum_r: i32 = 0;
                let mut sum_g: i32 = 0;
                let mut sum_b: i32 = 0;
                let mut count: i32 = 0;

                for dy in -2..=2 {
                    let ny = y + dy;
                    if ny < 0 || ny >= h {
                        continue;
                    }
                    for dx in -2..=2 {
                        let nx = x + dx;
                        if nx < 0 || nx >= w {
                            continue;
                        }
                        let n_idx = (ny * w + nx) as usize * 3;
                        let nr = original[n_idx] as i32;
                        let ng = original[n_idx + 1] as i32;
                        let nb = original[n_idx + 2] as i32;

                        let dist = (cr - nr).abs() + (cg - ng).abs() + (cb - nb).abs();
                        if dist < threshold {
                            sum_r += nr;
                            sum_g += ng;
                            sum_b += nb;
                            count += 1;
                        }
                    }
                }

                if count > 0 {
                    frame.data[center_idx] = (sum_r / count) as u8;
                    frame.data[center_idx + 1] = (sum_g / count) as u8;
                    frame.data[center_idx + 2] = (sum_b / count) as u8;
                }
            }
        }
    }
}

crate::register_effect!(Denoise);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn denoise_uniform_unchanged() {
        let mut effect = Denoise::default();
        let mut frame = RawFrame::filled(4, 4, 100, 100, 100);
        let original = frame.data.clone();
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 4, height: 4 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original);
    }

    #[test]
    fn denoise_smooths_noise() {
        let mut effect = Denoise::default();
        // 3x3 uniform image with one slightly noisy pixel
        let mut frame = RawFrame::filled(3, 3, 100, 100, 100);
        // Add a small amount of noise to center pixel — total color distance
        // is 3*4 = 12 which is < threshold 15, so neighbors will be included
        let center = (1 * 3 + 1) * 3;
        frame.data[center] = 104;
        frame.data[center + 1] = 104;
        frame.data[center + 2] = 104;
        let params = default_params(&effect.params()); // strength=5, threshold=15
        let ctx = FrameCtx { frame_number: 0, width: 3, height: 3 };
        effect.apply(&mut frame, &params, &ctx);
        // Center should be averaged toward 100 (noise reduced)
        assert!(frame.data[center] < 104, "noise should be reduced, got {}", frame.data[center]);
    }

    #[test]
    fn denoise_preserves_edges() {
        let mut effect = Denoise::default();
        // 3x3 image: left column = 0, rest = 200 — strong edge
        let mut frame = RawFrame::new(3, 3);
        for y in 0..3 {
            for x in 0..3 {
                let idx = (y * 3 + x) * 3;
                let val: u8 = if x == 0 { 0 } else { 200 };
                frame.data[idx] = val;
                frame.data[idx + 1] = val;
                frame.data[idx + 2] = val;
            }
        }
        let mut params = default_params(&effect.params());
        params.insert("strength".into(), 5.0); // threshold=15, edge dist=600 >> 15
        let ctx = FrameCtx { frame_number: 0, width: 3, height: 3 };
        effect.apply(&mut frame, &params, &ctx);
        // The dark column should stay dark (edge preserved)
        assert_eq!(frame.data[0], 0);
        // The bright side should stay bright
        let bright_idx = (0 * 3 + 1) * 3;
        assert_eq!(frame.data[bright_idx], 200);
    }
}
