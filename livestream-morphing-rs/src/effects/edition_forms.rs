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
            _ => unreachable!(),
        }
    }
}
