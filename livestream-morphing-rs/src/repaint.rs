//! Small, source-triggered paint passes, applied in display order before the wave.
use crate::effects::{RawFrame,distortion::Distortion};
#[derive(Default)]
pub struct Repaint { size:(u32,u32), canvas:Vec<u8>, marks:Vec<Mark>, coverage:Vec<u8>, previous:Vec<u8>, older:Vec<u8>, settled:Vec<u16>, warp:Distortion }
#[derive(Clone,Default)]
struct Mark { age:f32, active:bool }
impl Repaint {
    pub fn apply_mode(&mut self,frame:&mut RawFrame,number:u32,dt:f32,calm:bool) {
        self.paint_mode(frame,dt,calm);
        self.warp.apply_rgb_anchored(frame,0.012,8.,(number as f64*dt as f64/8.) as f32*std::f32::consts::TAU);
    }
    #[cfg(test)]
    fn paint(&mut self,frame:&mut RawFrame,dt:f32) {self.paint_mode(frame,dt,false);}
    fn paint_mode(&mut self,frame:&mut RawFrame,dt:f32,calm:bool) {
        let (w,h)=(frame.width as usize,frame.height as usize);
        let step=((w as f32/1920.)*24.).round().max(4.) as usize;
        let cols=w.div_ceil(step);let rows=h.div_ceil(step);
        if self.size!=(frame.width,frame.height) || self.canvas.len()!=frame.data.len() {
            self.size=(frame.width,frame.height);self.canvas=frame.data.clone();
            self.marks=vec![Mark::default();cols*rows];self.coverage=vec![0;w*h];self.previous=frame.data.clone();self.older=frame.data.clone();self.settled=frame.data.iter().map(|v|*v as u16*256).collect();return;
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
            brush_coverage(&mut self.coverage,w,h,center_x,center_y,length,width,cs,sn,progress,bx,by);
            if progress>=1. {mark.active=false;}
        }}
        // One Q15 response table per frame keeps the six-million-channel pass
        // integer-only. Pigment remains Q8, so gradual changes do not get stuck.
        let response=1.-(-dt.max(0.)/0.09).exp();
        let weights: [i32;256]=std::array::from_fn(|a|(response*(0.25+0.75*a as f32/255.)*32768.).round() as i32);
        let rise=(420.*dt*256.).round() as i32;let fall=(600.*dt*256.).round() as i32;
        for (at,&coverage) in self.coverage.iter().enumerate() {
            let alpha=coverage as u32;
            for c in 0..3 {
                let i=at*3+c;
                if calm {
                    // A three-frame median rejects isolated exposure/codec flashes.
                    // Q8 pigment keeps sub-byte changes from sticking or stepping.
                    let a=frame.data[i];let b=self.previous[i];let c=self.older[i];
                    let target=a.min(b).max(a.max(b).min(c)) as i32*256;
                    let current=self.settled[i] as i32;
                    let delta=target-current;
                    let change=((delta*weights[alpha as usize]+16384)>>15).clamp(-fall,rise);
                    let next=(current+change).clamp(0,65280);
                    self.settled[i]=next as u16;
                    self.canvas[i]=((next+128)>>8) as u8;
                } else {
                    self.canvas[i]=((self.canvas[i] as u32*(255-alpha)+frame.data[i] as u32*alpha+127)/255) as u8;
                    self.settled[i]=self.canvas[i] as u16*256;
                }
            }
        }
        self.older.copy_from_slice(&self.previous);
        self.previous.copy_from_slice(&frame.data);
        frame.data.copy_from_slice(&self.canvas);
    }
}
// Four neighboring footprint samples at a time. This is the same soft filbert
// sweep, including overlapping coverage; only its rasterization is batched.
#[allow(clippy::too_many_arguments)]
fn brush_coverage(coverage:&mut [u8],w:usize,h:usize,cx:f32,cy:f32,length:f32,width:f32,cs:f32,sn:f32,progress:f32,bx:f32,by:f32) {
    use wide::f32x4 as F;
    let start=(cx-bx).max(0.) as usize;let end=((cx+bx) as usize).min(w-1);
    for y in ((cy-by).max(0.) as usize)..=((cy+by) as usize).min(h-1) {
        let dy=F::splat(y as f32-cy);
        for x in (start..=end).step_by(4) {
            let dx=F::new(std::array::from_fn(|k|(x+k) as f32-cx));
            let u=(dx*cs+dy*sn)/length;let v=(-dx*sn+dy*cs)/width;
            let body=((F::ONE-u*u-v*v)*4.).max(F::ZERO).min(F::ONE);
            let travel=(u+F::ONE)*0.5;
            let t=((F::splat(progress*1.3)-travel)/0.3).max(F::ZERO).min(F::ONE);
            let alpha=(body*t*t*(F::splat(3.)-t*2.)*255.).to_array();
            for k in 0..(end-x+1).min(4) {let at=y*w+x+k;coverage[at]=coverage[at].max(alpha[k].round() as u8);}
        }
    }
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn batched_filbert_matches_scalar_with_partial_rows() {
  for seed in 0..25 {
   let (w,h)=(57,43);let t=seed as f32/24.;let cx=3.+t*49.;let cy=4.+(1.-t)*36.;
   let length=8.+t*10.;let width=5.+t*6.;let (sn,cs)=((t-0.5)*2.8).sin_cos();
   let bx=(cs.abs()*length+sn.abs()*width).ceil();let by=(sn.abs()*length+cs.abs()*width).ceil();
   for progress in [0.,0.12,0.35,0.7,1.] {
    let mut a=vec![13;w*h];let mut b=a.clone();
    brush_coverage(&mut a,w,h,cx,cy,length,width,cs,sn,progress,bx,by);
    for y in ((cy-by).max(0.) as usize)..=((cy+by) as usize).min(h-1) {for x in ((cx-bx).max(0.) as usize)..=((cx+bx) as usize).min(w-1) {
     let dx=x as f32-cx;let dy=y as f32-cy;let u=(dx*cs+dy*sn)/length;let v=(-dx*sn+dy*cs)/width;
     let body=((1.-u*u-v*v)*4.).clamp(0.,1.);let travel=(u+1.)*0.5;let t=((progress*1.3-travel)/0.3).clamp(0.,1.);
     let alpha=body*t*t*(3.-2.*t);b[y*w+x]=b[y*w+x].max((alpha*255.).round() as u8);
    }}
    assert_eq!(a,b,"seed {seed}, progress {progress}");
   }
  }
 }
 #[test] fn pigment_arithmetic_is_safe_at_lower_output_rates() {
  for dt in [1./30.,1./15.,1./10.,1.] {
   let mut p=Repaint::default();let mut f=RawFrame::filled(16,12,0,0,0);p.paint_mode(&mut f,dt,true);
   for _ in 0..30 {
    let previous=f.data.clone();f=RawFrame::filled(16,12,255,255,255);p.paint_mode(&mut f,dt,true);
    assert!(f.data.iter().zip(previous).all(|(a,b)|(*a as f32-b as f32)<=420.*dt+1.));
   }
   assert!(f.data.iter().all(|v|*v>200));
  }
 }
 #[test] fn calm_rejects_a_single_frame_flash_and_settles_sustained_changes() {
  let mut p=Repaint::default();let base=RawFrame::filled(96,54,40,60,80);let mut f=base.clone();p.paint_mode(&mut f,1./30.,true);
  f=RawFrame::filled(96,54,255,255,255);p.paint_mode(&mut f,1./30.,true);assert_eq!(f.data,base.data);
  f=base.clone();p.paint_mode(&mut f,1./30.,true);assert_eq!(f.data,base.data);
  let target=RawFrame::filled(96,54,180,120,100);
  for _ in 0..30 {f=target.clone();p.paint_mode(&mut f,1./30.,true);}
  assert!(f.data.iter().zip(&target.data).all(|(a,b)|a.abs_diff(*b)<=2));
  for _ in 0..15 {f=base.clone();p.paint_mode(&mut f,1./30.,true);}
  assert!(f.data.iter().zip(&base.data).all(|(a,b)|a.abs_diff(*b)<=3));
 }
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
