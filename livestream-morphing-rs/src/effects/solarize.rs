use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Solarize — inverts channel values that are above the threshold,
/// creating a partial-negative look.
#[derive(Default)]
pub struct Solarize;

impl Effect for Solarize {
    fn id(&self) -> &'static str {
        "solarize"
    }

    fn name(&self) -> &'static str {
        "Solarize"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("threshold", "Threshold", 0.0, 255.0, 128.0, 1.0)]
    }

    fn init(&mut self, _width: u32, _height: u32) {}

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let threshold = params.get("threshold").copied().unwrap_or(128.0) as u8;
        for byte in frame.data.iter_mut() {
            if *byte >= threshold {
                *byte = 255 - *byte;
            }
        }
    }
}

crate::register_effect!(Solarize);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn solarize_inverts_above_threshold() {
        let mut effect = Solarize::default();
        let mut frame = RawFrame::new(1, 1);
        frame.data = vec![200, 200, 200];
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        // 200 >= 128, so 255 - 200 = 55
        assert_eq!(frame.data, vec![55, 55, 55]);
    }

    #[test]
    fn solarize_leaves_below_threshold() {
        let mut effect = Solarize::default();
        let mut frame = RawFrame::new(1, 1);
        frame.data = vec![50, 50, 50];
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        // 50 < 128, unchanged
        assert_eq!(frame.data, vec![50, 50, 50]);
    }

    #[test]
    fn solarize_custom_threshold() {
        let mut effect = Solarize::default();
        let mut frame = RawFrame::new(1, 1);
        frame.data = vec![100, 200, 50];
        let mut params = default_params(&effect.params());
        params.insert("threshold".into(), 150.0);
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        // 100 < 150 -> unchanged, 200 >= 150 -> 55, 50 < 150 -> unchanged
        assert_eq!(frame.data, vec![100, 55, 50]);
    }
}
