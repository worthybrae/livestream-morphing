use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

#[derive(Default)]
pub struct Grayscale;

impl Effect for Grayscale {
    fn id(&self) -> &'static str {
        "grayscale"
    }
    fn name(&self) -> &'static str {
        "Grayscale"
    }
    fn params(&self) -> Vec<ParamDef> {
        vec![]
    }
    fn init(&mut self, _width: u32, _height: u32) {}
    fn apply(&mut self, frame: &mut RawFrame, _params: &ParamValues, _ctx: &FrameCtx) {
        for pixel in frame.data.chunks_exact_mut(3) {
            let r = pixel[0] as u16;
            let g = pixel[1] as u16;
            let b = pixel[2] as u16;
            let luminance = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
            pixel.fill(luminance);
        }
    }
}

crate::register_effect!(Grayscale);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_rgb_pixels_to_equal_luminance_channels() {
        let mut effect = Grayscale;
        let mut frame = RawFrame::new(2, 1);
        frame.data = vec![255, 0, 0, 0, 255, 0];
        let ctx = FrameCtx { delta_seconds: 1.0/30.0,
            frame_number: 0,
            width: 2,
            height: 1,
        };

        effect.apply(&mut frame, &ParamValues::new(), &ctx);

        assert_eq!(frame.data, vec![76, 76, 76, 149, 149, 149]);
    }
}
