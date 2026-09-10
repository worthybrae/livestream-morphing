//! Scene reconstruction studies. No frame history; final signature lives in Editions.
use super::RawFrame;

#[derive(Default)]
pub(super) struct Forms {
    size: (usize, usize),
    glass: Vec<(u32, u8)>,
    sites: Vec<(usize, usize, f32)>,
}
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
    pub(super) fn draw(&mut self, frame: &mut RawFrame, luma: &[f32], edition: u32, seconds: f32) {
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
            26 | 27 => paper_relief(frame, luma, edition, seconds),
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
