use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Levels / Contrast adjustment — remaps channel values from [black_point, white_point] to [0, 255]
/// and applies a gamma curve.
#[derive(Default)]
pub struct Levels;

impl Effect for Levels {
    fn id(&self) -> &'static str {
        "levels"
    }

    fn name(&self) -> &'static str {
        "Levels"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef::new("black_point", "Black Point", 0.0, 128.0, 0.0, 1.0),
            ParamDef::new("white_point", "White Point", 128.0, 255.0, 255.0, 1.0),
            ParamDef::new("gamma", "Gamma", 0.1, 3.0, 1.0, 0.05),
        ]
    }

    fn init(&mut self, _width: u32, _height: u32) {}

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let bp = params.get("black_point").copied().unwrap_or(0.0);
        let wp = params.get("white_point").copied().unwrap_or(255.0);
        let gamma = params.get("gamma").copied().unwrap_or(1.0).max(0.1);
        let inv_gamma = 1.0 / gamma;
        let range = (wp - bp).max(1.0);

        for byte in frame.data.iter_mut() {
            let val = *byte as f32;
            // Remap [black_point, white_point] -> [0, 1]
            let normalized = ((val - bp) / range).clamp(0.0, 1.0);
            // Apply gamma correction
            let corrected = normalized.powf(inv_gamma);
            *byte = (corrected * 255.0).clamp(0.0, 255.0) as u8;
        }
    }
}

crate::register_effect!(Levels);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn levels_identity_with_defaults() {
        let mut effect = Levels::default();
        let mut frame = RawFrame::new(1, 1);
        frame.data = vec![100, 150, 200];
        let original = frame.data.clone();
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original);
    }

    #[test]
    fn levels_crushes_blacks() {
        let mut effect = Levels::default();
        let mut frame = RawFrame::new(1, 1);
        frame.data = vec![50, 50, 50];
        let mut params = default_params(&effect.params());
        params.insert("black_point".into(), 100.0);
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        // 50 is below black_point 100, so it clamps to 0
        assert_eq!(frame.data, vec![0, 0, 0]);
    }

    #[test]
    fn levels_gamma_brightens() {
        let mut effect = Levels::default();
        let mut frame = RawFrame::new(1, 1);
        frame.data = vec![128, 128, 128];
        let mut params = default_params(&effect.params());
        params.insert("gamma".into(), 2.0);
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        // gamma > 1 brightens midtones: (128/255)^(1/2) * 255 ≈ 181
        assert!(frame.data[0] > 128, "gamma 2.0 should brighten midtones, got {}", frame.data[0]);
    }
}
