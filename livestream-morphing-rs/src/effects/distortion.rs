use std::f32::consts::PI;

use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// Sine-wave coordinate remapping for psychedelic melting effect.
#[derive(Default)]
pub struct Distortion {
    scratch: Option<RawFrame>,
}

impl Distortion {
    /// The caller supplies a monochrome RGB frame. Interpolate one channel only.
    pub fn apply_monochrome(&mut self, frame: &mut RawFrame, amplitude: f32, frequency: f32, phase: f32) {
        if amplitude == 0.0 || frame.width == 0 || frame.height == 0 { return; }
        let w=frame.width as usize; let h=frame.height as usize;
        if self.scratch.as_ref().map(|s|(s.width,s.height)) != Some((frame.width,frame.height)) {
            self.init(frame.width,frame.height);
        }
        let dst=&mut self.scratch.as_mut().unwrap().data;
        let columns: Vec<_>=(0..w).map(|x| {
            let sx=(x as f32+(phase+x as f32*frequency/w as f32).sin()*w as f32*amplitude).clamp(0.0,(w-1) as f32);
            let x0=sx as usize;
            (x0*3,(x0+1).min(w-1)*3,sx.fract())
        }).collect();
        for (y,row) in dst.chunks_exact_mut(w*3).enumerate() {
            let sy=(y as f32+(phase+y as f32*frequency/h as f32).sin()*h as f32*amplitude).clamp(0.0,(h-1) as f32);
            let y0=sy as usize; let fy=sy.fract();
            let a=&frame.data[y0*w*3..][..w*3];
            let b=&frame.data[(y0+1).min(h-1)*w*3..][..w*3];
            for (pixel,&(x0,x1,fx)) in row.chunks_exact_mut(3).zip(&columns) {
                let val=a[x0] as f32*(1.0-fx)*(1.0-fy)+a[x1] as f32*fx*(1.0-fy)
                    +b[x0] as f32*(1.0-fx)*fy+b[x1] as f32*fx*fy;
                pixel.fill(val.clamp(0.0,255.0) as u8);
            }
        }
        std::mem::swap(&mut frame.data,dst);
    }
    /// Identical signature coordinates and bilinear weights, with independent pigments.
    pub fn apply_rgb_at_phase(&mut self, frame: &mut RawFrame, amplitude: f32, frequency: f32, phase: f32) {
        if amplitude == 0.0 || frame.width == 0 || frame.height == 0 { return; }
        let w=frame.width as usize; let h=frame.height as usize;
        if self.scratch.as_ref().map(|s|(s.width,s.height)) != Some((frame.width,frame.height)) {
            self.init(frame.width,frame.height);
        }
        let dst=&mut self.scratch.as_mut().unwrap().data;
        let columns: Vec<_>=(0..w).map(|x| {
            let sx=(x as f32+(phase+x as f32*frequency/w as f32).sin()*w as f32*amplitude).clamp(0.0,(w-1) as f32);
            let x0=sx as usize;
            (x0*3,(x0+1).min(w-1)*3,sx.fract())
        }).collect();
        for (y,row) in dst.chunks_exact_mut(w*3).enumerate() {
            let sy=(y as f32+(phase+y as f32*frequency/h as f32).sin()*h as f32*amplitude).clamp(0.0,(h-1) as f32);
            let y0=sy as usize; let fy=sy.fract();
            let a=&frame.data[y0*w*3..][..w*3];
            let b=&frame.data[(y0+1).min(h-1)*w*3..][..w*3];
            for (pixel,&(x0,x1,fx)) in row.chunks_exact_mut(3).zip(&columns) {
                for c in 0..3 {
                    let val=a[x0+c] as f32*(1.0-fx)*(1.0-fy)+a[x1+c] as f32*fx*(1.0-fy)
                        +b[x0+c] as f32*(1.0-fx)*fy+b[x1+c] as f32*fx*fy;
                    pixel[c]=val.clamp(0.0,255.0) as u8;
                }
            }
        }
        std::mem::swap(&mut frame.data,dst);
    }
}

impl Effect for Distortion {
    fn id(&self) -> &'static str {
        "distortion"
    }

    fn name(&self) -> &'static str {
        "Psychedelic Distortion"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef::new("amplitude", "Amplitude", 0.0, 0.2, 0.02, 0.005),
            ParamDef::new("frequency", "Frequency", 1.0, 50.0, 12.0, 0.5),
            ParamDef::new("cycle_length", "Cycle Length", 30.0, 600.0, 180.0, 1.0),
        ]
    }

    fn init(&mut self, width: u32, height: u32) {
        self.scratch = Some(RawFrame::new(width, height));
    }

    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, ctx: &FrameCtx) {
        let amplitude = params.get("amplitude").copied().unwrap_or(0.02);
        let frequency = params.get("frequency").copied().unwrap_or(12.0);
        let cycle_length = params.get("cycle_length").copied().unwrap_or(180.0) as u32;

        let scratch = match &mut self.scratch {
            Some(s) => s,
            None => return,
        };

        let w = frame.width;
        let h = frame.height;
        let wf = w as f32;
        let hf = h as f32;
        let time = (ctx.frame_number % cycle_length) as f32 * (2.0 * PI / cycle_length as f32);

        // Coordinates are separable: calculate each column once per frame.
        let columns: Vec<_> = (0..w).map(|x| {
            let offset = (time + x as f32 * frequency / wf).sin() * wf * amplitude;
            let src = (x as f32 + offset).clamp(0.0, wf - 1.0);
            let x0 = src.floor() as u32;
            (x0, (x0 + 1).min(w - 1), src.fract())
        }).collect();
        for y in 0..h {
            let y_offset = (time + y as f32 * frequency / hf).sin() * hf * amplitude;
            let src_y = (y as f32 + y_offset).clamp(0.0, hf - 1.0);
            let y0 = src_y.floor() as u32;
            let y1 = (y0 + 1).min(h - 1);
            let fy = src_y.fract();
            for x in 0..w {
                let (x0, x1, fx) = columns[x as usize];

                let dst_idx = ((y * w + x) * 3) as usize;
                for c in 0..3 {
                    let p00 = frame.data[((y0 * w + x0) * 3) as usize + c] as f32;
                    let p10 = frame.data[((y0 * w + x1) * 3) as usize + c] as f32;
                    let p01 = frame.data[((y1 * w + x0) * 3) as usize + c] as f32;
                    let p11 = frame.data[((y1 * w + x1) * 3) as usize + c] as f32;
                    let val = p00 * (1.0 - fx) * (1.0 - fy)
                        + p10 * fx * (1.0 - fy)
                        + p01 * (1.0 - fx) * fy
                        + p11 * fx * fy;
                    scratch.data[dst_idx + c] = val.clamp(0.0, 255.0) as u8;
                }
            }
        }

        std::mem::swap(&mut frame.data, &mut scratch.data);
    }
}

crate::register_effect!(Distortion);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_params;

    #[test]
    fn monochrome_fast_path_matches_color_interpolation() {
        let mut a=RawFrame::new(32,18);
        for (i,p) in a.data.chunks_exact_mut(3).enumerate() { p.fill(((i*37)%256) as u8); }
        let mut b=a.clone();
        let mut reference=Distortion::default(); reference.init(32,18);
        let mut fast=Distortion::default();
        let p=default_params(&reference.params());
        let ctx=FrameCtx {frame_number:10,width:32,height:18,delta_seconds:1.0/30.0};
        reference.apply(&mut a,&p,&ctx);
        fast.apply_monochrome(&mut b,0.02,12.0,10.0*(2.0*PI/180.0));
        assert_eq!(a.data,b.data);
    }
    #[test]
    fn distortion_zero_amplitude_is_identity() {
        let mut effect = Distortion::default();
        effect.init(4, 4);
        let mut frame = RawFrame::filled(4, 4, 128, 64, 32);
        let original = frame.data.clone();
        let mut params = default_params(&effect.params());
        params.insert("amplitude".into(), 0.0);
        let ctx = FrameCtx { delta_seconds: 1.0/30.0, frame_number: 0, width: 4, height: 4 };
        effect.apply(&mut frame, &params, &ctx);
        assert_eq!(frame.data, original);
    }

    #[test]
    fn distortion_nonzero_amplitude_changes_pixels() {
        let mut effect = Distortion::default();
        effect.init(8, 8);
        let mut frame = RawFrame::new(8, 8);
        for y in 0..8u32 {
            for x in 0..8u32 {
                let idx = ((y * 8 + x) * 3) as usize;
                frame.data[idx] = (x * 32) as u8;
                frame.data[idx + 1] = (y * 32) as u8;
                frame.data[idx + 2] = 0;
            }
        }
        let original = frame.data.clone();
        let mut params = default_params(&effect.params());
        params.insert("amplitude".into(), 0.05);
        let ctx = FrameCtx { delta_seconds: 1.0/30.0, frame_number: 10, width: 8, height: 8 };
        effect.apply(&mut frame, &params, &ctx);
        assert_ne!(frame.data, original);
    }

    #[test]
    fn distortion_default_params() {
        let effect = Distortion::default();
        let params = default_params(&effect.params());
        assert_eq!(params.get("amplitude"), Some(&0.02));
        assert_eq!(params.get("frequency"), Some(&12.0));
        assert_eq!(params.get("cycle_length"), Some(&180.0));
    }
}
