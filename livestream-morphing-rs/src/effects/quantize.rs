use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Snap each color channel to N discrete levels.
#[derive(Default)]
pub struct Quantize;

impl Effect for Quantize {
    fn id(&self) -> &'static str {
        "quantize"
    }

    fn name(&self) -> &'static str {
        "Color Quantize"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("levels", "Levels", 2.0, 32.0, 10.0, 1.0)]
    }

    fn init(&mut self, _width: u32, _height: u32) {
        // No scratch buffers needed
    }

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let levels = params.get("levels").copied().unwrap_or(10.0).max(2.0) as u8;
        let step = 255.0 / (levels - 1) as f32;
        for byte in frame.data.iter_mut() {
            let val = *byte as f32;
            *byte = ((val / step).round() * step).clamp(0.0, 255.0) as u8;
        }
    }
}

crate::register_effect!(Quantize);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn quantize_snaps_to_levels() {
        let mut effect = Quantize::default();
        let mut frame = RawFrame::new(2, 1);
        frame.data = vec![50, 50, 50, 200, 200, 200];
        let params = default_params(&effect.params());
        // Use 2 levels to force black/white
        let mut params2 = params.clone();
        params2.insert("levels".into(), 2.0);
        let ctx = FrameCtx { frame_number: 0, width: 2, height: 1 };
        effect.apply(&mut frame, &params2, &ctx);
        assert_eq!(frame.data, vec![0, 0, 0, 255, 255, 255]);
    }

    #[test]
    fn quantize_with_more_levels() {
        let mut effect = Quantize::default();
        let mut frame = RawFrame::new(1, 1);
        frame.data = vec![100, 100, 100];
        let mut params = default_params(&effect.params());
        params.insert("levels".into(), 4.0);
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, vec![85, 85, 85]);
    }

    #[test]
    fn quantize_default_params() {
        let effect = Quantize::default();
        let params = default_params(&effect.params());
        assert_eq!(params.get("levels"), Some(&10.0));
    }
}
