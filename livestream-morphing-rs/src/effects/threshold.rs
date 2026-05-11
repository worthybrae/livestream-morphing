use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Threshold — converts each pixel to pure black or white based on luminance.
#[derive(Default)]
pub struct Threshold;

impl Effect for Threshold {
    fn id(&self) -> &'static str {
        "threshold"
    }

    fn name(&self) -> &'static str {
        "Threshold"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("cutoff", "Cutoff", 0.0, 255.0, 128.0, 1.0)]
    }

    fn init(&mut self, _width: u32, _height: u32) {}

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let cutoff = params.get("cutoff").copied().unwrap_or(128.0);
        for pixel in frame.data.chunks_exact_mut(3) {
            let gray = 0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32;
            let val = if gray >= cutoff { 255 } else { 0 };
            pixel[0] = val;
            pixel[1] = val;
            pixel[2] = val;
        }
    }
}

crate::register_effect!(Threshold);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn threshold_bright_becomes_white() {
        let mut effect = Threshold::default();
        let mut frame = RawFrame::new(1, 1);
        frame.data = vec![200, 200, 200];
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, vec![255, 255, 255]);
    }

    #[test]
    fn threshold_dark_becomes_black() {
        let mut effect = Threshold::default();
        let mut frame = RawFrame::new(1, 1);
        frame.data = vec![50, 50, 50];
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, vec![0, 0, 0]);
    }

    #[test]
    fn threshold_custom_cutoff() {
        let mut effect = Threshold::default();
        let mut frame = RawFrame::new(1, 1);
        // Gray value = 0.299*100 + 0.587*100 + 0.114*100 = 100
        frame.data = vec![100, 100, 100];
        let mut params = default_params(&effect.params());
        params.insert("cutoff".into(), 50.0);
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        // 100 >= 50, so white
        assert_eq!(frame.data, vec![255, 255, 255]);
    }
}
