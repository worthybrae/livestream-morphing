//! Scene reconstruction studies. No frame history; final signature lives in Editions.
use super::RawFrame;

#[derive(Default)]
pub(super) struct Forms {
    size: (usize, usize),
    glass: Vec<(u32, u8)>,
    sites: Vec<(usize, usize, f32)>,
    knife: super::knife_layers::KnifeLayers,
}
#[inline(always)]
fn hash(x: usize, y: usize) -> f32 {
    let mut v = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263);
    v = (v ^ (v >> 13)).wrapping_mul(1274126177);
    ((v ^ (v >> 16)) & 65535) as f32 / 65535.
}
fn blend(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0., 1.);
    std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t)
}
fn put(frame: &mut RawFrame, i: usize, c: [f32; 3]) {
    for k in 0..3 {
        frame.data[i * 3 + k] = c[k].clamp(0., 255.) as u8;
    }
}
fn spectrum(t: f32) -> [f32; 3] {
    let colors = [
        [18., 36., 83.],
        [12., 153., 155.],
        [161., 222., 181.],
        [244., 187., 92.],
        [235., 76., 93.],
        [103., 44., 139.],
    ];
    let p = t.rem_euclid(1.) * 6.;
    blend(
        colors[p as usize % 6],
        colors[(p as usize + 1) % 6],
        p.fract(),
    )
}
impl Forms {
    fn prepare_glass(&mut self, w: usize, h: usize) {
        if self.size == (w, h) {
            return;
        }
        self.size = (w, h);
        self.glass.clear();
        self.sites.clear();
        let cell = (w as f32 / 1920. * 38.).max(4.);
        let cols = (w as f32 / cell).ceil() as usize + 2;
        let rows = (h as f32 / cell).ceil() as usize + 2;
        for gy in 0..rows {
            for gx in 0..cols {
                let x = ((gx as f32 - 0.5 + hash(gx, gy) * 0.85) * cell).clamp(0., (w - 1) as f32)
                    as usize;
                let y = ((gy as f32 - 0.5 + hash(gy + 71, gx + 29) * 0.85) * cell)
                    .clamp(0., (h - 1) as f32) as usize;
                self.sites.push((x, y, hash(gx + 911, gy + 53)));
            }
        }
        self.glass.reserve(w * h);
        for y in 0..h {
            for x in 0..w {
                let gx = (x as f32 / cell + 0.5) as usize;
                let gy = (y as f32 / cell + 0.5) as usize;
                let (mut near, mut second, mut id) = (f32::MAX, f32::MAX, 0);
                for yy in gy.saturating_sub(1)..=(gy + 1).min(rows - 1) {
                    for xx in gx.saturating_sub(1)..=(gx + 1).min(cols - 1) {
                        let index = yy * cols + xx;
                        let (sx, sy, _) = self.sites[index];
                        let d = (x as f32 - sx as f32).powi(2) + (y as f32 - sy as f32).powi(2);
                        if d < near {
                            second = near;
                            near = d;
                            id = index;
                        } else if d < second {
                            second = d;
                        }
                    }
                }
                // Distance difference gives a beveled edge without per-frame cell searches.
                let edge = ((second.sqrt() - near.sqrt()) / cell * 255.).clamp(0., 255.) as u8;
                self.glass.push((id as u32, edge));
            }
        }
    }
    pub(super) fn draw(
        &mut self,
        frame: &mut RawFrame,
        luma: &[f32],
        edition: u32,
        seconds: f32,
        environment: [f32; 3],
    ) {
        let w = frame.width as usize;
        let h = frame.height as usize;
        let scale = w as f32 / 1920.;
        match edition {
            22 => {
                self.prepare_glass(w, h);
                let colors: Vec<_> = self
                    .sites
                    .iter()
                    .map(|&(x, y, n)| {
                        let l = luma[y * w + x];
                        spectrum(l * 0.63 + n * 0.16 + seconds / 96.)
                    })
                    .collect();
                for i in 0..w * h {
                    let (id, edge) = self.glass[i];
                    let e = edge as f32 / 255.;
                    let l = luma[i];
                    let c = blend([3., 8., 18.], colors[id as usize], 0.22 + l * 1.15);
                    let c = if e < 0.025 {
                        [9., 12., 22.]
                    } else if e < 0.11 {
                        blend(c, [255., 229., 174.], (1. - (e - 0.025) / 0.085) * 0.8)
                    } else {
                        c
                    };
                    put(frame, i, c);
                }
            }
            23 => {
                // Opaque ribbons drawn back to front. Source light sculpts their silhouette.
                for p in frame.data.chunks_exact_mut(3) {
                    p.copy_from_slice(&[9, 8, 24]);
                }
                let step = (18. * scale).max(3.) as usize;
                for row in (0..h + step * 5).step_by(step) {
                    let sample_y = row.min(h - 1);
                    let color = spectrum(row as f32 / h as f32 * 0.8 + seconds / 90.);
                    let breath = (seconds * 0.38 + row as f32 / h as f32 * 5.).sin();
                    for x in 0..w {
                        let l = luma[sample_y * w + x];
                        let elevation = l.powi(2) * (105. + breath * 9.) * scale;
                        let top = row as f32 - elevation;
                        let depth = (5. + l * 12.) * scale;
                        let slope = luma[sample_y * w + (x + 2).min(w - 1)]
                            - luma[sample_y * w + x.saturating_sub(2)];
                        for yy in top.floor().max(0.) as usize
                            ..=((top + depth).ceil() as usize).min(h - 1)
                        {
                            let v = (yy as f32 - top) / depth.max(1.);
                            let edge = (1. - v / 0.18).max(0.);
                            let shade = blend(
                                [8., 10., 29.],
                                color,
                                0.48 + l * 0.6 - v * 0.35 + slope * 3.,
                            );
                            put(
                                frame,
                                yy * w + x,
                                blend(shade, [250., 222., 192.], edge * 0.7),
                            );
                        }
                    }
                }
            }
            24 => {
                let colors = [
                    [20., 26., 49.],
                    [36., 72., 176.],
                    [222., 66., 52.],
                    [238., 153., 162.],
                    [238., 188., 74.],
                    [246., 231., 200.],
                ];
                let thresholds = [0.07, 0.15, 0.24, 0.37, 0.55];
                let offsets: Vec<_> = (0..5)
                    .map(|n| {
                        let phase = seconds * 0.32 + n as f32 * 1.7;
                        (
                            (phase.sin() * 5. * scale) as isize,
                            (phase.cos() * 4. * scale) as isize,
                        )
                    })
                    .collect();
                let shadow = (5. * scale).max(1.) as usize;
                for y in 0..h {
                    for x in 0..w {
                        let rough = (hash(x / 3, y / 3) - 0.5) * 0.014;
                        let mut c = colors[0];
                        for n in 0..5 {
                            let (ox, oy) = offsets[n];
                            let sx = (x as isize + ox).clamp(0, w as isize - 1) as usize;
                            let sy = (y as isize + oy).clamp(0, h as isize - 1) as usize;
                            let sample = luma[sy * w + sx];
                            if sample + rough > thresholds[n] {
                                c = colors[n + 1];
                            } else if luma
                                [sy.saturating_sub(shadow) * w + sx.saturating_sub(shadow)]
                                + rough
                                > thresholds[n]
                            {
                                c = blend(c, [10., 13., 29.], 0.48);
                            }
                        }
                        let fiber = (hash(x, y) - 0.5) * 3.;
                        put(frame, y * w + x, std::array::from_fn(|k| c[k] + fiber));
                    }
                }
            }
            25 => {
                for p in frame.data.chunks_exact_mut(3) {
                    p.copy_from_slice(&[5, 9, 18]);
                }
                let spacing = (18. * scale).max(3.) as usize;
                let turn = seconds * 0.21;
                let drift = (turn.cos() * 0.32, turn.sin() * 0.32);
                for sy in (0..h).step_by(spacing) {
                    for sx in (0..w).step_by(spacing) {
                        let seed = hash(sx, sy);
                        let mut x = (sx as f32 + seed * spacing as f32).min((w - 1) as f32);
                        let mut y =
                            (sy as f32 + hash(sy + 97, sx) * spacing as f32).min((h - 1) as f32);
                        let l = luma[y as usize * w + x as usize];
                        let color = spectrum(l * 0.7 + seed * 0.13 + seconds / 120.);
                        let mut velocity = (0.8 + drift.0, 0.2 + drift.1);
                        for step in 0..26 {
                            let ix = x.clamp(1., (w - 2) as f32) as usize;
                            let iy = y.clamp(1., (h - 2) as f32) as usize;
                            let dx = luma[iy * w + ix + 1] - luma[iy * w + ix - 1];
                            let dy = luma[(iy + 1) * w + ix] - luma[(iy - 1) * w + ix];
                            // Follow a tangent to light contours, with inertia for long curved strokes.
                            velocity.0 = velocity.0 * 0.65 + (0.65 + drift.0 - dy * 28.) * 0.35;
                            velocity.1 = velocity.1 * 0.65 + (0.15 + drift.1 + dx * 28.) * 0.35;
                            let norm = (velocity.0 * velocity.0 + velocity.1 * velocity.1)
                                .sqrt()
                                .max(0.01);
                            let (vx, vy) = (
                                velocity.0 / norm * 2. * scale,
                                velocity.1 / norm * 2. * scale,
                            );
                            let alpha =
                                (1. - (step as f32 / 26. * 2. - 1.).abs()) * (0.25 + l * 0.75);
                            let radius = (0.8 + l * 0.8) * scale;
                            for sub in 0..3 {
                                let px = x + vx * sub as f32 / 3.;
                                let py = y + vy * sub as f32 / 3.;
                                let reach = radius.ceil() as isize;
                                for yy in py as isize - reach..=py as isize + reach {
                                    for xx in px as isize - reach..=px as isize + reach {
                                        if xx < 0 || yy < 0 || xx >= w as isize || yy >= h as isize
                                        {
                                            continue;
                                        }
                                        let distance = ((xx as f32 - px).powi(2)
                                            + (yy as f32 - py).powi(2))
                                        .sqrt();
                                        let cover = (radius + 0.5 - distance).clamp(0., 1.) * alpha;
                                        let i = yy as usize * w + xx as usize;
                                        let prior =
                                            std::array::from_fn(|k| frame.data[i * 3 + k] as f32);
                                        put(frame, i, blend(prior, color, cover));
                                    }
                                }
                            }
                            x += vx;
                            y += vy;
                            if x < 1. || x >= (w - 2) as f32 || y < 1. || y >= (h - 2) as f32 {
                                break;
                            }
                        }
                    }
                }
            }
            47 | 48 => self.knife.draw(frame, luma, seconds, edition == 48),
            45 | 46 => palette_knife(frame, luma, edition),
            26 | 27 => paper_relief(frame, luma, edition, seconds),
            32..=44 => paper_cutouts(frame, luma, edition, seconds, environment),
            28..=31 => filaments(frame, luma, edition, seconds),
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
fn sample(luma: &[f32], w: usize, h: usize, x: f32, y: f32) -> f32 {
    let x = x.clamp(0., (w - 1) as f32);
    let y = y.clamp(0., (h - 1) as f32);
    let ix = x as usize;
    let iy = y as usize;
    let tx = x - ix as f32;
    let ty = y - iy as f32;
    let jx = (ix + 1).min(w - 1);
    let jy = (iy + 1).min(h - 1);
    let a = luma[iy * w + ix] * (1. - tx) + luma[iy * w + jx] * tx;
    let b = luma[jy * w + ix] * (1. - tx) + luma[jy * w + jx] * tx;
    a * (1. - ty) + b * ty
}
// Fixed translations share their interpolation coordinates across every pixel in a row/column.
struct ShiftSample {
    x: Vec<(usize, usize, f32)>,
    y: Vec<(usize, usize, f32)>,
}
impl ShiftSample {
    fn new(w: usize, h: usize, ox: f32, oy: f32, dx: f32, dy: f32) -> Self {
        let axis = |len: usize, offset: f32, shift: f32, stride: usize| {
            (0..len)
                .map(|i| {
                    let p = ((i as f32 + offset) - shift).clamp(0., (len - 1) as f32);
                    let j = p as usize;
                    (j * stride, (j + 1).min(len - 1) * stride, p - j as f32)
                })
                .collect()
        };
        Self {
            x: axis(w, ox, dx, 1),
            y: axis(h, oy, dy, w),
        }
    }
    #[inline]
    fn get(&self, luma: &[f32], x: usize, y: usize) -> f32 {
        let (ix, jx, tx) = self.x[x];
        let (iy, jy, ty) = self.y[y];
        let a = luma[iy + ix] * (1. - tx) + luma[iy + jx] * tx;
        let b = luma[jy + ix] * (1. - tx) + luma[jy + jx] * tx;
        a * (1. - ty) + b * ty
    }
}
fn paper_relief(frame: &mut RawFrame, luma: &[f32], edition: u32, seconds: f32) {
    let w = frame.width as usize;
    let h = frame.height as usize;
    let scale = w as f32 / 1920.;
    let colors = if edition == 26 {
        [
            [20., 28., 55.],
            [41., 73., 181.],
            [221., 70., 56.],
            [232., 158., 164.],
            [248., 226., 181.],
        ]
    } else {
        [
            [18., 25., 36.],
            [31., 76., 87.],
            [77., 118., 117.],
            [207., 130., 69.],
            [245., 220., 174.],
        ]
    };
    let thresholds = [0.10, 0.22, 0.37, 0.55];
    let offsets: Vec<_> = (0..4)
        .map(|n| {
            let phase = seconds * 0.23 + n as f32 * 1.3;
            (
                phase.sin() * (n + 1) as f32 * 1.6 * scale,
                phase.cos() * (n + 1) as f32 * 1.2 * scale,
            )
        })
        .collect();
    let samplers: Vec<_> = offsets
        .iter()
        .enumerate()
        .map(|(n, &(ox, oy))| {
            let depth = (8. + n as f32 * 2.) * scale;
            [
                ShiftSample::new(w, h, ox, oy, 0., 0.),
                ShiftSample::new(w, h, ox, oy, depth, depth * 0.7),
                ShiftSample::new(w, h, ox, oy, depth * 0.6, depth * 0.42),
                ShiftSample::new(w, h, ox, oy, 1.5 * scale, 1.5 * scale),
            ]
        })
        .collect();
    for y in 0..h {
        for x in 0..w {
            let rough = (hash(x / 5, y / 5) - 0.5) * 0.003;
            let mut c = colors[0];
            for n in 0..4 {
                let maps = &samplers[n];
                let l = maps[0].get(luma, x, y) + rough;
                let coverage = ((l - thresholds[n]) / 0.006 + 0.5).clamp(0., 1.);
                if coverage < 1. {
                    let a = maps[1].get(luma, x, y);
                    let b = maps[2].get(luma, x, y);
                    let shadow = (((a - thresholds[n]) / 0.018 + 0.5).clamp(0., 1.)
                        + ((b - thresholds[n]) / 0.012 + 0.5).clamp(0., 1.))
                        * 0.23;
                    c = blend(c, [8., 12., 24.], shadow * (1. - coverage));
                }
                if coverage > 0. {
                    let above = maps[3].get(luma, x, y);
                    let rim = ((l - above) * 50.).clamp(0., 0.28);
                    c = blend(c, blend(colors[n + 1], [255., 243., 217.], rim), coverage);
                }
            }
            // Restrained, spatially fixed paper tooth, not frame-varying noise.
            let fiber = (hash(x / 2, y / 2) - 0.5) * 1.4;
            put(frame, y * w + x, std::array::from_fn(|k| c[k] + fiber));
        }
    }
}
// A single tonal stack gives bold, registered cutouts. Depth is confined to edges;
// the shared final warp supplies all motion without separating the paper layers.
fn garden_palette([hour, cloud, rain]: [f32; 3]) -> [[f32; 3]; 5] {
    let day = [
        [25., 39., 39.],
        [44., 83., 76.],
        [93., 146., 116.],
        [232., 157., 169.],
        [246., 233., 209.],
    ];
    let night = [
        [16., 22., 36.],
        [27., 48., 60.],
        [58., 88., 88.],
        [152., 117., 155.],
        [202., 205., 207.],
    ];
    let dusk = [
        [36., 30., 44.],
        [54., 74., 69.],
        [115., 143., 100.],
        [243., 157., 148.],
        [255., 226., 185.],
    ];
    // Clock-based artistic anchors, not calculated sunrise/sunset times.
    let anchors = [
        (0., night),
        (5., night),
        (8., day),
        (16., day),
        (19., dusk),
        (22., night),
        (24., night),
    ];
    let h = hour.clamp(0., 24.);
    let pair = anchors.windows(2).find(|p| h <= p[1].0).unwrap();
    let t = ((h - pair[0].0) / (pair[1].0 - pair[0].0)).clamp(0., 1.);
    let t = t * t * (3. - 2. * t);
    let overcast = [
        [25., 35., 44.],
        [49., 75., 82.],
        [106., 137., 129.],
        [211., 163., 181.],
        [231., 226., 215.],
    ];
    std::array::from_fn(|i| {
        let base = blend(pair[0].1[i], pair[1].1[i], t);
        let cloudy = blend(base, overcast[i], cloud.clamp(0., 1.) * 0.28);
        std::array::from_fn(|k| cloudy[k] * (1. - rain.clamp(0., 1.) * 0.09))
    })
}

#[cfg(test)]
mod garden_tests {
    use super::*;
    #[test]
    fn environment_is_continuous_and_changes_the_palette() {
        let day = garden_palette([12., 0., 0.]);
        assert_ne!(day, garden_palette([0., 0., 0.]));
        assert_ne!(day, garden_palette([12., 1., 1.]));
        assert_eq!(
            garden_palette([0., 0.4, 0.2]),
            garden_palette([24., 0.4, 0.2])
        );
        for h in [5., 8., 16., 19., 22.] {
            let a = garden_palette([h - 0.001, 0.5, 0.]);
            let b = garden_palette([h + 0.001, 0.5, 0.]);
            for i in 0..5 {
                for k in 0..3 {
                    assert!((a[i][k] - b[i][k]).abs() < 0.01);
                }
            }
        }
    }
}

// Smooth deterministic pigment field, elongated by the brush coordinate mapping.
#[inline(always)]
fn paint_noise(x: f32, y: f32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let tx = x - x.floor();
    let ty = y - y.floor();
    let tx = tx * tx * (3. - 2. * tx);
    let ty = ty * ty * (3. - 2. * ty);
    let h = |dx: i32, dy: i32| hash((ix + dx) as usize, (iy + dy) as usize);
    let a = h(0, 0) * (1. - tx) + h(1, 0) * tx;
    let b = h(0, 1) * (1. - tx) + h(1, 1) * tx;
    a * (1. - ty) + b * ty
}

fn paper_cutouts(
    frame: &mut RawFrame,
    luma: &[f32],
    edition: u32,
    seconds: f32,
    environment: [f32; 3],
) {
    let w = frame.width as usize;
    let h = frame.height as usize;
    let scale = w as f32 / 1920.;
    let colors = if edition == 41 {
        garden_palette(environment)
    } else if edition == 37 {
        [
            [20., 22., 40.],
            [67., 42., 124.],
            [173., 84., 180.],
            [218., 236., 93.],
            [251., 246., 205.],
        ]
    } else if edition == 38 {
        [
            [21., 29., 62.],
            [42., 69., 132.],
            [86., 137., 180.],
            [161., 209., 211.],
            [238., 235., 213.],
        ]
    } else if edition == 39 {
        [
            [43., 24., 39.],
            [105., 42., 69.],
            [192., 76., 58.],
            [237., 162., 75.],
            [251., 230., 171.],
        ]
    } else if edition == 40 || edition >= 42 {
        [
            [25., 39., 39.],
            [44., 83., 76.],
            [93., 146., 116.],
            [232., 157., 169.],
            [246., 233., 209.],
        ]
    } else if edition == 34 {
        [
            [25., 42., 53.],
            [54., 108., 126.],
            [204., 99., 66.],
            [227., 173., 137.],
            [245., 231., 199.],
        ]
    } else {
        [
            [19., 27., 49.],
            [37., 72., 180.],
            [220., 65., 49.],
            [235., 155., 148.],
            [246., 225., 184.],
        ]
    };
    let thresholds = [0.10, 0.22, 0.37, 0.55];
    let (dx, dy, strength) = if edition == 35 {
        // Light travels slowly; geometry and the signature warp remain untouched.
        let phase = seconds * std::f32::consts::TAU / 24.;
        (phase.cos() * 6., phase.sin() * 4.5, 0.26)
    } else if edition == 33 || edition >= 36 {
        (6., 4.5, 0.28)
    } else {
        (4., 3., 0.22)
    };
    let shadow = ShiftSample::new(w, h, 0., 0., dx * scale, dy * scale);
    let rim = ShiftSample::new(w, h, 0., 0., -dx * 0.3 * scale, -dy * 0.3 * scale);
    if edition >= 36 {
        // Classify before translating: a fixed luminance blend band can cover many
        // pixels on low-contrast slopes. Discrete paper masks have no such haze.
        let layers: Vec<f32> = luma
            .iter()
            .map(|l| thresholds.iter().filter(|t| l >= *t).count() as f32)
            .collect();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let layer = layers[i];
                let shade = (shadow.get(&layers, x, y) - layer).clamp(0., 1.) * strength;
                let edge = (layer - rim.get(&layers, x, y)).clamp(0., 1.) * 0.16;
                let c = blend(
                    blend(colors[layer as usize], [10., 15., 29.], shade),
                    [255., 242., 213.],
                    edge,
                );
                let c = if edition == 42 || edition == 43 {
                    let sx = x as f32 / scale;
                    let sy = y as f32 / scale;
                    // Broad diagonal pulls across the foreground, finer marks above.
                    // Source luminance modulates pigment, without temporal randomness.
                    let breadth = 0.65 + sy / 1080. * 0.85;
                    let u = (sx * 0.91 + sy * 0.41) / breadth;
                    let v = (sy * 0.91 - sx * 0.41) / breadth;
                    let load = paint_noise(u / 68., v / 13. + layer * 19.);
                    let ridge = paint_noise(u / 45., v / 2.8 + layer * 31.);
                    let slope = paint_noise(u / 45., (v + 1.4) / 2.8 + layer * 31.) - ridge;
                    let broken = paint_noise(u / 19. + 73., v / 7.);
                    let amount = if edition == 43 { 1.4 } else { 0.85 };
                    let pigment = (load - 0.5) * 15. + (broken - 0.5) * 4.;
                    let relief = slope * 32. * (0.3 + load * 0.7);
                    let tone = (luma[i] - [0.05, 0.16, 0.295, 0.46, 0.68][layer as usize])
                        .clamp(-0.1, 0.1)
                        * 30.;
                    std::array::from_fn(|k| c[k] + amount * (pigment + relief + tone))
                } else {
                    c
                };
                let fiber = (hash(x / 2, y / 2) - 0.5) * 0.65;
                put(frame, i, std::array::from_fn(|k| c[k] + fiber));
            }
        }
        if edition == 44 {
            painted_dabs(frame, luma, &layers);
        }
        // The final shared warp supplies spatial antialiasing on the hard masks.
        return;
    }
    // Continuous layer coordinate also antialiases the cuts and their shadows.
    let level = |l: f32| -> f32 {
        thresholds
            .iter()
            .map(|t| ((l - t) / 0.006 + 0.5).clamp(0., 1.))
            .sum()
    };
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let layer = level(luma[i]);
            let n = (layer as usize).min(3);
            let c = blend(colors[n], colors[n + 1], layer - n as f32);
            let shade = (level(shadow.get(luma, x, y)) - layer).clamp(0., 1.) * strength;
            let edge = (layer - level(rim.get(luma, x, y))).clamp(0., 1.) * 0.16;
            let c = blend(blend(c, [10., 15., 29.], shade), [255., 242., 213.], edge);
            let fiber = (hash(x / 2, y / 2) - 0.5) * 0.65;
            put(frame, i, std::array::from_fn(|k| c[k] + fiber));
        }
    }
}
// Opaque angular deposits construct the image, without paper-layer masks.
fn palette_knife(frame: &mut RawFrame, luma: &[f32], edition: u32) {
    let w = frame.width as usize;
    let h = frame.height as usize;
    let scale = w as f32 / 1920.;
    let palette = [
        [23., 43., 44.],
        [48., 91., 79.],
        [113., 153., 116.],
        [224., 155., 170.],
        [247., 229., 194.],
    ];
    let color_at = |l: f32| {
        let z = (l * 5.8).clamp(0., 4.);
        let n = (z as usize).min(3);
        blend(palette[n], palette[n + 1], z - n as f32)
    };
    for i in 0..w * h {
        let c = color_at(luma[i]);
        put(frame, i, blend(c, [73., 65., 77.], 0.25));
    }
    let sizes = if edition == 45 {
        [48., 25., 12.]
    } else {
        [36., 18., 8.]
    };
    for (coat, size) in sizes.iter().enumerate() {
        let step = (size * scale).max(2.) as usize;
        for gy in (0..h).step_by(step) {
            for gx in (0..w).step_by(step) {
                let seed = hash(gx + coat * 791, gy + 29);
                let cx = (gx as f32 + (seed - 0.5) * step as f32).clamp(0., (w - 1) as f32);
                let cy =
                    (gy as f32 + (hash(gx + 83, gy) - 0.5) * step as f32).clamp(0., (h - 1) as f32);
                let x = cx as usize;
                let y = cy as usize;
                let i = y * w + x;
                let d = step.max(2);
                let dx = luma[y * w + (x + d).min(w - 1)] - luma[y * w + x.saturating_sub(d)];
                let dy = luma[(y + d).min(h - 1) * w + x] - luma[y.saturating_sub(d) * w + x];
                let gradient = dx.abs() + dy.abs();
                if coat == 2 && gradient < 0.065 {
                    continue;
                }
                let angle = if gradient > 0.03 {
                    dy.atan2(dx) + 1.57 + (seed - 0.5) * 0.45
                } else {
                    (seed - 0.5) * 2.6
                };
                let (sn, cs) = angle.sin_cos();
                let length = (size * scale).max(1.) * (0.8 + seed * 0.75);
                let width = (size * scale).max(1.) * (0.35 + hash(gx, gy + 41) * 0.25);
                let bx = cs.abs() * length + sn.abs() * width;
                let by = sn.abs() * length + cs.abs() * width;
                let c = color_at(luma[i]);
                let tint = (seed - 0.5) * 16.;
                // Clip scanlines to the convex knife face, retaining a two-pixel
                // guard for floating-point boundary rounding. Pixel math is unchanged.
                let ua = cs as f64 / length as f64;
                let ub = sn as f64 / length as f64;
                let va = -(sn as f64) / width as f64;
                let vb = cs as f64 / width as f64;
                let planes = [
                    (0.2 * ua + va, 0.2 * ub + vb, 0.9),
                    (0.2 * ua - va, 0.2 * ub - vb, 0.9),
                    (ua + 0.18 * va, ub + 0.18 * vb, 1.),
                    (ua - 0.18 * va, ub - 0.18 * vb, 1.),
                    (-ua + 0.18 * va, -ub + 0.18 * vb, 1.),
                    (-ua - 0.18 * va, -ub - 0.18 * vb, 1.),
                ];
                for yy in (cy - by).max(0.) as usize..=(cy + by).min((h - 1) as f32) as usize {
                    let mut left = -bx as f64;
                    let mut right = bx as f64;
                    for &(a, b, c) in &planes {
                        if a.abs() > 1e-12 {
                            let edge = (c - b * (yy as f64 - cy as f64)) / a;
                            if a > 0. {
                                right = right.min(edge);
                            } else {
                                left = left.max(edge);
                            }
                        }
                    }
                    let start =
                        (cx as f64 + left - 2.).floor().max(0.).min((w - 1) as f64) as usize;
                    let end = (cx as f64 + right + 2.).ceil().max(0.).min((w - 1) as f64) as usize;
                    let start = start.max((cx - bx).max(0.) as usize);
                    let end = end.min((cx + bx).min((w - 1) as f32) as usize);
                    if start > end {
                        continue;
                    }
                    let row = &mut frame.data[(yy * w + start) * 3..(yy * w + end + 1) * 3];
                    for (batch, pixels) in row.chunks_mut(12).enumerate() {
                        knife_pixels(
                            pixels,
                            start + batch * 4,
                            yy,
                            cx,
                            cy,
                            cs,
                            sn,
                            length,
                            width,
                            seed,
                            coat,
                            tint,
                            c,
                        );
                    }
                }
            }
        }
    }
}

// Explicit four-lane SIMD keeps pigment hashing, lighting, and coverage packed
// on both ARM and x86. No fast-math or approximate reciprocal is used.
#[inline]
fn knife_pixels(
    pixels: &mut [u8],
    x: usize,
    y: usize,
    cx: f32,
    cy: f32,
    cs: f32,
    sn: f32,
    length: f32,
    width: f32,
    seed: f32,
    coat: usize,
    tint: f32,
    c: [f32; 3],
) {
    use wide::{f32x4 as F, CmpGt, CmpLt};
    let ox = F::new(std::array::from_fn(|k| (x + k) as f32 - cx));
    let oy = y as f32 - cy;
    let u = (ox * cs + F::splat(oy * sn)) / F::splat(length);
    let v = (-ox * sn + F::splat(oy * cs)) / F::splat(width);
    let distance = (v.abs() - (F::splat(0.9) - u * 0.2)).max(u.abs() + v.abs() * 0.18 - 1.);
    // The conservative scanline guard also produces wholly empty batches.
    // Reject those before evaluating pigment or reading destination colors.
    if distance.cmp_lt(F::ZERO).none() {
        return;
    }
    let cover = (-distance * width).max(F::ZERO).min(F::ONE) * 0.94;
    let scrape = knife_noise4(u * 4. + seed * 19., v * 13. + coat as f32 * 37.);
    let ridge = v.cmp_gt(F::splat(0.58)).blend(
        (v - 0.58) * 35.,
        v.cmp_lt(F::splat(-0.65)).blend(F::splat(-9.), F::ZERO),
    );
    let light = F::splat(tint) + ridge + (scrape - 0.5) * 9.;
    let mut colors = [[0.; 4]; 3];
    for channel in 0..3 {
        let old = F::new(std::array::from_fn(|k| {
            pixels.get(k * 3 + channel).copied().unwrap_or(0) as f32
        }));
        colors[channel] = (old + (F::splat(c[channel]) + light - old) * cover)
            .max(F::ZERO)
            .min(F::splat(255.))
            .to_array();
    }
    for (k, rgb) in pixels.chunks_exact_mut(3).enumerate() {
        for channel in 0..3 {
            rgb[channel] = colors[channel][k] as u8;
        }
    }
}

#[inline]
fn knife_noise4(x: wide::f32x4, y: wide::f32x4) -> wide::f32x4 {
    use wide::{f32x4 as F, u32x4 as U};
    let floor_x = x.floor();
    let floor_y = y.floor();
    let ix = U::new(floor_x.trunc_int().to_array().map(|v| v as u32));
    let iy = U::new(floor_y.trunc_int().to_array().map(|v| v as u32));
    let tx = x - floor_x;
    let ty = y - floor_y;
    let tx = tx * tx * (F::splat(3.) - tx * 2.);
    let ty = ty * ty * (F::splat(3.) - ty * 2.);
    let h = |dx: u32, dy: u32| {
        let v = ((ix + U::splat(dx)) * U::splat(374761393))
            ^ ((iy + U::splat(dy)) * U::splat(668265263));
        let v = (v ^ (v >> 13u32)) * U::splat(1274126177);
        let v = (v ^ (v >> 16u32)) & U::splat(65535);
        F::new(v.to_array().map(|v| v as f32)) / F::splat(65535.)
    };
    let a = h(0, 0) * (F::ONE - tx) + h(1, 0) * tx;
    let b = h(0, 1) * (F::ONE - tx) + h(1, 1) * tx;
    a * (F::ONE - ty) + b * ty
}

// Two coats of separately deposited paint. Marks are clipped to the paper masks,
// and their orientation follows coarse source gradients with a stable seed offset.
fn painted_dabs(frame: &mut RawFrame, luma: &[f32], layers: &[f32]) {
    let w = frame.width as usize;
    let h = frame.height as usize;
    let scale = w as f32 / 1920.;
    let base = frame.data.clone();
    for (coat, spacing) in [24., 12.].iter().enumerate() {
        let step = (spacing * scale).max(3.) as usize;
        for gy in (0..h).step_by(step) {
            for gx in (0..w).step_by(step) {
                let seed = hash(gx + coat * 131, gy + 67);
                let cx = (gx as f32 + (seed - 0.5) * step as f32).clamp(0., (w - 1) as f32);
                let cy =
                    (gy as f32 + (hash(gx + 97, gy) - 0.5) * step as f32).clamp(0., (h - 1) as f32);
                let ix = cx as usize;
                let iy = cy as usize;
                let center = iy * w + ix;
                let d = step;
                let dx = luma[iy * w + (ix + d).min(w - 1)] - luma[iy * w + ix.saturating_sub(d)];
                let dy = luma[(iy + d).min(h - 1) * w + ix] - luma[iy.saturating_sub(d) * w + ix];
                let angle = if dx.abs() + dy.abs() > 0.025 {
                    dy.atan2(dx) + 1.57 + (seed - 0.5) * 0.9
                } else {
                    seed * 6.283
                };
                let (sn, cs) = angle.sin_cos();
                let length = step as f32 * (1.05 + seed * 0.8);
                let width = step as f32 * (0.36 + hash(gx, gy + 43) * 0.28);
                let bound = length + width;
                let pigment = (seed - 0.5) * 22.;
                let color: [f32; 3] =
                    std::array::from_fn(|k| base[center * 3 + k] as f32 + pigment);
                for y in
                    ((cy - bound).max(0.) as usize)..=((cy + bound).min((h - 1) as f32) as usize)
                {
                    for x in ((cx - bound).max(0.) as usize)
                        ..=((cx + bound).min((w - 1) as f32) as usize)
                    {
                        let i = y * w + x;
                        if layers[i] != layers[center] {
                            continue;
                        }
                        let ox = x as f32 - cx;
                        let oy = y as f32 - cy;
                        let u = (ox * cs + oy * sn) / length;
                        let v = (-ox * sn + oy * cs) / width;
                        let irregular =
                            0.09 * ((u * 9. + seed * 12.).sin()) * (1. - u.abs()).max(0.);
                        let q = u * u + (v + irregular) * (v + irregular);
                        if q >= 1. {
                            continue;
                        }
                        let rim = ((1. - q) * 10.).clamp(0., 1.);
                        let broken = 0.65
                            + 0.35 * paint_noise(u * 3. + seed * 37., v * 5. + coat as f32 * 19.);
                        let opacity = rim * broken * if coat == 0 { 0.65 } else { 0.42 };
                        let ridge =
                            ((v * 18. + u * 2. + seed * 20.).sin() * 0.6 + v * 3.) * (1. - q) * 2.;
                        // Pigment changes across each stroke; a lit lip gives the deposit body.
                        let light = (-v * 7. - u * 2. + ridge) * (1. - q).sqrt();
                        let old = std::array::from_fn(|k| frame.data[i * 3 + k] as f32);
                        put(
                            frame,
                            i,
                            blend(old, std::array::from_fn(|k| color[k] + light), opacity),
                        );
                    }
                }
            }
        }
    }
}

// One antialiased capsule per integration step, replacing overlapping disc stamps.
fn stroke(
    frame: &mut RawFrame,
    a: (f32, f32),
    b: (f32, f32),
    radius: f32,
    color: [f32; 3],
    opacity: f32,
) {
    let w = frame.width as usize;
    let h = frame.height as usize;
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let inv = 1. / (vx * vx + vy * vy).max(0.0001);
    let outer = radius + 0.6;
    let inner = (radius - 0.6).max(0.);
    let band = 1. / (outer * outer - inner * inner).max(0.01);
    let left = (a.0.min(b.0) - outer).floor().max(0.) as usize;
    let right = (a.0.max(b.0) + outer).ceil().min((w - 1) as f32) as usize;
    let top = (a.1.min(b.1) - outer).floor().max(0.) as usize;
    let bottom = (a.1.max(b.1) + outer).ceil().min((h - 1) as f32) as usize;
    for y in top..=bottom {
        for x in left..=right {
            let t = (((x as f32 - a.0) * vx + (y as f32 - a.1) * vy) * inv).clamp(0., 1.);
            let dx = x as f32 - a.0 - t * vx;
            let dy = y as f32 - a.1 - t * vy;
            let coverage = ((outer * outer - dx * dx - dy * dy) * band).clamp(0., 1.) * opacity;
            if coverage > 0. {
                let i = y * w + x;
                let old = std::array::from_fn(|k| frame.data[i * 3 + k] as f32);
                put(frame, i, blend(old, color, coverage));
            }
        }
    }
}
// Independent phase per strand; smooth oscillations have no resets or accumulated history.
fn strand_motion(seed: f32, along: f32, seconds: f32, edition: u32) -> (f32, f32, f32) {
    let phase = seed * std::f32::consts::TAU;
    let expressive = edition == 31;
    let speed = if expressive { 1.15 } else { 0.8 };
    let sway = (seconds * speed * 0.63 + phase).sin() * (if expressive { 5. } else { 3. });
    let ripple = (seconds * speed + phase + along * std::f32::consts::TAU * 1.2).sin()
        * (if expressive { 11. } else { 6. });
    let breath = 0.5
        + 0.5 * (seconds * speed * 0.84 + phase * 1.7 - along * std::f32::consts::TAU * 0.9).sin();
    let width = 0.45 + breath * (if expressive { 2.7 } else { 1.7 });
    (sway, ripple, width)
}
fn filaments(frame: &mut RawFrame, luma: &[f32], edition: u32, seconds: f32) {
    let w = frame.width as usize;
    let h = frame.height as usize;
    let scale = w as f32 / 1920.;
    for p in frame.data.chunks_exact_mut(3) {
        p.copy_from_slice(&[7, 13, 22]);
    }
    let animated = edition >= 30;
    let silk = edition == 29 || animated;
    let spacing = ((if edition == 31 {
        40.
    } else if animated {
        32.
    } else if silk {
        30.
    } else {
        22.
    }) * scale)
        .max(3.) as usize;
    let span = (3. * scale).max(1.) as usize;
    // Smooth orientation fields are evaluated once per coordinate, not per stroke step.
    let horizontal: Vec<_> = (0..w)
        .map(|x| (x as f32 / w as f32 * 5. + seconds * 0.12).sin())
        .collect();
    let vertical: Vec<_> = (0..h)
        .map(|y| (y as f32 / h as f32 * 6. - seconds * 0.10).cos())
        .collect();
    for sy in (0..h).step_by(spacing) {
        for sx in (0..w).step_by(spacing) {
            let seed = hash(sx + 19, sy + 53);
            let mut x = (sx as f32 + seed * spacing as f32).min((w - 2) as f32);
            let mut y = (sy as f32 + hash(sy + 97, sx) * spacing as f32).min((h - 2) as f32);
            let l = luma[y as usize * w + x as usize];
            // Thin the dark background continuously instead of popping strokes on/off.
            let strength = ((l - 0.025) * 3.).clamp(0.035, 1.);
            let color = if silk {
                blend(
                    [45., 112., 120.],
                    [239., 180., 132.],
                    (l * 1.5 + seed * 0.2).clamp(0., 1.),
                )
            } else {
                blend(
                    [60., 120., 143.],
                    [247., 217., 170.],
                    (l * 1.7).clamp(0., 1.),
                )
            };
            let steps = if silk {
                38 + (seed * 28.) as usize
            } else {
                12 + (seed * 25.) as usize
            };
            let mut velocity = (vertical[y as usize] * 0.7, horizontal[x as usize] * 0.7);
            let mut previous_drawn = None;
            for step in 0..steps {
                let ix = x.clamp(0., (w - 1) as f32) as usize;
                let iy = y.clamp(0., (h - 1) as f32) as usize;
                let dx =
                    luma[iy * w + (ix + span).min(w - 1)] - luma[iy * w + ix.saturating_sub(span)];
                let dy =
                    luma[(iy + span).min(h - 1) * w + ix] - luma[iy.saturating_sub(span) * w + ix];
                let target = (
                    -dy * 14. + vertical[iy] * 0.5,
                    dx * 14. + horizontal[ix] * 0.5,
                );
                velocity.0 = velocity.0 * 0.72 + target.0 * 0.28;
                velocity.1 = velocity.1 * 0.72 + target.1 * 0.28;
                let norm = (velocity.0 * velocity.0 + velocity.1 * velocity.1)
                    .sqrt()
                    .max(0.04);
                let next = (
                    x + velocity.0 / norm * 2.4 * scale,
                    y + velocity.1 / norm * 2.4 * scale,
                );
                let t = step as f32 / steps as f32;
                let taper = (t * 8.).min((1. - t) * 6.).min(1.).max(0.);
                let pulse = (t - seconds / 12. - seed).rem_euclid(1.);
                let head = (1. - (pulse - 0.5).abs() * 5.).max(0.);
                let radius = (if animated {
                    strand_motion(seed, t, seconds, edition).2
                } else if silk {
                    0.8 + head * 0.7
                } else {
                    0.6 + l * 0.7
                }) * scale
                    * taper;
                let (draw_a, draw_b) = if animated {
                    let normal = (-velocity.1 / norm, velocity.0 / norm);
                    let offset = |fraction: f32| {
                        let (sway, ripple, _) = strand_motion(seed, fraction, seconds, edition);
                        (
                            normal.0 * ripple * scale + sway * scale,
                            normal.1 * ripple * scale + sway * 0.55 * scale,
                        )
                    };
                    let a = offset(t);
                    let b = offset((step + 1) as f32 / steps as f32);
                    let start = previous_drawn.unwrap_or((x + a.0, y + a.1));
                    let end = (next.0 + b.0, next.1 + b.1);
                    previous_drawn = Some(end);
                    (start, end)
                } else {
                    ((x, y), next)
                };
                stroke(
                    frame,
                    draw_a,
                    draw_b,
                    radius,
                    color,
                    strength * taper * (if animated { 0.85 } else { 0.5 + head * 0.45 }),
                );
                x = next.0;
                y = next.1;
                if x < 1. || y < 1. || x >= (w - 2) as f32 || y >= (h - 2) as f32 {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod sampling_tests {
    use super::*;
    #[test]
    fn strand_motion_changes_position_and_width_smoothly_with_independent_phases() {
        for edition in [30, 31] {
            let a = strand_motion(0.2, 0.4, 1., edition);
            let b = strand_motion(0.2, 0.4, 1.001, edition);
            assert_ne!(a, b);
            assert!(
                (a.0 - b.0).abs() < 0.02 && (a.1 - b.1).abs() < 0.02 && (a.2 - b.2).abs() < 0.02
            );
            assert_ne!(a, strand_motion(0.7, 0.4, 1., edition));
            assert_ne!(a, strand_motion(0.2, 0.4, 2., edition));
            for i in 0..500 {
                let (_, _, width) = strand_motion(0.2, 0.4, i as f32 * 0.1, edition);
                assert!((0.45..=3.16).contains(&width));
            }
        }
    }
    #[test]
    fn precomputed_translations_preserve_bilinear_samples_at_borders_and_fractional_offsets() {
        let (w, h) = (19, 13);
        let data: Vec<_> = (0..w * h).map(|i| (i * 37 % 251) as f32 / 251.).collect();
        for (ox, oy, dx, dy) in [
            (0., 0., 0., 0.),
            (2.3, -1.7, 9.1, 5.4),
            (-8.2, 4.1, 0.3, 0.7),
            (22., -20., 1.5, 1.5),
        ] {
            let map = ShiftSample::new(w, h, ox, oy, dx, dy);
            for y in 0..h {
                for x in 0..w {
                    assert_eq!(
                        map.get(&data, x, y),
                        sample(&data, w, h, x as f32 + ox - dx, y as f32 + oy - dy)
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod knife_batch_tests {
    use super::*;
    #[test]
    fn batched_pixels_match_scalar_at_edges_and_partial_rows() {
        for trial in 0..256 {
            let seed = hash(trial, 731);
            let angle = seed * 6.283;
            let (sn, cs) = angle.sin_cos();
            let length = 2. + seed * 55.;
            let width = 1. + hash(trial, 17) * 25.;
            let cx = 16. + seed;
            let cy = 12. + hash(trial, 31);
            let coat = trial % 3;
            let tint = (seed - 0.5) * 16.;
            let c = [113., 153., 116.];
            for count in 1..=4 {
                for y in 0..25 {
                    for x in (0..32).step_by(8) {
                        let mut actual: Vec<u8> = (0..count * 3)
                            .map(|k| ((k * 71 + trial * 13) % 256) as u8)
                            .collect();
                        let mut expected = actual.clone();
                        knife_pixels(
                            &mut actual,
                            x,
                            y,
                            cx,
                            cy,
                            cs,
                            sn,
                            length,
                            width,
                            seed,
                            coat,
                            tint,
                            c,
                        );
                        for k in 0..count {
                            let ox = (x + k) as f32 - cx;
                            let oy = y as f32 - cy;
                            let u = (ox * cs + oy * sn) / length;
                            let v = (-ox * sn + oy * cs) / width;
                            let distance =
                                (v.abs() - (0.9 - 0.2 * u)).max(u.abs() + v.abs() * 0.18 - 1.);
                            if distance >= 0. {
                                continue;
                            }
                            let cover = (-distance * width).clamp(0., 1.);
                            let scrape =
                                paint_noise(u * 4. + seed * 19., v * 13. + coat as f32 * 37.);
                            let ridge = if v > 0.58 {
                                (v - 0.58) * 35.
                            } else if v < -0.65 {
                                -9.
                            } else {
                                0.
                            };
                            let light = tint + ridge + (scrape - 0.5) * 9.;
                            let old =
                                std::array::from_fn(|channel| expected[k * 3 + channel] as f32);
                            let color = blend(old, c.map(|v| v + light), cover * 0.94);
                            for channel in 0..3 {
                                expected[k * 3 + channel] = color[channel].clamp(0., 255.) as u8;
                            }
                        }
                        assert_eq!(actual, expected, "trial={trial} count={count} x={x} y={y}");
                    }
                }
            }
        }
    }
}
