use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Gradient Map — converts each pixel to grayscale luminance then maps it
/// through a predefined color gradient palette.
#[derive(Default)]
pub struct GradientMap;

/// A gradient stop: (position 0-255, r, g, b).
type Stop = (u8, u8, u8, u8);

fn palette_stops(index: u8) -> &'static [Stop] {
    match index {
        0 => &[(0, 20, 0, 80), (128, 200, 80, 20), (255, 255, 200, 50)],           // Sunset
        1 => &[(0, 10, 10, 60), (128, 20, 120, 180), (255, 200, 240, 255)],         // Ocean
        2 => &[(0, 180, 0, 255), (128, 0, 255, 200), (255, 0, 255, 0)],             // Neon
        3 => &[(0, 0, 0, 0), (85, 180, 0, 0), (170, 255, 150, 0), (255, 255, 255, 100)], // Fire
        4 => &[(0, 40, 0, 80), (128, 100, 150, 220), (255, 220, 240, 255)],         // Ice
        5 => &[(0, 40, 30, 20), (128, 180, 150, 100), (255, 240, 230, 210)],        // Vintage
        6 => &[(0, 0, 10, 0), (128, 0, 100, 0), (255, 0, 255, 50)],                // Matrix
        7 => &[(0, 30, 0, 60), (85, 180, 0, 50), (170, 255, 100, 0), (255, 255, 255, 150)], // Infrared
        _ => &[(0, 0, 0, 0), (255, 255, 255, 255)],                                 // Fallback grayscale
    }
}

fn lerp_gradient(stops: &[Stop], lum: u8) -> (u8, u8, u8) {
    let pos = lum;
    // Find the two stops the luminance falls between
    let mut lo = 0;
    for i in 1..stops.len() {
        if stops[i].0 >= pos {
            lo = i - 1;
            break;
        }
        lo = i - 1;
    }
    let hi = (lo + 1).min(stops.len() - 1);
    let (p0, r0, g0, b0) = stops[lo];
    let (p1, r1, g1, b1) = stops[hi];
    if p0 == p1 {
        return (r0, g0, b0);
    }
    let t = (pos as f32 - p0 as f32) / (p1 as f32 - p0 as f32);
    let r = (r0 as f32 + t * (r1 as f32 - r0 as f32)).clamp(0.0, 255.0) as u8;
    let g = (g0 as f32 + t * (g1 as f32 - g0 as f32)).clamp(0.0, 255.0) as u8;
    let b = (b0 as f32 + t * (b1 as f32 - b0 as f32)).clamp(0.0, 255.0) as u8;
    (r, g, b)
}

impl Effect for GradientMap {
    fn id(&self) -> &'static str {
        "gradient_map"
    }

    fn name(&self) -> &'static str {
        "Gradient Map"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("palette", "Palette", 0.0, 7.0, 0.0, 1.0)]
    }

    fn init(&mut self, _width: u32, _height: u32) {}

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let palette_idx = params.get("palette").copied().unwrap_or(0.0).clamp(0.0, 7.0) as u8;
        let stops = palette_stops(palette_idx);

        for pixel in frame.data.chunks_exact_mut(3) {
            let lum = (0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32)
                .clamp(0.0, 255.0) as u8;
            let (r, g, b) = lerp_gradient(stops, lum);
            pixel[0] = r;
            pixel[1] = g;
            pixel[2] = b;
        }
    }
}

crate::register_effect!(GradientMap);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn gradient_map_black_maps_to_first_stop() {
        let mut effect = GradientMap::default();
        let mut frame = RawFrame::filled(1, 1, 0, 0, 0);
        let params = default_params(&effect.params()); // palette=0 (Sunset)
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        // Luminance 0 -> first stop of Sunset: (20, 0, 80)
        assert_eq!(frame.data, vec![20, 0, 80]);
    }

    #[test]
    fn gradient_map_white_maps_to_last_stop() {
        let mut effect = GradientMap::default();
        let mut frame = RawFrame::filled(1, 1, 255, 255, 255);
        let params = default_params(&effect.params()); // palette=0 (Sunset)
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        // Luminance 255 -> last stop of Sunset: (255, 200, 50)
        assert_eq!(frame.data, vec![255, 200, 50]);
    }

    #[test]
    fn gradient_map_different_palette() {
        let mut effect = GradientMap::default();
        let mut frame = RawFrame::filled(1, 1, 0, 0, 0);
        let mut params = default_params(&effect.params());
        params.insert("palette".into(), 6.0); // Matrix
        let ctx = FrameCtx { frame_number: 0, width: 1, height: 1 };
        effect.apply(&mut frame, &params, &ctx);
        // Luminance 0 -> first stop of Matrix: (0, 10, 0)
        assert_eq!(frame.data, vec![0, 10, 0]);
    }
}
