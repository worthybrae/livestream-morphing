use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Chromatic Aberration — shifts the red and blue channels horizontally
/// in opposite directions to simulate lens dispersion.
#[derive(Default)]
pub struct ChromaticAberration;

impl Effect for ChromaticAberration {
    fn id(&self) -> &'static str {
        "chromatic_aberration"
    }

    fn name(&self) -> &'static str {
        "Chromatic Aberration"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("offset", "Offset", 1.0, 20.0, 4.0, 1.0)]
    }

    fn init(&mut self, _width: u32, _height: u32) {}

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let offset = params.get("offset").copied().unwrap_or(4.0).max(1.0) as i32;
        let w = frame.width as i32;
        let h = frame.height as i32;
        let original = frame.data.clone();

        for y in 0..h {
            for x in 0..w {
                let dst = (y * w + x) as usize * 3;

                // Red channel: read from (x + offset, y)
                let rx = (x + offset).clamp(0, w - 1);
                let r_src = (y * w + rx) as usize * 3;
                frame.data[dst] = original[r_src];

                // Green channel: unchanged (x, y)
                // frame.data[dst + 1] already has the correct value from original
                frame.data[dst + 1] = original[dst + 1];

                // Blue channel: read from (x - offset, y)
                let bx = (x - offset).clamp(0, w - 1);
                let b_src = (y * w + bx) as usize * 3;
                frame.data[dst + 2] = original[b_src + 2];
            }
        }
    }
}

crate::register_effect!(ChromaticAberration);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn chromatic_aberration_shifts_channels() {
        let mut effect = ChromaticAberration::default();
        // 5x1 image, only the center pixel is white
        let mut frame = RawFrame::new(5, 1);
        // All black except center pixel (index 2)
        frame.data[6] = 255; // R at pixel 2
        frame.data[7] = 255; // G at pixel 2
        frame.data[8] = 255; // B at pixel 2
        let mut params = default_params(&effect.params());
        params.insert("offset".into(), 1.0);
        let ctx = FrameCtx { frame_number: 0, width: 5, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        // Red channel shifts right: pixel 1 should get red from pixel 2
        assert_eq!(frame.data[3], 255); // pixel 1 red = original pixel 2 red
        // Blue channel shifts left: pixel 3 should get blue from pixel 2
        assert_eq!(frame.data[11], 255); // pixel 3 blue = original pixel 2 blue
        // Green stays at pixel 2
        assert_eq!(frame.data[7], 255);
    }

    #[test]
    fn chromatic_aberration_clamps_at_edges() {
        let mut effect = ChromaticAberration::default();
        // 2x1 image
        let mut frame = RawFrame::new(2, 1);
        frame.data = vec![100, 150, 200, 50, 75, 100];
        let mut params = default_params(&effect.params());
        params.insert("offset".into(), 5.0); // larger than width
        let ctx = FrameCtx { frame_number: 0, width: 2, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        // Should not panic — offset clamped to valid range
        assert_eq!(frame.data.len(), 6);
    }

    #[test]
    fn chromatic_aberration_uniform_unchanged() {
        let mut effect = ChromaticAberration::default();
        let mut frame = RawFrame::filled(4, 4, 100, 100, 100);
        let original = frame.data.clone();
        let mut params = default_params(&effect.params());
        params.insert("offset".into(), 2.0);
        let ctx = FrameCtx { frame_number: 0, width: 4, height: 4 };
        effect.apply(&mut frame, &params, &ctx);
        // Uniform image: shifting channels still reads same value
        assert_eq!(frame.data, original);
    }
}
