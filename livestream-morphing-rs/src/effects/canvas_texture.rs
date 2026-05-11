use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Multiply-blend a procedural canvas-weave texture over the frame.
#[derive(Default)]
pub struct CanvasTexture {
    texture: Vec<u8>,
}

fn generate_canvas_texture(width: u32, height: u32) -> Vec<u8> {
    let mut texture = vec![0u8; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let h = x
                .wrapping_mul(374761393)
                .wrapping_add(y.wrapping_mul(668265263))
                .wrapping_mul(1274126177);
            let noise = ((h >> 24) & 0x1F) as u8;
            let weave: u8 = if (x % 4 < 2) ^ (y % 4 < 2) { 10 } else { 0 };
            texture[(y * width + x) as usize] = 200u8.wrapping_add(noise).wrapping_add(weave);
        }
    }
    texture
}

impl Effect for CanvasTexture {
    fn id(&self) -> &'static str {
        "canvas_texture"
    }

    fn name(&self) -> &'static str {
        "Canvas Texture"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![ParamDef::new("strength", "Strength", 0.0, 1.0, 0.15, 0.01)]
    }

    fn init(&mut self, width: u32, height: u32) {
        self.texture = generate_canvas_texture(width, height);
    }

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, _ctx: &FrameCtx) {
        let strength = params.get("strength").copied().unwrap_or(0.15);
        let pixel_count = (frame.width * frame.height) as usize;
        for i in 0..pixel_count {
            let tex = self.texture[i % self.texture.len()] as f32 / 255.0;
            let factor = 1.0 - strength + strength * tex;
            for c in 0..3 {
                let idx = i * 3 + c;
                frame.data[idx] = (frame.data[idx] as f32 * factor).clamp(0.0, 255.0) as u8;
            }
        }
    }
}

crate::register_effect!(CanvasTexture);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn canvas_texture_darkens_pixels_at_half_strength() {
        let mut effect = CanvasTexture::default();
        effect.init(4, 4);
        let mut frame = RawFrame::filled(4, 4, 200, 200, 200);
        let mut params = default_params(&effect.params());
        params.insert("strength".into(), 0.5);
        let ctx = FrameCtx { frame_number: 0, width: 4, height: 4 };
        effect.apply(&mut frame, &params, &ctx);
        // Canvas texture values are ~200/255, so factor < 1.0 at strength 0.5 → darkened
        assert!(frame.data[0] < 200, "Should be darkened, got {}", frame.data[0]);
        assert!(frame.data[0] > 100, "Shouldn't be too dark, got {}", frame.data[0]);
    }

    #[test]
    fn canvas_texture_zero_strength_is_identity() {
        let mut effect = CanvasTexture::default();
        effect.init(4, 4);
        let mut frame = RawFrame::filled(4, 4, 150, 150, 150);
        let original = frame.data.clone();
        let mut params = default_params(&effect.params());
        params.insert("strength".into(), 0.0);
        let ctx = FrameCtx { frame_number: 0, width: 4, height: 4 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original, "Zero strength should leave frame unchanged");
    }

    #[test]
    fn canvas_texture_default_params() {
        let effect = CanvasTexture::default();
        let params = default_params(&effect.params());
        assert_eq!(params.get("strength"), Some(&0.15));
    }
}
