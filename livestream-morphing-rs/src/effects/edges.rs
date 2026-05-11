use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Sobel edge detection + dark overlay on the frame.
#[derive(Default)]
pub struct EdgeDetect {
    grayscale: Vec<u8>,
    edges: Vec<u8>,
}

impl Effect for EdgeDetect {
    fn id(&self) -> &'static str {
        "edges"
    }

    fn name(&self) -> &'static str {
        "Edge Detection"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef::new("threshold", "Threshold", 1.0, 128.0, 30.0, 1.0),
            ParamDef::new("darkness", "Darkness", 0.0, 255.0, 80.0, 1.0),
        ]
    }

    fn init(&mut self, width: u32, height: u32) {
        let pixel_count = (width * height) as usize;
        self.grayscale = vec![0u8; pixel_count];
        self.edges = vec![0u8; pixel_count];
    }

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let threshold = params.get("threshold").copied().unwrap_or(30.0) as u8;
        let darkness = params.get("darkness").copied().unwrap_or(80.0) as u8;

        let w = frame.width as usize;
        let h = frame.height as usize;

        // RGB → grayscale (BT.601 weights)
        for i in 0..(w * h) {
            let r = frame.data[i * 3] as u16;
            let g = frame.data[i * 3 + 1] as u16;
            let b = frame.data[i * 3 + 2] as u16;
            self.grayscale[i] = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
        }

        // Clear edges
        self.edges.iter_mut().for_each(|e| *e = 0);

        // Sobel (skip border pixels)
        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                let g = |dy: i32, dx: i32| -> i16 {
                    self.grayscale[((y as i32 + dy) as usize) * w + (x as i32 + dx) as usize]
                        as i16
                };
                let gx =
                    -g(-1, -1) + g(-1, 1) - 2 * g(0, -1) + 2 * g(0, 1) - g(1, -1) + g(1, 1);
                let gy =
                    -g(-1, -1) - 2 * g(-1, 0) - g(-1, 1) + g(1, -1) + 2 * g(1, 0) + g(1, 1);
                let mag = ((gx.unsigned_abs() + gy.unsigned_abs()) / 2).min(255) as u8;
                self.edges[y * w + x] = if mag > threshold { 255 } else { 0 };
            }
        }

        // Overlay dark edges
        for i in 0..(w * h) {
            if self.edges[i] > 0 {
                frame.data[i * 3] = frame.data[i * 3].saturating_sub(darkness);
                frame.data[i * 3 + 1] = frame.data[i * 3 + 1].saturating_sub(darkness);
                frame.data[i * 3 + 2] = frame.data[i * 3 + 2].saturating_sub(darkness);
            }
        }
    }
}

crate::register_effect!(EdgeDetect);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn edges_detected_at_sharp_boundary() {
        // Frame: left half white (255), right half black (0)
        let mut frame = RawFrame::new(8, 4);
        for y in 0..4u32 {
            for x in 0..8u32 {
                let idx = ((y * 8 + x) * 3) as usize;
                let val = if x < 4 { 255 } else { 0 };
                frame.data[idx] = val;
                frame.data[idx + 1] = val;
                frame.data[idx + 2] = val;
            }
        }
        let original = frame.data.clone();

        let mut effect = EdgeDetect::default();
        effect.init(8, 4);
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 8, height: 4 };
        effect.apply(&mut frame, &params, &ctx);

        // Pixel at (1, 3) is on the white side near the boundary — should be darkened
        let white_edge_pixel = frame.data[((1 * 8 + 3) * 3) as usize];
        let original_white = original[((1 * 8 + 3) * 3) as usize];
        assert!(
            white_edge_pixel < original_white,
            "Edge pixel on white side should be darkened: got {white_edge_pixel}, original {original_white}"
        );
    }

    #[test]
    fn no_edges_on_uniform_frame() {
        let mut frame = RawFrame::filled(8, 8, 128, 128, 128);
        let original = frame.data.clone();
        let mut effect = EdgeDetect::default();
        effect.init(8, 8);
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 8, height: 8 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original, "Uniform frame should have no edges");
    }

    #[test]
    fn edges_default_params() {
        let effect = EdgeDetect::default();
        let params = default_params(&effect.params());
        assert_eq!(params.get("threshold"), Some(&30.0));
        assert_eq!(params.get("darkness"), Some(&80.0));
    }
}
