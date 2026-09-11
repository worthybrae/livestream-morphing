//! Small, source-triggered paint passes, applied in display order before the wave.
use crate::effects::{RawFrame,distortion::Distortion};
#[derive(Default)]
pub struct Repaint { size:(u32,u32), canvas:Vec<u8>, marks:Vec<Mark>, coverage:Vec<u8>, warp:Distortion }
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
            self.marks=vec![Mark::default();cols*rows];self.coverage=vec![0;w*h];return;
        }
        self.coverage.fill(0);
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
            // Overlapping filbert footprints: the grid only places brush centers.
            // Never clip a refresh to its detection cell or force its rectangle.
            let angle=(seed-0.5)*2.8;let (sn,cs)=angle.sin_cos();
            let center_x=(x0 as f32+step as f32*(0.4+seed*0.2)).min((w-1) as f32);
            let center_y=(y0 as f32+step as f32*(0.6-seed*0.2)).min((h-1) as f32);
            let length=step as f32*(1.8+seed*0.3);let width=step as f32*1.35;
            let bx=(cs.abs()*length+sn.abs()*width).ceil();
            let by=(sn.abs()*length+cs.abs()*width).ceil();
            for y in ((center_y-by).max(0.) as usize)..=((center_y+by) as usize).min(h-1) {
                for x in ((center_x-bx).max(0.) as usize)..=((center_x+bx) as usize).min(w-1) {
                    let dx=x as f32-center_x;let dy=y as f32-center_y;
                    let u=(dx*cs+dy*sn)/length;let v=(-dx*sn+dy*cs)/width;
                    let body=((1.-u*u-v*v)*4.).clamp(0.,1.);
                    if body==0. {continue;}
                    let travel=(u+1.)*0.5;
                    let t=((progress*1.3-travel)/0.3).clamp(0.,1.);
                    let alpha=body*t*t*(3.-2.*t);
                    let at=y*w+x;
                    self.coverage[at]=self.coverage[at].max((alpha*255.).round() as u8);
                }
            }
            if progress>=1. {mark.active=false;}
        }}
        for (at,&coverage) in self.coverage.iter().enumerate() {
            let alpha=coverage as u32;
            for c in 0..3 {
                let i=at*3+c;
                self.canvas[i]=((self.canvas[i] as u32*(255-alpha)+frame.data[i] as u32*alpha+127)/255) as u8;
            }
        }
        frame.data.copy_from_slice(&self.canvas);
    }
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn isolated_change_has_no_rectangular_refresh_edge() {
  let mut p=Repaint::default();let mut blank=RawFrame::filled(192,108,0,0,0);p.paint(&mut blank,1./30.);
  // One active detection region must paint across its cell boundary with an oval falloff.
  p.marks[10*48+20]=Mark {age:0.12,active:true};
  p.paint(&mut blank,1./30.);
  let nonzero:Vec<_>=p.coverage.iter().enumerate().filter(|(_,a)|**a>0).collect();
  let min_x=nonzero.iter().map(|(i,_)|i%192).min().unwrap();
  let max_x=nonzero.iter().map(|(i,_)|i%192).max().unwrap();
  assert!(min_x<80 && max_x>84);
  assert!(p.coverage.iter().any(|a|*a>0 && *a<255));
 }
 #[test] fn unchanged_scene_is_exact_and_change_finishes_without_trails() {
  let mut p=Repaint::default();let original=RawFrame::filled(64,36,40,60,80);let mut f=original.clone();p.paint(&mut f,1./30.);
  for _ in 0..20 {let mut f=original.clone();p.paint(&mut f,1./30.);assert_eq!(f.data,original.data);}
  let target=RawFrame::filled(64,36,180,100,50);let mut f=target.clone();p.paint(&mut f,1./30.);assert_ne!(f.data,target.data);
  for _ in 0..12 {f=target.clone();p.paint(&mut f,1./30.);}
  assert_eq!(f.data,target.data);
 }
}
