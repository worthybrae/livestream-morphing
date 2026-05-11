use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Mirror / Kaleidoscope — mirrors the frame along various axes.
#[derive(Default)]
pub struct Mirror;

impl Effect for Mirror {
    fn id(&self) -> &'static str {
        "mirror"
    }

    fn name(&self) -> &'static str {
        "Mirror"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("mode", "Mode", 0.0, 3.0, 0.0, 1.0)]
    }

    fn init(&mut self, _width: u32, _height: u32) {}

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let mode = params.get("mode").copied().unwrap_or(0.0).clamp(0.0, 3.0) as u32;
        let w = frame.width as usize;
        let h = frame.height as usize;
        let original = frame.data.clone();

        match mode {
            0 => {
                // Horizontal: left half mirrors to right
                for y in 0..h {
                    for x in (w / 2)..w {
                        let src_x = w - 1 - x;
                        let dst = (y * w + x) * 3;
                        let src = (y * w + src_x) * 3;
                        frame.data[dst..dst + 3].copy_from_slice(&original[src..src + 3]);
                    }
                }
            }
            1 => {
                // Vertical: top half mirrors to bottom
                for y in (h / 2)..h {
                    let src_y = h - 1 - y;
                    for x in 0..w {
                        let dst = (y * w + x) * 3;
                        let src = (src_y * w + x) * 3;
                        frame.data[dst..dst + 3].copy_from_slice(&original[src..src + 3]);
                    }
                }
            }
            2 => {
                // Quad: top-left mirrors to all 4 quadrants
                let half_w = w / 2;
                let half_h = h / 2;
                for y in 0..h {
                    for x in 0..w {
                        let src_x = if x >= half_w { w - 1 - x } else { x };
                        let src_y = if y >= half_h { h - 1 - y } else { y };
                        let dst = (y * w + x) * 3;
                        let src = (src_y * w + src_x) * 3;
                        frame.data[dst..dst + 3].copy_from_slice(&original[src..src + 3]);
                    }
                }
            }
            3 | _ => {
                // Diagonal: read from transposed coordinates for interesting symmetry
                for y in 0..h {
                    for x in 0..w {
                        let sx = ((y as u64 * w as u64) / h as u64).min((w - 1) as u64) as usize;
                        let sy = ((x as u64 * h as u64) / w as u64).min((h - 1) as u64) as usize;
                        let dst = (y * w + x) * 3;
                        let src = (sy * w + sx) * 3;
                        frame.data[dst..dst + 3].copy_from_slice(&original[src..src + 3]);
                    }
                }
            }
        }
    }
}

crate::register_effect!(Mirror);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn mirror_horizontal_symmetry() {
        let mut effect = Mirror::default();
        // 4x1 image: [10, 20, 30, 40] (one channel for simplicity, but we use RGB)
        let mut frame = RawFrame::new(4, 1);
        frame.data = vec![
            10, 10, 10, 20, 20, 20, 30, 30, 30, 40, 40, 40,
        ];
        let params = default_params(&effect.params()); // mode=0
        let ctx = FrameCtx { frame_number: 0, width: 4, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        // Right half mirrors left: pixel[2] = pixel[1], pixel[3] = pixel[0]
        assert_eq!(frame.data[6], 20); // pixel 2 = pixel 1
        assert_eq!(frame.data[9], 10); // pixel 3 = pixel 0
    }

    #[test]
    fn mirror_vertical_symmetry() {
        let mut effect = Mirror::default();
        // 1x4 image
        let mut frame = RawFrame::new(1, 4);
        frame.data = vec![
            10, 10, 10, 20, 20, 20, 30, 30, 30, 40, 40, 40,
        ];
        let mut params = default_params(&effect.params());
        params.insert("mode".into(), 1.0);
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 4 };
        effect.apply(&mut frame, &params, &ctx);
        // Bottom half mirrors top: row[2] = row[1], row[3] = row[0]
        assert_eq!(frame.data[6], 20); // row 2 = row 1
        assert_eq!(frame.data[9], 10); // row 3 = row 0
    }

    #[test]
    fn mirror_uniform_unchanged() {
        let mut effect = Mirror::default();
        let mut frame = RawFrame::filled(4, 4, 100, 100, 100);
        let original = frame.data.clone();
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 4, height: 4 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original);
    }
}
