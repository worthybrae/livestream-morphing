//! Material studies. Every edition ends with the unchanged Living Ink wave field.
//! No temporal accumulation, optical flow, remote inference, or extra video source.
use super::{distortion::Distortion, living_ink::LivingInk};
use crate::{
    effects::RawFrame,
    registry::{Effect, FrameCtx, ParamDef, ParamValues},
};

#[derive(Default)]
pub struct Editions {
    ink: LivingInk,
    warp: Distortion,
    luma: Vec<f32>,
    smooth: Vec<f32>,
    scratch: Vec<f32>,
    glow: Vec<f32>,
}
fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0., 1.);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}
fn grain(x: usize, y: usize) -> f32 {
    let mut n = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263);
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    ((n ^ (n >> 16)) & 1023) as f32 / 1023.
}
fn repeat(v: f32, period: f32) -> f32 {
    v - (v / period).floor() * period
}
fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

impl Effect for Editions {
    fn id(&self) -> &'static str {
        "ink_editions"
    }
    fn name(&self) -> &'static str {
        "Living Ink Editions"
    }
    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef::new("edition", "Edition", 0., 16., 0., 1.),
            ParamDef::new("detail", "Detail", 0.5, 2., 1., 0.05),
            ParamDef::new("pigment", "Pigment", 0.5, 1.5, 1., 0.05),
        ]
    }
    fn init(&mut self, w: u32, h: u32) {
        self.ink.init(w, h);
        self.luma = vec![0.; (w * h) as usize];
        self.smooth = vec![0.; (w * h) as usize];
        self.scratch = vec![0.; (w * h) as usize];
        self.glow = vec![0.; (w * h) as usize];
    }
    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, ctx: &FrameCtx) {
        let w = frame.width as usize;
        let h = frame.height as usize;
        if w < 3 || h < 3 {
            return;
        }
        if self.luma.len() != w * h {
            self.init(frame.width, frame.height)
        }
        let get = |id: &str, default: f32| {
            params
                .get(id)
                .copied()
                .filter(|x| x.is_finite())
                .unwrap_or(default)
        };
        let edition = get("edition", 0.).round().clamp(0., 16.) as u32;
        let detail = get("detail", 1.).clamp(0.5, 2.);
        let pigment = get("pigment", 1.).clamp(0.5, 1.5);
        for (i, p) in frame.data.chunks_exact(3).enumerate() {
            self.luma[i] = (p[0] as f32 * 0.299 + p[1] as f32 * 0.587 + p[2] as f32 * 0.114) / 255.
        }
        // Small spatial average quiets sensor grain; it never smears moving objects over time.
        super::living_ink::blur(
            &self.luma,
            &mut self.scratch,
            &mut self.smooth,
            w,
            h,
            if edition == 5 { 6 } else { 2 },
        );
        let mut ink_params: ParamValues =
            serde_json::from_str(include_str!("../living_ink_preset.json")).unwrap();
        ink_params.insert("warp_amplitude".into(), 0.); // apply signature once, after the material
        ink_params.insert("charcoal".into(), 0.);
        ink_params.insert("highlight".into(), 255.);
        ink_params.insert("fine_threshold".into(), 16. / detail);
        ink_params.insert(
            "fine_weight".into(),
            if edition == 3 || edition == 7 {
                1.2
            } else {
                1.5
            },
        );
        self.ink.apply(frame, &ink_params, ctx);
        for (i, p) in frame.data.chunks_exact(3).enumerate() {
            self.luma[i] = p[0] as f32 / 255.;
        }
        if edition == 8 {
            super::living_ink::blur(&self.luma, &mut self.scratch, &mut self.glow, w, h, 3);
        }
        let scale = w as f32 / 1920.;
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let p = i * 3;
                let line = frame.data[p] as f32 / 255.;
                let l = self.smooth[i];
                let n = grain(x, y);
                let darkness = (1. - l).clamp(0., 1.);
                let sx = x as f32 / scale;
                let sy = y as f32 / scale;
                let dx = self.smooth[y * w + (x + 1).min(w - 1)]
                    - self.smooth[y * w + x.saturating_sub(1)];
                let dy = self.smooth[(y + 1).min(h - 1) * w + x]
                    - self.smooth[y.saturating_sub(1) * w + x];
                let c = match edition {
                    // Etched graphite: a pale paper field, open strokes, a second hatch in shadow.
                    0 => {
                        let hatch = (1. - (repeat(sx + sy, 10.) - 5.).abs() / 0.9).max(0.)
                            * smoothstep(0.55, 0.9, darkness)
                            * 0.28;
                        let cross = (1. - (repeat(sx - sy, 13.) - 6.5).abs() / 0.8).max(0.)
                            * smoothstep(0.8, 0.98, darkness)
                            * 0.20;
                        mix(
                            [239. + n * 5., 237. + n * 5., 230. + n * 5.],
                            [30., 31., 30.],
                            (line * 0.9 + hatch + cross) * pigment,
                        )
                    }
                    // Silver relief: signed light catches one face of each engraved contour.
                    1 => {
                        let relief = ((dx - dy) * 4.).clamp(-0.3, 0.3)
                            * smoothstep(0.025, 0.10, (dx - dy).abs());
                        let v = (0.14 + relief + line * 0.80 + l * 0.05 + n * 0.006).clamp(0., 1.);
                        mix([8., 12., 17.], [226., 233., 236.], v * pigment)
                    }
                    // Cyanotype: light-sensitive blue paper, cream outlines and a quiet tonal wash.
                    2 => mix(
                        [8., 24., 58.],
                        [216., 231., 219.],
                        (line * 0.95 + smoothstep(0.12, 0.70, l) * 0.28 + n * 0.006) * pigment,
                    ),
                    // Copperpoint: amber metal against a nearly black ground.
                    3 => {
                        let screen = if repeat(sx + sy * 0.65, 8.) < 0.8 {
                            l * 0.13
                        } else {
                            0.
                        };
                        mix(
                            [18., 13., 12.],
                            [246., 183., 116.],
                            (line * 1.15 + screen) * pigment,
                        )
                    }
                    // Porcelain: hard black calligraphy with a slight cool cast in the paper.
                    4 => mix(
                        [239., 243., 239.],
                        [13., 23., 28.],
                        (line * 1.05 + smoothstep(0.78, 0.94, darkness) * 0.10) * pigment,
                    ),
                    // Isobars: brightness becomes stacked contour lines, with the drawing as anchor.
                    5 => {
                        let band = (l * 14.).fract();
                        let iso = (1. - smoothstep(0.02, 0.12, band.min(1. - band))) * 0.65;
                        mix(
                            [20., 23., 24.],
                            [211., 223., 213.],
                            (line * 0.7 + iso * 0.45) * pigment,
                        )
                    }
                    // Night glass: dual-tone edge light, geometry remains untouched until the shared warp.
                    6 => {
                        let base = mix(
                            [7., 14., 22.],
                            [76., 143., 150.],
                            (line * 0.85 + l * 0.07) * pigment,
                        );
                        mix(
                            base,
                            [235., 184., 112.],
                            ((dx + dy) * 12.).max(0.).min(line) * 0.65,
                        )
                    }
                    // Two-ink risograph: grain-bound ultramarine shadows and vermilion contours.
                    7 => {
                        let hatch = (1. - (repeat(sx + sy * 0.5, 6.) - 3.).abs() / 1.1).max(0.)
                            * smoothstep(0.45, 0.92, darkness);
                        let ink = hatch * 0.42 + (darkness * 0.08);
                        let paper = mix([244., 232., 206.], [31., 48., 92.], ink);
                        mix(paper, [164., 37., 39.], line * 1.2 * pigment)
                    }
                    // Phosphor: thin mineral-green light; no persistent afterimage.
                    8 => {
                        let scan = if (sy as usize) % 4 == 0 { 0.88 } else { 1. };
                        mix(
                            [8., 17., 15.],
                            [177., 221., 187.],
                            (line * scan + self.glow[i] * 0.45 + l * 0.035) * pigment,
                        )
                    }
                    // Aquatint: spatially stable stippling carries the shadow, ink carries the edges.
                    9 => {
                        let dots = if n < darkness * 0.30 { 0.45 } else { 0. };
                        mix(
                            [223., 218., 204.],
                            [42., 39., 35.],
                            (line * 0.8 + dots) * pigment,
                        )
                    }
                    // Solar silver: a folded tonal curve creates metallic light around dark masses.
                    10 => {
                        let fold = (l * 2.2 - 0.7).abs().clamp(0., 1.);
                        mix(
                            [12., 15., 20.],
                            [230., 233., 232.],
                            (fold * 0.48 + line * 0.8) * pigment,
                        )
                    }
                    // Cinnabar: broad vermilion masses crossed by hairline ivory.
                    11 => {
                        let mass = smoothstep(0.08, 0.65, l);
                        let base = mix([35., 15., 18.], [137., 49., 45.], mass * 0.65);
                        mix(base, [236., 218., 183.], line * pigment)
                    }
                    // Iris: violet mineral ground, pale green accents at the strongest ridges.
                    12 => {
                        let base = mix(
                            [22., 19., 36.],
                            [153., 138., 178.],
                            (line * 0.9 + l * 0.10) * pigment,
                        );
                        mix(base, [201., 226., 192.], smoothstep(0.6, 1., line) * 0.65)
                    }
                    // Engraved halftone: dot diameter follows light, fixed cells move with the signature.
                    13 => {
                        let cx = repeat(sx + sy * 0.22, 7.) - 3.5;
                        let cy = repeat(sy, 7.) - 3.5;
                        let radius = 2.7 * smoothstep(0.1, 0.95, darkness);
                        let dots = 1.
                            - smoothstep(
                                (radius - 0.45).max(0.),
                                radius + 0.45,
                                (cx * cx + cy * cy).sqrt(),
                            );
                        mix(
                            [239., 229., 207.],
                            [27., 29., 31.],
                            (dots * 0.80 + line * 0.85) * pigment,
                        )
                    }
                    // Woodcut: broad black masses opened by thin cuts of unprinted paper.
                    14 => {
                        let mass = smoothstep(0.32, 0.44, darkness);
                        let cuts = (1. - (repeat(sx - sy * 0.3, 12.) - 6.).abs() / 1.).max(0.)
                            * smoothstep(0.55, 0.95, darkness)
                            * 0.23;
                        let ink = (mass * 0.90 - line * 0.55 - cuts).clamp(0., 1.);
                        mix([229., 218., 190.], [21., 25., 24.], ink * pigment)
                    }
                    // Woven topography: contour pressure bends a fixed loom, without a second time warp.
                    15 => {
                        let thread = (1. - (repeat(sy + l * 22., 8.) - 4.).abs() / 1.1).max(0.);
                        let weave = (1. - (repeat(sx + l * 10., 19.) - 9.5).abs() / 0.7).max(0.);
                        mix(
                            [17., 24., 28.],
                            [192., 209., 204.],
                            (thread * (0.20 + l * 0.50) + weave * 0.11 + line * 0.72) * pigment,
                        )
                    }
                    // Mineral blocks: a small fixed palette turns broad light into cut paper.
                    _ => {
                        let tone = smoothstep(0.03, 0.65, l);
                        let base = if tone < 0.22 {
                            [20., 35., 46.]
                        } else if tone < 0.48 {
                            [47., 83., 93.]
                        } else if tone < 0.73 {
                            [132., 145., 129.]
                        } else {
                            [203., 191., 151.]
                        };
                        mix(base, [237., 225., 194.], line * 0.85 * pigment)
                    }
                };
                for k in 0..3 {
                    frame.data[p + k] = c[k].round().clamp(0., 255.) as u8;
                }
            }
        }
        let seconds = ctx.frame_number as f64 * ctx.delta_seconds.max(0.) as f64;
        self.warp.apply_rgb_at_phase(
            frame,
            0.012,
            8.,
            (seconds / 8.) as f32 * std::f32::consts::TAU,
        );
    }
}
crate::register_effect!(Editions);

#[cfg(test)]
mod tests {
    use super::*;
    fn image() -> RawFrame {
        let mut frame = RawFrame::new(96, 54);
        for y in 0..54 {
            for x in 0..96 {
                let p = (y * 96 + x) * 3;
                frame.data[p] = ((x * 5 + y * 2) % 256) as u8;
                frame.data[p + 1] = ((x * 2 + y * 7) % 256) as u8;
                frame.data[p + 2] = ((x + y) * 3 % 256) as u8;
            }
        }
        frame
    }
    fn context(number: u32) -> FrameCtx {
        FrameCtx {
            frame_number: number,
            delta_seconds: 1001. / 30000.,
            width: 96,
            height: 54,
        }
    }
    #[test]
    fn rgb_signature_matches_existing_monochrome_geometry_byte_for_byte() {
        let mut original = image();
        for p in original.data.chunks_exact_mut(3) {
            p[1] = p[0];
            p[2] = p[0];
        }
        for phase in [0., 0.4, 2.7, 6.2] {
            let mut a = original.clone();
            let mut b = original.clone();
            Distortion::default().apply_monochrome(&mut a, 0.012, 8., phase);
            Distortion::default().apply_rgb_at_phase(&mut b, 0.012, 8., phase);
            assert_eq!(a.data, b.data);
        }
    }
    #[test]
    fn every_material_is_distinct_deterministic_and_keeps_moving() {
        let mut results = std::collections::HashSet::new();
        for edition in 0..=16 {
            let params = ParamValues::from([("edition".into(), edition as f32)]);
            let mut a = image();
            let mut b = image();
            let mut c = image();
            Editions::default().apply(&mut a, &params, &context(0));
            Editions::default().apply(&mut b, &params, &context(0));
            Editions::default().apply(&mut c, &params, &context(45));
            assert_eq!(a.data, b.data);
            assert_ne!(a.data, c.data);
            assert!(results.insert(a.data));
        }
    }
}
