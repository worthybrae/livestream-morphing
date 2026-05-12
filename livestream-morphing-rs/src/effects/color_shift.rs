use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Per-pixel HSV adjustment: hue rotation, saturation scaling, brightness scaling.
#[derive(Default)]
pub struct ColorShift;

// ---------------------------------------------------------------------------
// RGB ↔ HSV helpers (all values in [0,1] range)
// ---------------------------------------------------------------------------

/// Convert RGB (each 0..=255) to HSV (h: 0..360, s: 0..=1, v: 0..=1).
fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;

    let cmax = rf.max(gf).max(bf);
    let cmin = rf.min(gf).min(bf);
    let delta = cmax - cmin;

    let h = if delta < 1e-6 {
        0.0
    } else if (cmax - rf).abs() < 1e-6 {
        60.0 * (((gf - bf) / delta) % 6.0)
    } else if (cmax - gf).abs() < 1e-6 {
        60.0 * (((bf - rf) / delta) + 2.0)
    } else {
        60.0 * (((rf - gf) / delta) + 4.0)
    };

    let h = if h < 0.0 { h + 360.0 } else { h };

    let s = if cmax < 1e-6 { 0.0 } else { delta / cmax };
    let v = cmax;

    (h, s, v)
}

/// Convert HSV (h: 0..360, s: 0..=1, v: 0..=1) back to RGB (each 0..=255).
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    let h = h.rem_euclid(360.0);

    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    let to_u8 = |f: f32| ((f + m) * 255.0).clamp(0.0, 255.0) as u8;
    (to_u8(r1), to_u8(g1), to_u8(b1))
}

// ---------------------------------------------------------------------------
// Effect impl
// ---------------------------------------------------------------------------

impl Effect for ColorShift {
    fn id(&self) -> &'static str {
        "color_shift"
    }

    fn name(&self) -> &'static str {
        "Color Shift"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef::new("hue", "Hue", 0.0, 360.0, 0.0, 1.0),
            ParamDef::new("saturation", "Saturation", 0.0, 2.0, 1.0, 0.05),
            ParamDef::new("brightness", "Brightness", 0.0, 2.0, 1.0, 0.05),
        ]
    }

    fn init(&mut self, _width: u32, _height: u32) {
        // No scratch buffers needed.
    }

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let hue = params.get("hue").copied().unwrap_or(0.0);
        let saturation = params.get("saturation").copied().unwrap_or(1.0);
        let brightness = params.get("brightness").copied().unwrap_or(1.0);

        // Identity fast-path
        if hue == 0.0 && (saturation - 1.0).abs() < 1e-6 && (brightness - 1.0).abs() < 1e-6 {
            return;
        }

        for pixel in frame.data.chunks_exact_mut(3) {
            let (h, s, v) = rgb_to_hsv(pixel[0], pixel[1], pixel[2]);
            let new_h = h + hue;
            let new_s = (s * saturation).clamp(0.0, 1.0);
            let new_v = (v * brightness).clamp(0.0, 1.0);
            let (r, g, b) = hsv_to_rgb(new_h, new_s, new_v);
            pixel[0] = r;
            pixel[1] = g;
            pixel[2] = b;
        }
    }
}

crate::register_effect!(ColorShift);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn color_shift_identity_at_defaults() {
        let mut effect = ColorShift::default();
        let mut frame = RawFrame::filled(4, 4, 100, 150, 200);
        let original = frame.data.clone();
        let params = default_params(&effect.params());
        let ctx = FrameCtx { frame_number: 0, width: 4, height: 4 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original);
    }

    #[test]
    fn color_shift_brightness_half_darkens() {
        let mut effect = ColorShift::default();
        // Use a mid-grey pixel so rounding is clear.
        let mut frame = RawFrame::filled(2, 2, 200, 200, 200);
        let mut params = default_params(&effect.params());
        params.insert("brightness".into(), 0.5);
        let ctx = FrameCtx { frame_number: 0, width: 2, height: 2 };
        effect.apply(&mut frame, &params, &ctx);
        // Every channel should be darker than the original 200.
        for chunk in frame.data.chunks_exact(3) {
            assert!(chunk[0] < 200, "red channel should be darker");
            assert!(chunk[1] < 200, "green channel should be darker");
            assert!(chunk[2] < 200, "blue channel should be darker");
        }
    }

    /// A pure red pixel rotated by 120° should become green-dominant.
    #[test]
    fn color_shift_hue_120_red_becomes_green_dominant() {
        let mut effect = ColorShift::default();
        // Pure red: (255, 0, 0) → HSV (0°, 1, 1)
        let mut frame = RawFrame::new(1, 1);
        frame.data = vec![255, 0, 0];
        let mut params = default_params(&effect.params());
        params.insert("hue".into(), 120.0);
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        // After +120° rotation (red→green), green channel should dominate.
        assert!(
            frame.data[1] > frame.data[0] && frame.data[1] > frame.data[2],
            "green ({}) should dominate red ({}) and blue ({})",
            frame.data[1],
            frame.data[0],
            frame.data[2]
        );
    }
}
