//! Small, source-triggered paint passes, applied in display order before the wave.
use crate::effects::{RawFrame,distortion::Distortion};
#[derive(Default)]
pub struct Repaint { size:(u32,u32), canvas:Vec<u8>, marks:Vec<Mark>, coverage:Vec<u8>, mask:Vec<u8>, previous:Vec<u8>, older:Vec<u8>, settled:Vec<u16>, warp:Distortion, paint_ms:f64, warp_ms:f64, load:f32 }
#[derive(Clone,Default)]
struct Mark { age:f32, active:bool }
impl Repaint {
    pub fn set_load(&mut self,load:f32) {self.load=if load.is_finite() {load.clamp(0.,1.)} else {0.};}
    pub fn apply_mode(&mut self,frame:&mut RawFrame,number:u32,dt:f32,calm:bool) {
        self.advance_paint(frame,dt,calm);
        self.move_canvas(frame,number,dt);
    }
    pub fn advance_paint(&mut self,frame:&mut RawFrame,dt:f32,calm:bool) {
        let start=std::time::Instant::now();self.paint_mode(frame,dt,calm);self.paint_ms+=start.elapsed().as_secs_f64()*1000.;
    }
    pub fn move_canvas(&mut self,frame:&mut RawFrame,number:u32,dt:f32) {
        let start=std::time::Instant::now();self.warp.apply_rgb_anchored(frame,0.012,8.,(number as f64*dt as f64/8.) as f32*std::f32::consts::TAU);self.warp_ms+=start.elapsed().as_secs_f64()*1000.;
    }
    pub fn take_timings(&mut self)->(f64,f64) {(std::mem::take(&mut self.paint_ms),std::mem::take(&mut self.warp_ms))}
    #[cfg(test)]
    fn paint(&mut self,frame:&mut RawFrame,dt:f32) {self.paint_mode(frame,dt,false);}
    fn paint_mode(&mut self,frame:&mut RawFrame,dt:f32,calm:bool) {
        let (w,h)=(frame.width as usize,frame.height as usize);
        let step=((w as f32/1920.)*24.).round().max(4.) as usize;
        let cols=w.div_ceil(step);let rows=h.div_ceil(step);
        // Only the slowly moving refresh front is reduced. Painted RGB remains
        // full resolution; bilinear reconstruction keeps the front softly curved.
        let mask_scale=if w>=960 {4} else {1};
        let mw=w.div_ceil(mask_scale);let mh=h.div_ceil(mask_scale);
        if self.size!=(frame.width,frame.height) || self.canvas.len()!=frame.data.len() {
            self.size=(frame.width,frame.height);self.canvas=frame.data.clone();
            self.marks=vec![Mark::default();cols*rows];self.coverage=vec![0;w*h];self.mask=vec![0;mw*mh];self.previous=frame.data.clone();self.older=frame.data.clone();self.settled=frame.data.iter().map(|v|*v as u16*256).collect();return;
        }
        self.mask.fill(0);
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
            let scale=mask_scale as f32;
            let (mx,my)=((center_x+0.5)/scale-0.5,(center_y+0.5)/scale-0.5);
            let (length,width)=(length/scale,width/scale);
            let bx=(cs.abs()*length+sn.abs()*width).ceil();
            let by=(sn.abs()*length+cs.abs()*width).ceil();
            brush_coverage(&mut self.mask,mw,mh,mx,my,length,width,cs,sn,progress,bx,by);
            if progress>=1. {mark.active=false;}
        }}
        reconstruct_coverage(&self.mask,mw,mh,&mut self.coverage,w,h,mask_scale);
        // One Q15 response table per frame keeps the six-million-channel pass
        // integer-only. Pigment remains Q8, so gradual changes do not get stuck.
        let response=1.-(-dt.max(0.)/(0.09-0.04*self.load)).exp();
        let floor=0.25-0.15*self.load;
        let weights: [i32;256]=std::array::from_fn(|a|(response*(floor+(1.-floor)*a as f32/255.)*32768.).round() as i32);
        let rise=(420.*dt*256.).round() as i32;let fall=(600.*dt*256.).round() as i32;
        if calm {
            settle_pigments(&frame.data,&self.previous,&self.older,&mut self.settled,&mut self.canvas,&self.coverage,&weights,rise,fall);
        } else {
            for (at,&coverage) in self.coverage.iter().enumerate() {
                let alpha=coverage as u32;
                for c in 0..3 {
                    let i=at*3+c;
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
// Four interleaved RGB components, preserving the scalar Q8/Q15 arithmetic.
fn settle_pigments(target:&[u8],previous:&[u8],older:&[u8],settled:&mut [u16],canvas:&mut [u8],coverage:&[u8],weights:&[i32;256],rise:i32,fall:i32) {
    use wide::i32x4 as I;
    let last=target.len()-1;
    for (batch,output) in canvas.chunks_mut(4).enumerate() {
        let at=batch*4;let ids: [usize;4]=std::array::from_fn(|k|(at+k).min(last));
        let a=I::new(ids.map(|i|target[i] as i32));let b=I::new(ids.map(|i|previous[i] as i32));let c=I::new(ids.map(|i|older[i] as i32));
        let pigment=a.min(b).max(a.max(b).min(c))*256;
        let current=I::new(ids.map(|i|settled[i] as i32));
        let weight=I::new(ids.map(|i|weights[coverage[i/3] as usize]));
        let change=(((pigment-current)*weight+I::splat(16384))>>15u32).max(I::splat(-fall)).min(I::splat(rise));
        let next=(current+change).max(I::splat(0)).min(I::splat(65280));
        let bytes=((next+I::splat(128))>>8u32).to_array();let values=next.to_array();
        for k in 0..output.len() {settled[at+k]=values[k] as u16;output[k]=bytes[k] as u8;}
    }
}

fn reconstruct_coverage(mask:&[u8],mw:usize,mh:usize,output:&mut [u8],w:usize,h:usize,scale:usize) {
    if scale==1 {output.copy_from_slice(mask);return;}
    // Pixel-center coordinates have denominator 2*scale. Integer weights are
    // exact here and avoid a floating-point round operation for every pixel.
    let d=(2*scale) as u32;let denominator=d*d;
    debug_assert!(denominator.is_power_of_two());let shift=denominator.trailing_zeros();
    let coordinate=|at:usize,extent:usize| {
        let n=(2*at+1).saturating_sub(scale).min((extent-1)*d as usize);
        (n/d as usize,(n/d as usize+1).min(extent-1),(n%d as usize) as u32)
    };
    let columns:Vec<_>=(0..w).map(|x|coordinate(x,mw)).collect();
    for y in 0..h {
        let (y0,y1,fy)=coordinate(y,mh);
        for (x,&(x0,x1,fx)) in columns.iter().enumerate() {
            let a=mask[y0*mw+x0] as u32*(d-fx)+mask[y0*mw+x1] as u32*fx;
            let b=mask[y1*mw+x0] as u32*(d-fx)+mask[y1*mw+x1] as u32*fx;
            output[y*w+x]=((a*(d-fy)+b*fy+denominator/2)>>shift) as u8;
        }
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
 #[test] fn loaded_paint_keeps_fine_contrast_without_admitting_a_flash() {
  let base=RawFrame::filled(192,108,70,90,110);let dt=1./15.;
  let mut slow=Repaint::default();let mut loaded=Repaint::default();loaded.set_load(1.);
  for p in [&mut slow,&mut loaded] {
   let mut f=base.clone();p.paint_mode(&mut f,dt,true);
   f=RawFrame::filled(192,108,255,255,255);p.paint_mode(&mut f,dt,true);assert_eq!(f.data,base.data);
   f=base.clone();p.paint_mode(&mut f,dt,true);assert_eq!(f.data,base.data);
  }
  let mut detail=base.clone();for y in 20..88 {for x in 92..95 {detail.data[(y*192+x)*3..][..3].fill(130);}}
  let mut a=base.clone();let mut b=base.clone();
  for _ in 0..5 {a=detail.clone();slow.paint_mode(&mut a,dt,true);b=detail.clone();loaded.paint_mode(&mut b,dt,true);}
  let error=|f:&RawFrame|f.data.iter().zip(&detail.data).map(|(a,b)|a.abs_diff(*b) as usize).sum::<usize>();
  assert!(error(&b)<error(&a),"loaded marks should retain more of the narrow painted detail");
  assert!(b.data.iter().zip(&base.data).zip(&detail.data).all(|((b,a),t)|*b>=(*a).min(*t)&&*b<=(*a).max(*t)));
 }
 #[test] fn batched_pigment_matches_scalar_for_extremes_and_partial_batches() {
  for count in [3,9,111,513] {
   let target:Vec<u8>=(0..count).map(|i|((i*71)%256) as u8).collect();let previous:Vec<u8>=target.iter().rev().copied().collect();let older:Vec<u8>=target.iter().map(|v|255-v).collect();
   let coverage:Vec<u8>=(0..count/3).map(|i|((i*29)%256) as u8).collect();let weights=std::array::from_fn(|i|(i as i32*32768)/255);
   let mut state:Vec<u16>=(0..count).map(|i|((i*4171)%65281) as u16).collect();let mut expected=state.clone();let mut out=vec![0;count];
   for i in 0..count {let a=target[i];let b=previous[i];let c=older[i];let t=a.min(b).max(a.max(b).min(c)) as i32*256;let delta=t-expected[i] as i32;
    let change=((delta*weights[coverage[i/3] as usize]+16384)>>15).clamp(-5120,3584);expected[i]=(expected[i] as i32+change).clamp(0,65280) as u16;}
   settle_pigments(&target,&previous,&older,&mut state,&mut out,&coverage,&weights,3584,5120);
   assert_eq!(state,expected);assert!(out.iter().zip(expected).all(|(a,b)|*a as u16==(b+128)>>8));
  }
 }
 #[test] fn integer_mask_reconstruction_matches_float_pixel_centers() {
  for scale in [2,4] {
   let (w,h)=(37usize,19usize);let mw=w.div_ceil(scale);let mh=h.div_ceil(scale);
   let mask:Vec<u8>=(0..mw*mh).map(|i|((i*73+41)%256) as u8).collect();let mut out=vec![0;w*h];
   reconstruct_coverage(&mask,mw,mh,&mut out,w,h,scale);
   for y in 0..h {for x in 0..w {
    let sx=((x as f32+0.5)/scale as f32-0.5).clamp(0.,(mw-1) as f32);let sy=((y as f32+0.5)/scale as f32-0.5).clamp(0.,(mh-1) as f32);
    let (x0,y0)=(sx as usize,sy as usize);let (x1,y1)=((x0+1).min(mw-1),(y0+1).min(mh-1));let (fx,fy)=(sx.fract(),sy.fract());
    let a=mask[y0*mw+x0] as f32*(1.-fx)+mask[y0*mw+x1] as f32*fx;let b=mask[y1*mw+x0] as f32*(1.-fx)+mask[y1*mw+x1] as f32*fx;
    assert_eq!(out[y*w+x],(a*(1.-fy)+b*fy).round() as u8);
   }}
  }
 }
 #[test] fn coarse_refresh_mask_has_smooth_edges_and_handles_odd_dimensions() {
  let mut out=vec![0;7*5];reconstruct_coverage(&[96;4],2,2,&mut out,7,5,4);assert!(out.iter().all(|v|*v==96));
  reconstruct_coverage(&[0,255,0,255],2,2,&mut out,7,5,4);
  assert!(out[..7].windows(2).all(|p|p[0]<=p[1] && p[1]-p[0]<=64));
  let mut p=Repaint::default();let mut f=RawFrame::filled(961,541,0,0,0);p.paint_mode(&mut f,1./30.,true);
  f=RawFrame::filled(961,541,100,120,80);p.paint_mode(&mut f,1./30.,true);
  assert_eq!(p.mask.len(),241*136);assert_eq!(f.data.len(),961*541*3);
  assert!(p.coverage.iter().any(|v|*v>0 && *v<255));
 }
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
