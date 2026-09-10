//! Cached knife footprints, continuously appearing detail and staggered paint deposits.
//! No camera-frame history: source motion stays current and workers remain interchangeable.
use super::RawFrame;
#[derive(Default)]
pub(super) struct KnifeLayers {
    width: usize,
    stamps: Vec<Vec<Pixel>>,
}
struct Pixel {
    x: i16,
    y: i16,
    alpha: f32,
    light: f32,
    along: f32,
}
fn hash(x: usize, y: usize) -> f32 {
    let mut n = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263);
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    ((n ^ (n >> 16)) & 65535) as f32 / 65535.
}
fn smooth(x: f32) -> f32 {
    let t = x.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
fn color(l: f32) -> [f32; 3] {
    let p = [
        [23., 43., 44.],
        [48., 91., 79.],
        [113., 153., 116.],
        [224., 155., 170.],
        [247., 229., 194.],
    ];
    let z = (l * 5.8).clamp(0., 4.);
    let n = (z as usize).min(3);
    std::array::from_fn(|k| p[n][k] + (p[n + 1][k] - p[n][k]) * (z - n as f32))
}
impl KnifeLayers {
    fn prepare(&mut self, w: usize) {
        if self.width == w {
            return;
        }
        self.width = w;
        self.stamps.clear();
        let scale = w as f32 / 1920.;
        for size in [36., 18., 8.] {
            for variant in 0..4 {
                for angle in 0..24 {
                    let seed = (variant as f32 + 0.5) / 4.;
                    let length = (size * scale).max(1.) * (0.8 + seed * 0.75);
                    let width = (size * scale).max(1.) * (0.35 + hash(variant, 41) * 0.25);
                    let (sn, cs) = (angle as f32 * std::f32::consts::PI / 24.).sin_cos();
                    let bx = (cs.abs() * length + sn.abs() * width).ceil() as i16;
                    let by = (sn.abs() * length + cs.abs() * width).ceil() as i16;
                    let mut pixels = Vec::new();
                    for y in -by..=by {
                        for x in -bx..=bx {
                            let u = (x as f32 * cs + y as f32 * sn) / length;
                            let v = (-x as f32 * sn + y as f32 * cs) / width;
                            let distance = (v.abs() - (0.9 - 0.2 * u.abs()))
                                .max(u.abs() + v.abs() * 0.18 - 1.);
                            if distance >= 0. {
                                continue;
                            }
                            let ridge = if v.abs() > 0.58 {
                                (v.abs() - 0.58) * 25.
                            } else {
                                0.
                            };
                            let scrape = hash(
                                ((u.abs() + 2.) * 25.) as usize + variant * 71,
                                ((v.abs() + 2.) * 37.) as usize,
                            );
                            pixels.push(Pixel {
                                x,
                                y,
                                alpha: (-distance * width).clamp(0., 1.) * 0.94,
                                light: ridge + (scrape - 0.5) * 4.,
                                along: u.abs(),
                            });
                        }
                    }
                    self.stamps.push(pixels);
                }
            }
        }
    }
    pub(super) fn draw(&mut self, frame: &mut RawFrame, luma: &[f32], seconds: f32, opaque: bool) {
        let w = frame.width as usize;
        let h = frame.height as usize;
        self.prepare(w);
        for (i, p) in frame.data.chunks_exact_mut(3).enumerate() {
            let c = color(luma[i]);
            for k in 0..3 {
                p[k] = (c[k] * 0.75 + [73., 65., 77.][k] * 0.25) as u8;
            }
        }
        let scale = w as f32 / 1920.;
        let radius = (80. * scale).ceil() as i32 + 4;
        let side = (radius * 2 + 1) as usize;
        let mut coverage = vec![0f32; if opaque { side * side } else { 0 }];
        let mut lighting = coverage.clone();
        let mut touched = Vec::new();
        for (coat, size) in [36., 18., 8.].iter().enumerate() {
            let step = (size * scale).max(2.) as usize;
            for gy in (0..h).step_by(step) {
                for gx in (0..w).step_by(step) {
                    let seed = hash(gx + coat * 791, gy + 29);
                    let x =
                        (gx as f32 + (seed - 0.5) * step as f32).clamp(0., (w - 1) as f32) as usize;
                    let y = (gy as f32 + (hash(gx + 83, gy) - 0.5) * step as f32)
                        .clamp(0., (h - 1) as f32) as usize;
                    let d = step.max(2);
                    let left = luma[y * w + x.saturating_sub(d)];
                    let right = luma[y * w + (x + d).min(w - 1)];
                    let top = luma[y.saturating_sub(d) * w + x];
                    let bottom = luma[(y + d).min(h - 1) * w + x];
                    let dx = right - left;
                    let dy = bottom - top;
                    let g = dx.abs() + dy.abs();
                    let detail = if coat == 2 {
                        smooth((g - 0.035) / 0.09)
                    } else {
                        1.
                    };
                    if detail < 0.005 {
                        continue;
                    }
                    // Blend unoriented axes instead of switching abruptly at a contrast threshold.
                    let fallback = (seed - 0.5) * 2.6;
                    let power = (dx * dx + dy * dy).max(1e-8);
                    let weight = smooth((g - 0.015) / 0.09);
                    let a = (fallback * 2.).cos() * (1. - weight)
                        + (dy * dy - dx * dx) / power * weight;
                    let b = (fallback * 2.).sin() * (1. - weight) - 2. * dx * dy / power * weight;
                    let orientation = (0.5 * b.atan2(a)).rem_euclid(std::f32::consts::PI) * 24.
                        / std::f32::consts::PI;
                    let angle = orientation.floor() as usize;
                    let fraction = orientation.fract();
                    let variant = ((seed * 4.) as usize).min(3);
                    let c = color(luma[y * w + x] * 0.6 + (left + right + top + bottom) * 0.1);
                    // Lower coats stay present. Each top stroke is refreshed by a staggered wipe.
                    let cycle = (seconds / 9. + seed + coat as f32 * 0.27).rem_euclid(1.);
                    let phase = if cycle < 0.8 {
                        smooth(cycle / 0.8)
                    } else {
                        1. - smooth((cycle - 0.8) / 0.2)
                    };
                    let tint = (seed - 0.5) * 16.;
                    for (rotation, weight) in [(angle, 1. - fraction), ((angle + 1) % 24, fraction)]
                    {
                        if weight < 0.005 {
                            continue;
                        }
                        for p in &self.stamps[(coat * 4 + variant) * 24 + rotation] {
                            let xx = x as i32 + p.x as i32;
                            let yy = y as i32 + p.y as i32;
                            if xx < 0 || yy < 0 || xx >= w as i32 || yy >= h as i32 {
                                continue;
                            }
                            let wipe = smooth((phase * 2.6 - 0.3 - (p.along + 1.) * 0.5) * 5.);
                            let deposit = if coat == 0 { 1. } else { 0.5 + 0.5 * wipe };
                            let alpha = if opaque {
                                let reveal = if coat == 0 {
                                    1.
                                } else {
                                    smooth((0.55 + phase * 0.6 - p.along) * 12.)
                                };
                                p.alpha * detail * weight * reveal
                            } else {
                                p.alpha * detail * weight * deposit
                            };
                            if opaque {
                                let j = (p.y as i32 + radius) as usize * side
                                    + (p.x as i32 + radius) as usize;
                                if coverage[j] == 0. && alpha > 0. {
                                    touched.push(j);
                                }
                                coverage[j] += alpha;
                                lighting[j] += alpha * p.light;
                                continue;
                            }
                            let i = (yy as usize * w + xx as usize) * 3;
                            for k in 0..3 {
                                let old = frame.data[i + k] as f32;
                                let target = c[k] + p.light + tint;
                                frame.data[i + k] =
                                    (old + (target - old) * alpha).clamp(0., 255.) as u8;
                            }
                        }
                    }
                    if opaque {
                        for j in touched.drain(..) {
                            let alpha = coverage[j].min(1.);
                            let xx = x as i32 + (j % side) as i32 - radius;
                            let yy = y as i32 + (j / side) as i32 - radius;
                            let i = (yy as usize * w + xx as usize) * 3;
                            let light = lighting[j] / coverage[j] * 1.5;
                            for k in 0..3 {
                                let old = frame.data[i + k] as f32;
                                frame.data[i + k] = (old + (c[k] + light + tint - old) * alpha)
                                    .clamp(0., 255.)
                                    as u8;
                            }
                            coverage[j] = 0.;
                            lighting[j] = 0.;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_and_fresh_workers_match_and_deposits_keep_changing() {
        let luma: Vec<_> = (0..192 * 108)
            .map(|i| 0.1 + (i % 192) as f32 / 400.)
            .collect();
        let mut painter = KnifeLayers::default();
        let mut a = RawFrame::new(192, 108);
        let mut b = a.clone();
        let mut c = a.clone();
        painter.draw(&mut a, &luma, 1., false);
        painter.draw(&mut b, &luma, 4., false);
        KnifeLayers::default().draw(&mut c, &luma, 4., false);
        assert_eq!(b.data, c.data);
        assert_ne!(a.data, b.data);
        let mut repeat = RawFrame::new(192, 108);
        painter.draw(&mut repeat, &luma, 13., false);
        let error: u64 = b
            .data
            .iter()
            .zip(&repeat.data)
            .map(|(x, y)| x.abs_diff(*y) as u64)
            .sum();
        assert!(error < 500, "continuous periodic phase, allowing rounding");
    }
}
