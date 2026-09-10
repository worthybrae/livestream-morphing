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
            45 | 46 => palette_knife::<0>(frame, luma, edition, environment),
            49 => palette_knife::<1>(frame, luma, edition, environment),
            50 => palette_knife::<2>(frame, luma, edition, environment),
            51 => palette_knife::<3>(frame, luma, edition, environment),
            52 => palette_knife::<4>(frame, luma, edition, environment),
            53 => palette_knife::<5>(frame, luma, edition, environment),
            54 => palette_knife::<6>(frame, luma, edition, environment),
            55 => palette_knife::<7>(frame, luma, edition, environment),
            56 => palette_knife::<8>(frame, luma, edition, environment),
            57 => palette_knife::<9>(frame, luma, edition, environment),
            58 => palette_knife::<10>(frame, luma, edition, environment),
            59 => palette_knife::<11>(frame, luma, edition, environment),
            60 => palette_knife::<12>(frame, luma, edition, environment),
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
// Smooth, circular London clock palette. Computed once per frame, independent
// of the painting/warp phase. These are artistic clock anchors, not solar events.
fn knife_day_tint([hour, cloud, rain]: [f32; 3]) -> [f32; 3] {
    let stops = [
        (0., [-13., -5., 27.]),
        (4., [-13., -5., 27.]),
        (6.5, [32., 3., 12.]),
        (10., [15., 12., -9.]),
        (15., [15., 12., -9.]),
        (18.5, [37., 1., -16.]),
        (21., [8., -14., 30.]),
        (24., [-13., -5., 27.]),
    ];
    let hour = hour.rem_euclid(24.);
    let pair = stops.windows(2).find(|p| hour < p[1].0).unwrap();
    let t = (hour - pair[0].0) / (pair[1].0 - pair[0].0);
    let base = blend(pair[0].1, pair[1].1, t * t * (3. - 2. * t));
    let cloud = cloud.clamp(0., 1.);
    let rain = rain.clamp(0., 1.);
    let overcast = [-5., 1., 10.];
    let wet = [-5., -1., 8.];
    std::array::from_fn(|k| base[k] * (1. - cloud * 0.22)
        + overcast[k] * cloud * 0.45 + wet[k] * rain)
}

fn knife_night_strength(hour: f32) -> f32 {
    let hour = hour.rem_euclid(24.);
    let smooth = |t: f32| { let t = t.clamp(0., 1.); t*t*(3.-2.*t) };
    1. - smooth((hour - 4.5) / 3.5) + smooth((hour - 18.) / 3.)
}

// A hue-preserving soft shoulder, with positive slope even in the brightest
// highlights. Build 256 entries once; pixels need no divisions or extra pass.
fn knife_highlight_table(night: f32) -> [(f32, f32); 256] {
    std::array::from_fn(|i| {
        let peak = i as f32;
        let excess = (peak - 150.).max(0.);
        let compressed = peak - excess + excess / (1. + excess / 95.);
        let gain = if i == 0 { 1. } else { 1. + night * (compressed / peak - 1.) };
        let tint = 1. - night * ((peak - 160.) / 95.).clamp(0., 1.) * 0.85;
        (gain, tint)
    })
}

// Source-based saturation/exposure, before paint relief. Keep a trace of
// source hue at night rather than recoloring the whole street purple.
fn knife_atmosphere(rgb: [f32; 3], grade: [f32; 3], gain: f32, tint: f32, weather: [f32; 3]) -> [f32; 3] {
    let gray = rgb[0]*0.299 + rgb[1]*0.587 + rgb[2]*0.114;
    std::array::from_fn(|k| (gray + (rgb[k]-gray)*grade[0]) * gain * grade[1]
        + weather[k]*tint*grade[2])
}

// Continuous, bounded alignment: faint gradients cannot flip a whole stroke.
// Deterministic anchors keep the paint surface coherent without frame blending.
fn painted_stroke_direction(dx: f32, dy: f32, seed: f32) -> f32 {
    let base = (seed - 0.5) * 2.6;
    let (sn, cs) = base.sin_cos();
    let a = -dy*cs + dx*sn;
    let b = dy*sn + dx*cs;
    base + 0.85 * (2.*a*b) / (a*a + b*b + 0.008)
}
fn painted_detail_opacity(gradient: f32) -> f32 {
    let t = ((gradient - 0.035) / 0.09).clamp(0., 1.);
    t*t*(3.-2.*t)
}

// Deterministic brush families: width/length variation comes from the stroke
// identity, not the current frame. Each family has a distinctly different aspect.
fn mixed_brush(seed: f32, variation: f32) -> (f32, f32) {
    if seed < 0.34 { (0.95 + variation*0.7, 0.55 + variation*0.35) }
    else if seed < 0.70 { (2.0 + variation*1.2, 0.12 + variation*0.10) }
    else { (0.42 + variation*0.32, 0.38 + variation*0.30) }
}

fn impasto_dimensions(kind: f32, length_seed: f32, width_seed: f32) -> (f32, f32) {
    if kind < 0.50 { (0.55+length_seed*1.5, 0.55+width_seed*0.8) }
    else if kind < 0.78 { (0.25+length_seed*0.55, 0.35+width_seed*0.6) }
    else { (1.6+length_seed*1.8, 0.08+width_seed*0.13) }
}

// MODE 0 retains the reference pixels. New studies share its optimized geometry.
fn palette_knife<const MODE: u8>(frame: &mut RawFrame, luma: &[f32], edition: u32, environment: [f32; 3]) {
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
    let source = if MODE > 0 { frame.data.clone() } else { Vec::new() };
    // A few RGB levels of warmth/coolness, never a replacement palette.
    let warmth = (1. - environment[1]) * 3. - environment[2] * 2.;
    let weather = if MODE >= 3 { knife_day_tint(environment) } else { [warmth, warmth * 0.15, -warmth] };
    let highlights = if MODE >= 4 { knife_highlight_table(knife_night_strength(environment[0])) } else { [(1., 1.); 256] };
    let night = knife_night_strength(environment[0]);
    let grade = [1.4 - night*1.18, 1.04 - night*0.42, 0.75 - night*0.68];
    let source_color = |x: usize, y: usize, radius: usize| {
        let points = [(x,y), (x.saturating_sub(radius),y), ((x+radius).min(w-1),y),
                      (x,y.saturating_sub(radius)), (x,(y+radius).min(h-1))];
        let rgb: [f32; 3] = std::array::from_fn(|k| points.iter().map(|&(xx,yy)| source[(yy*w+xx)*3+k] as f32).sum::<f32>() * 0.2);
        let (gain, tint) = if MODE >= 4 { highlights[rgb[0].max(rgb[1]).max(rgb[2]) as usize] } else { (1., 1.) };
        if MODE >= 5 { knife_atmosphere(rgb, grade, gain, tint, weather) }
        else { std::array::from_fn(|k| rgb[k] * gain + weather[k] * tint) }
    };
    for i in 0..w * h {
        if MODE == 0 {
            let c = color_at(luma[i]);
            put(frame, i, blend(c, [73., 65., 77.], 0.25));
        } else {
            let (gain, tint) = if MODE >= 4 {
                highlights[*source[i*3..i*3+3].iter().max().unwrap() as usize]
            } else { (1., 1.) };
            if MODE >= 5 {
                let c = knife_atmosphere(std::array::from_fn(|k| source[i*3+k] as f32), grade, gain, tint, weather);
                put(frame, i, c.map(|v| v*0.92));
            } else {
                put(frame, i, std::array::from_fn(|k| source[i*3+k] as f32 * gain * 0.92 + weather[k] * tint));
            }
        }
    }
    let geometry = if MODE == 12 { 0 } else { MODE };
    let sizes = if geometry == 11 { [150., 58., 12.] }
    else if geometry == 10 { [80., 30., 8.] }
    else if geometry >= 9 { [70., 30., 9.] }
    else if geometry == 8 { [80., 32., 10.] }
    else if geometry == 7 { [60., 26., 8.] }
    else if edition == 45 {
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
                let accent = if MODE == 12 && coat == 0 && seed < 0.20 {
                    let t = (1.-gradient/0.05).clamp(0.,1.);
                    t*t*(3.-2.*t)
                } else { 0. };
                // Preserve broad paint fields. Fine accents have stable sparse
                // placement; their opacity still responds continuously to detail.
                if geometry >= 10 && coat == 2 && hash(gx+913, gy+227) > if geometry == 10 { 0.55 } else { 0.34 } { continue; }
                let opacity = if geometry >= 10 && coat == 1 {
                    let base = if geometry == 10 { 0.55 } else { 0.30 };
                    base + painted_detail_opacity(gradient)*(1.-base)
                } else if geometry >= 6 && coat == 2 { painted_detail_opacity(gradient) } else { 1. };
                if (geometry >= 6 && opacity <= 0.) || (geometry < 6 && coat == 2 && gradient < 0.065) {
                    continue;
                }
                let angle = if geometry >= 9 {
                    let regional = paint_noise(cx / (180.*scale).max(1.), cy / (180.*scale).max(1.));
                    painted_stroke_direction(dx, dy, regional) + (seed-0.5)*0.32
                } else if geometry >= 6 {
                    painted_stroke_direction(dx, dy, seed)
                } else if gradient > 0.03 {
                    dy.atan2(dx) + 1.57 + (seed - 0.5) * 0.45
                } else {
                    (seed - 0.5) * 2.6
                };
                let (sn, cs) = angle.sin_cos();
                let length = (size * scale).max(1.) * (0.8 + seed * 0.75) * if geometry >= 6 { 1.12 } else { 1. };
                let width = (size * scale).max(1.) * (0.35 + hash(gx, gy + 41) * 0.25) * if geometry >= 6 { 0.92 } else { 1. };
                let (length, width) = if geometry >= 10 {
                    let (l,w) = impasto_dimensions(seed, hash(gx+417,gy+coat*97), hash(gx+619,gy+coat*53));
                    ((size*scale).max(1.)*l, (size*scale).max(1.)*w)
                } else if geometry >= 9 {
                    let r = hash(gx+417, gy+coat*97);
                    ((size*scale).max(1.)*(0.85+r*1.25), (size*scale).max(1.)*(0.30+seed*0.38))
                } else if geometry >= 7 {
                    let variation = hash(gx + 417, gy + coat*97);
                    let (l, w) = mixed_brush(seed, variation);
                    ((size*scale).max(1.)*l, (size*scale).max(1.)*w)
                } else { (length, width) };
                let length = length * (1. + accent*0.45);
                let reach = if geometry >= 9 { width*1.5 } else { width*(1.+accent*0.2) };
                let bx = cs.abs() * length + sn.abs() * reach;
                let by = sn.abs() * length + cs.abs() * reach;
                let c = if MODE == 0 { color_at(luma[i]) } else { source_color(x, y, (step / 4).max(1)) };
                let drag = if geometry >= 9 {
                    let at = |sign: f32| source_color(
                        (cx + sign*cs*length*0.5).clamp(0., (w-1) as f32) as usize,
                        (cy + sign*sn*length*0.5).clamp(0., (h-1) as f32) as usize, (step/4).max(1));
                    let a = at(-1.); let b = at(1.);
                    std::array::from_fn(|k| (b[k]-a[k])*0.45)
                } else { [0.;3] };
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
                ]
                .map(|(a, b, c)| (a, b, c + if geometry >= 9 { 0.6 } else { accent as f64*0.2 }, if a.abs() > 1e-12 { 1. / a } else { 0. }));
                for yy in (cy - by).max(0.) as usize..=(cy + by).min((h - 1) as f32) as usize {
                    let mut left = -bx as f64;
                    let mut right = bx as f64;
                    for &(a, b, c, inverse) in &planes {
                        if a.abs() > 1e-12 {
                            let edge = (c - b * (yy as f64 - cy as f64)) * inverse;
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
                        knife_pixels::<MODE>(
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
                            drag,
                            opacity,
                            accent,
                        );
                    }
                }
            }
        }
    }
}

// Explicit four-lane SIMD keeps pigment hashing, lighting, and coverage packed
// on both ARM and x86. Pigment normalization preserves division results exactly.
#[inline]
fn knife_pixels<const MODE: u8>(
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
    drag: [f32; 3],
    opacity: f32,
    accent: f32,
) {
    use wide::{f32x4 as F, CmpGt, CmpLt};
    let material = if MODE == 12 { 0 } else { MODE };
    let ox = F::new(std::array::from_fn(|k| (x + k) as f32 - cx));
    let oy = y as f32 - cy;
    let u = (ox * cs + F::splat(oy * sn)) / F::splat(length);
    let v = (-ox * sn + F::splat(oy * cs)) / F::splat(width);
    let v = if MODE == 12 {
        v - (F::ONE-u*u).max(F::ZERO) * (accent*0.18)
    } else if material >= 9 {
        v - (F::ONE-u*u).max(F::ZERO) * ((seed-0.5)*0.85) * (F::ONE + u*0.4)
    } else { v };
    let distance = if material >= 9 {
        let end = ((u.abs()-0.72) / F::splat(0.28)).max(F::ZERO);
        let cap = (F::ONE-end*end).max(F::ZERO).sqrt();
        let pressure = (F::splat(0.65+seed*0.16) - u*0.13) * cap;
        (v.abs()-pressure).max(u.abs()-1.)
    } else { (v.abs() - (F::splat(0.9) - u * 0.2)).max(u.abs() + v.abs() * 0.18 - 1.) };
    // The conservative scanline guard also produces wholly empty batches.
    // Reject those before evaluating pigment or reading destination colors.
    if distance.cmp_lt(F::ZERO).none() {
        return;
    }
    let cover = (-distance * width).max(F::ZERO).min(F::ONE) * 0.94;
    let scrape = knife_noise4(u * (if material >= 9 { 1.3 } else { 4. }) + seed * 19., v * (if material >= 9 { 19. } else { 13. }) + coat as f32 * 37.);
    let cover = if material >= 9 {
        let fringe = (scrape-0.35).max(F::ZERO)*0.12;
        let edge = distance + fringe;
        let paint_load = F::splat(0.86 + seed*0.08) - (u+0.3).max(F::ZERO)*(F::ONE-scrape)*0.13;
        (-edge*width).max(F::ZERO).min(F::ONE) * paint_load * opacity
    } else if material >= 6 {
        // Rounded, slightly ragged tips inside the original convex bounds.
        // The same pigment field supplies dry-brush breakup near the edge.
        let curved_v = v + (F::ONE - u*u).max(F::ZERO) * ((seed-0.5)*0.22);
        let rounded = (u*u + curved_v*curved_v*0.65 - 0.92) * 0.5;
        let bristles = (scrape - 0.35).max(F::ZERO) * 0.12;
        let edge = distance.max(rounded + bristles);
        (-edge * width).max(F::ZERO).min(F::ONE) * (0.94 * opacity)
    } else { cover };
    let ridge = v.cmp_gt(F::splat(0.58)).blend(
        (v - 0.58) * 35.,
        v.cmp_lt(F::splat(-0.65)).blend(F::splat(-9.), F::ZERO),
    );
    let light = if material < 2 {
        F::splat(tint) + ridge + (scrape - 0.5) * 9.
    } else {
        // Blade-local channels give thick paint a directional surface. Reuse
        // existing pigment noise; no extra noise field or full-frame lighting pass.
        let phase = v * (if material >= 7 { 3. + seed*6. } else { 5.5 }) + scrape * 0.7 + seed * 7.;
        let fraction = phase - phase.floor();
        let groove = (fraction - 0.5).abs() * 4. - 1.;
        let shoulder = (F::ONE - (v - 0.55).abs() * 6.).max(F::ZERO);
        let valley = (F::ONE - (v + 0.62).abs() * 8.).max(F::ZERO);
        let body = (F::ONE - u.abs()).max(F::ZERO);
        let lighting = 0.65 + (cs * 0.7 - sn * 0.7).abs() * 0.35;
        F::splat(tint * 0.6) + shoulder * ((if material >= 10 { 34. } else { 26. }) * lighting) - valley * (if material >= 10 { 21. } else { 17. })
            + groove * body * 9. + (scrape - 0.5) * 15.
    };
    let mut colors = [[0.; 4]; 3];
    for channel in 0..3 {
        let old = F::new(std::array::from_fn(|k| {
            pixels.get(k * 3 + channel).copied().unwrap_or(0) as f32
        }));
        let pigment = if material >= 9 { F::splat(c[channel]) + u*drag[channel] } else { F::splat(c[channel]) };
        colors[channel] = (old + (pigment + light - old) * cover)
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
    // Knife-local coordinates are finite and far inside the i32 range.
    let ix = U::new(floor_x.fast_trunc_int().to_array().map(|v| v as u32));
    let iy = U::new(floor_y.fast_trunc_int().to_array().map(|v| v as u32));
    let tx = x - floor_x;
    let ty = y - floor_y;
    let tx = tx * tx * (F::splat(3.) - tx * 2.);
    let ty = ty * ty * (F::splat(3.) - ty * 2.);
    let h = |dx: u32, dy: u32| {
        let v = ((ix + U::splat(dx)) * U::splat(374761393))
            ^ ((iy + U::splat(dy)) * U::splat(668265263));
        let v = (v ^ (v >> 13u32)) * U::splat(1274126177);
        let v = (v ^ (v >> 16u32)) & U::splat(65535);
        normalize_pigment_hash(v)
    };
    let a = h(0, 0) * (F::ONE - tx) + h(1, 0) * tx;
    let b = h(0, 1) * (F::ONE - tx) + h(1, 1) * tx;
    a * (F::ONE - ty) + b * ty
}

// Exact n/65535 for integer n in 0..=65535. A rounded reciprocal is
// one ULP low for 512 inputs. Their product bits match this mask; correcting
// those cases avoids the SIMD divide without changing any pigment value.
#[inline]
fn normalize_pigment_hash(value: wide::u32x4) -> wide::f32x4 {
    use wide::{f32x4 as F, u32x4 as U, CmpEq};
    let scaled = F::new(value.to_array().map(|v| v as f32)) * (1. / 65535.);
    let bits = U::new(scaled.to_array().map(f32::to_bits));
    let correction = (bits & U::splat(0x1ff01)).cmp_eq(U::splat(0x8000));
    F::new((bits - correction).to_array().map(f32::from_bits))
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
    fn impasto_width_and_length_vary_independently() {
        for kind in [0.1, 0.6, 0.9] {
            let a = impasto_dimensions(kind, 0., 0.);
            let long = impasto_dimensions(kind, 1., 0.);
            let wide = impasto_dimensions(kind, 0., 1.);
            assert!(long.0 > a.0 && long.1 == a.1);
            assert!(wide.1 > a.1 && wide.0 == a.0);
        }
        assert!(impasto_dimensions(0.1,0.5,1.).1 > impasto_dimensions(0.9,0.5,0.).1*10.);
    }
    #[test]
    fn brush_families_have_distinct_proportions_and_bounded_sizes() {
        for v in [0., 0.5, 1.] {
            let broad = mixed_brush(0.1, v);
            let thin = mixed_brush(0.5, v);
            let dab = mixed_brush(0.9, v);
            assert!(thin.0/thin.1 > 9.);
            assert!(broad.1 > thin.1*2.);
            assert!(dab.0 < broad.0 && dab.0 < thin.0*0.4);
            for (l,w) in [broad,thin,dab] { assert!(l>0. && l<=3.2 && w>0. && w<=0.9); }
        }
    }
    #[test]
    fn painted_strokes_do_not_pop_at_old_gradient_thresholds() {
        for seed in [0., 0.2, 0.5, 0.8, 1.] {
            for threshold in [0., 0.03, 0.035, 0.065, 0.125] {
                let a = painted_stroke_direction(threshold-0.0001, 0.01, seed);
                let b = painted_stroke_direction(threshold+0.0001, 0.01, seed);
                assert!((a-b).abs() < 0.005);
                assert!((painted_detail_opacity(threshold-0.0001)
                    - painted_detail_opacity(threshold+0.0001)).abs() < 0.005);
            }
        }
        assert_eq!(painted_detail_opacity(0.), 0.);
        assert_eq!(painted_detail_opacity(1.), 1.);
    }
    #[test]
    fn atmosphere_is_darker_and_grayer_at_night_and_richer_by_day() {
        let range = |c: [f32; 3]| c.iter().copied().fold(f32::NEG_INFINITY, f32::max)
            - c.iter().copied().fold(f32::INFINITY, f32::min);
        for rgb in [[160.,80.,60.], [70.,150.,85.], [75.,90.,160.]] {
            let day = knife_atmosphere(rgb, [1.4,1.04,0.75],1.,1.,[0.;3]);
            let night = knife_atmosphere(rgb, [0.22,0.62,0.07],1.,1.,[0.;3]);
            assert!(range(day) > range(rgb));
            assert!(range(night) < range(rgb)*0.2);
            let luminance = |c: [f32; 3]| c[0]*0.299 + c[1]*0.587 + c[2]*0.114;
            assert!(luminance(night) < luminance(day)*0.7);
        }
        let gray = knife_atmosphere([100.;3], [0.22,0.62,0.07],1.,1.,[0.;3]);
        assert!(range(gray) < 0.001);
    }
    #[test]
    fn night_highlights_are_monotonic_and_leave_day_and_shadows_alone() {
        let day = knife_highlight_table(knife_night_strength(12.));
        assert!(day.iter().all(|&(gain,tint)| gain == 1. && tint == 1.));
        let night = knife_highlight_table(knife_night_strength(0.));
        assert!(night[..151].iter().all(|&(gain,tint)| gain == 1. && tint == 1.));
        assert!(night[255].0 * 255. < 205.);
        for i in 1..256 { assert!(night[i].0 * i as f32 > night[i-1].0 * (i-1) as f32); }
        assert_eq!(knife_night_strength(0.), knife_night_strength(24.));
        for h in 0..240 { assert!((0.0..=1.0).contains(&knife_night_strength(h as f32 / 10.))); }
        let luma = vec![0.4; 96*54];
        let mut a = RawFrame::filled(96, 54, 235, 195, 110);
        let mut b = a.clone();
        palette_knife::<3>(&mut a, &luma, 51, [12., 0.3, 0.]);
        palette_knife::<4>(&mut b, &luma, 52, [12., 0.3, 0.]);
        assert_eq!(a.data, b.data);
    }
    #[test]
    fn clock_tint_is_continuous_cyclic_and_weather_modulated() {
        assert_eq!(knife_day_tint([0., 0., 0.]), knife_day_tint([24., 0., 0.]));
        for h in [0., 4., 6.5, 10., 15., 18.5, 21., 24.] {
            let before = knife_day_tint([h - 0.001, 0.5, 0.3]);
            let after = knife_day_tint([h + 0.001, 0.5, 0.3]);
            assert!(before.iter().zip(after).all(|(a,b)| (a-b).abs() < 0.01));
        }
        let day = knife_day_tint([12., 0., 0.]);
        let night = knife_day_tint([0., 0., 0.]);
        assert!(day[0] > day[2] && night[2] > night[0]);
        let rainy = knife_day_tint([12., 1., 1.]);
        assert!(rainy[0] < day[0] && rainy[2] > day[2]);
        for h in 0..240 {
            for cloud in [0., 1.] {
                for rain in [0., 1.] {
                    assert!(knife_day_tint([h as f32 / 10., cloud, rain]).iter()
                        .all(|v| v.is_finite() && v.abs() < 40.));
                }
            }
        }
    }
    #[test]
    fn source_pigments_survive_reconstruction_and_weather_is_restrained() {
        // Identical geometry/luminance isolates RGB retention from edge detection.
        let luma = vec![0.4; 96 * 54];
        for rgb in [[170, 65, 40], [40, 150, 65], [40, 65, 170]] {
            let make = || RawFrame::filled(96, 54, rgb[0], rgb[1], rgb[2]);
            let mut neutral = make();
            let mut rainy = make();
            palette_knife::<2>(&mut neutral, &luma, 50, [14., 1., 0.]);
            palette_knife::<2>(&mut rainy, &luma, 50, [14., 1., 1.]);
            let dominant = rgb.iter().enumerate().max_by_key(|(_,v)| *v).unwrap().0;
            for p in neutral.data.chunks_exact(3) {
                assert!((0..3).all(|k| k == dominant || p[dominant] > p[k]));
            }
            assert_ne!(neutral.data, rainy.data);
            assert!(neutral.data.iter().zip(&rainy.data).all(|(a,b)| a.abs_diff(*b) <= 4));
        }
    }
    #[test]
    fn pigment_normalization_is_exact_for_the_entire_hash_domain() {
        for start in (0u32..65536).step_by(4) {
            let values = std::array::from_fn(|k| start + k as u32);
            let actual = normalize_pigment_hash(wide::u32x4::new(values)).to_array();
            for k in 0..4 {
                assert_eq!(
                    actual[k].to_bits(),
                    (values[k] as f32 / 65535.).to_bits(),
                    "hash {}",
                    values[k]
                );
            }
        }
    }
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
                        knife_pixels::<0>(
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
                            [0.;3],
                            1.,
                            0.,
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
