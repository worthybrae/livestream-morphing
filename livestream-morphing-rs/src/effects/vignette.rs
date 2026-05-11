use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Vignette — darkens pixel values toward the edges/corners of the frame.
#[derive(Default)]
pub struct Vignette;

impl Effect for Vignette {
    fn id(&self) -> &'static str {
        "vignette"
    }

    fn name(&self) -> &'static str {
        "Vignette"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef::new("strength", "Strength", 0.0, 1.0, 0.5, 0.05),
            ParamDef::new("radius", "Radius", 0.5, 2.0, 1.2, 0.05),
        ]
    }

    fn init(&mut self, _width: u32, _height: u32) {}

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let strength = params.get("strength").copied().unwrap_or(0.5);
        let radius = params.get("radius").copied().unwrap_or(1.2).max(0.01);
        let w = frame.width as f32;
        let h = frame.height as f32;
        let cx = w / 2.0;
        let cy = h / 2.0;
        // Max distance is from center to corner
        let max_dist = (cx * cx + cy * cy).sqrt();

        for y in 0..frame.height {
            for x in 0..frame.width {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                let dist = (dx * dx + dy * dy).sqrt() / max_dist;
                let ratio = dist / radius;
                let factor = (1.0 - strength * (ratio * ratio).max(0.0)).max(0.0);
                let idx = ((y * frame.width + x) * 3) as usize;
                frame.data[idx] = (frame.data[idx] as f32 * factor).clamp(0.0, 255.0) as u8;
                frame.data[idx + 1] = (frame.data[idx + 1] as f32 * factor).clamp(0.0, 255.0) as u8;
                frame.data[idx + 2] = (frame.data[idx + 2] as f32 * factor).clamp(0.0, 255.0) as u8;
            }
        }
    }
}

crate::register_effect!(Vignette);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn vignette_center_unchanged() {
        let mut effect = Vignette::default();
        // Use an odd-sized frame so there's a clear center pixel
        let mut frame = RawFrame::filled(3, 3, 200, 200, 200);
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 3, height: 3 };
        effect.apply(&mut frame, &params, &ctx);
        // Center pixel (1,1) should be close to original
        let center_idx = (1 * 3 + 1) * 3;
        assert!(frame.data[center_idx] >= 195, "center should stay bright, got {}", frame.data[center_idx]);
    }

    #[test]
    fn vignette_corners_darker() {
        let mut effect = Vignette::default();
        let mut frame = RawFrame::filled(10, 10, 200, 200, 200);
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 10, height: 10 };
        effect.apply(&mut frame, &params, &ctx);
        // Corner pixel (0,0) should be darker than center
        let corner = frame.data[0];
        let center_idx = (5 * 10 + 5) * 3;
        let center = frame.data[center_idx];
        assert!(corner < center, "corner ({}) should be darker than center ({})", corner, center);
    }

    #[test]
    fn vignette_zero_strength_is_noop() {
        let mut effect = Vignette::default();
        let mut frame = RawFrame::filled(4, 4, 128, 128, 128);
        let original = frame.data.clone();
        let mut params = default_params(&effect.params());
        params.insert("strength".into(), 0.0);
        let ctx = FrameCtx { frame_number: 0, width: 4, height: 4 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original);
    }
}
