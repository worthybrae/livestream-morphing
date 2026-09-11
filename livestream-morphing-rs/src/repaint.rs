//! Small, source-triggered paint passes, applied in display order before the wave.
use crate::effects::{RawFrame,distortion::Distortion};
#[derive(Default)]
pub struct Repaint { size:(u32,u32), canvas:Vec<u8>, marks:Vec<Mark>, warp:Distortion }
#[derive(Clone,Default)]
struct Mark { age:f32, active:bool }
impl Repaint {
    pub fn apply(&mut self,frame:&mut RawFrame,number:u32,dt:f32) {
        self.paint(frame,dt);
        self.warp.apply_rgb_anchored(frame,0.012,8.,(number as f64*dt as f64/8.) as f32*std::f32::consts::TAU);
    }
    fn paint(&mut self,frame:&mut RawFrame,dt:f32) {
        let (w,h)=(frame.width as usize,frame.height as usize);
        let step=((w as f32/1920.)*24.).round().max(4.) as usize;
        let cols=w.div_ceil(step);let rows=h.div_ceil(step);
        if self.size!=(frame.width,frame.height) || self.canvas.len()!=frame.data.len() {
            self.size=(frame.width,frame.height);self.canvas=frame.data.clone();
            self.marks=vec![Mark::default();cols*rows];return;
        }
        for cy in 0..rows {for cx in 0..cols {
            let id=cy*cols+cx;let seed=((id as u32).wrapping_mul(2654435761)>>16) as f32/65535.;
            let mark=&mut self.marks[id];let x0=cx*step;let y0=cy*step;
            let x1=(x0+step).min(w);let y1=(y0+step).min(h);
            if !mark.active {
                let mut change=0u32;let mut count=0;
                for y in (y0..y1).step_by(3) {for x in (x0..x1).step_by(3) {
                    for c in 0..3 {change+=self.canvas[(y*w+x)*3+c].abs_diff(frame.data[(y*w+x)*3+c]) as u32;count+=1;}
                }}
                if change <= count*3 {continue;}
                mark.active=true;mark.age=-seed*0.04;
            }
            mark.age+=dt;
            let duration=0.14+seed*0.10;
            let progress=(mark.age/duration).clamp(0.,1.);
            // A short diagonal brush travel, with a soft front.
            // Tiny staggered marks avoid a global reveal/wipe.
            for y in y0..y1 {for x in x0..x1 {
                let u=(x-x0) as f32/step as f32;let v=(y-y0) as f32/step as f32;
                let travel=if seed<0.5 {u*0.7+v*0.3} else {(1.-u)*0.3+v*0.7};
                let front=(progress*1.35-travel)/0.35;
                let t=front.clamp(0.,1.);let alpha=if progress>=1. {1.} else {t*t*(3.-2.*t)};
                let at=(y*w+x)*3;
                for c in 0..3 {let old=self.canvas[at+c] as f32;self.canvas[at+c]=(old+(frame.data[at+c] as f32-old)*alpha).round() as u8;}
            }}
            if progress>=1. {mark.active=false;}
        }}
        frame.data.copy_from_slice(&self.canvas);
    }
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn unchanged_scene_is_exact_and_change_finishes_without_trails() {
  let mut p=Repaint::default();let original=RawFrame::filled(64,36,40,60,80);let mut f=original.clone();p.paint(&mut f,1./30.);
  for _ in 0..20 {let mut f=original.clone();p.paint(&mut f,1./30.);assert_eq!(f.data,original.data);}
  let target=RawFrame::filled(64,36,180,100,50);let mut f=target.clone();p.paint(&mut f,1./30.);assert_ne!(f.data,target.data);
  for _ in 0..12 {f=target.clone();p.paint(&mut f,1./30.);}
  assert_eq!(f.data,target.data);
 }
}
