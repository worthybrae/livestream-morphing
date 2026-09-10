use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// A single native renderer shared by production and local replay.
#[derive(Default)]
pub struct LivingInk {
    luma: Vec<f32>, smooth: Vec<f32>, broad: Vec<f32>, scratch: Vec<f32>,
    magnitude: Vec<f32>, direction: Vec<u8>, fine_lines: Vec<f32>,
    broad_lines: Vec<f32>, previous: Vec<f32>, trail: Vec<f32>,
    has_previous: bool,
    warp: super::distortion::Distortion,
}

// Separable box smoothing, O(pixels), independent of radius.
pub(crate) fn blur(src: &[f32], tmp: &mut [f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let n = (2*r+1) as f32;
    for y in 0..h {
        let mut sum = (0..=2*r).map(|k| src[y*w+k.saturating_sub(r).min(w-1)]).sum::<f32>();
        for x in 0..w {
            tmp[y*w+x] = sum/n;
            sum += src[y*w+(x+r+1).min(w-1)] - src[y*w+x.saturating_sub(r)];
        }
    }
    // Walk contiguous rows so each cache line is used before advancing.
    // Summation order for each pixel is identical to the column-wise pass.
    let mut sums = vec![0.0; w];
    for k in 0..=2*r {
        let row=&tmp[k.saturating_sub(r).min(h-1)*w..][..w];
        for (sum,value) in sums.iter_mut().zip(row) { *sum += value; }
    }
    for y in 0..h {
        let above=&tmp[y.saturating_sub(r)*w..][..w];
        let below=&tmp[(y+r+1).min(h-1)*w..][..w];
        let out=&mut dst[y*w..][..w];
        for (((value,sum),a),b) in out.iter_mut().zip(&mut sums).zip(above).zip(below) {
            *value=*sum/n;
            *sum += b-a;
        }
    }
}

fn contours(src: &[f32], mag: &mut [f32], dir: &mut [u8], out: &mut [f32], w: usize, h: usize, threshold: f32, body: f32, subpixel: bool) {
    mag.fill(0.0); out.fill(0.0);
    if w < 3 || h < 3 { return; }
    let threshold_squared=threshold*threshold;
    for y in 1..h-1 {
        let a=&src[(y-1)*w..][..w]; let b=&src[y*w..][..w]; let c=&src[(y+1)*w..][..w];
        let mr=&mut mag[y*w..][..w]; let dr=&mut dir[y*w..][..w]; let output=&mut out[y*w..][..w];
        for x in 1..w-1 {
        let gx=-a[x-1]+a[x+1]-2.0*b[x-1]+2.0*b[x+1]-c[x-1]+c[x+1];
        let gy=-a[x-1]-2.0*a[x]-a[x+1]+c[x-1]+2.0*c[x]+c[x+1];
        // Squared magnitudes preserve ridge ordering. Flat areas need no sqrt or direction.
        mr[x]=(gx*gx+gy*gy)/16.0;
        if mr[x] <= threshold_squared { continue; }
        if body == 1.0 && !subpixel {
            let v = ((mr[x].sqrt()-threshold)/(threshold*1.5).max(8.0)).clamp(0.0,1.0);
            output[x] = v*v*(3.0-2.0*v);
            continue;
        }
        dr[x] = if gy.abs() < gx.abs()*0.4142 { 0 }
            else if gx.abs() < gy.abs()*0.4142 { 1 }
            else if gx*gy > 0.0 { 2 } else { 3 };
    }}
    if body == 1.0 && !subpixel { return; }
    for y in 1..h-1 { for x in 1..w-1 {
        let i=y*w+x;
        if mag[i] <= threshold_squared { continue; }
        let d = match dir[i] { 0 => 1, 1 => w, 2 => w+1, _ => w-1 };
        let squared=mag[i];
        let m=squared.sqrt();
        // Keep a single ridge even when two pixels have identical strength.
        let ridge = squared >= mag[i-d] && squared > mag[i+d];
        if ridge || body > 0.0 {
            let v = ((m-threshold)/(threshold*1.5).max(8.0)).clamp(0.0,1.0);
            // Preserve the smooth gradient shoulders for fuller, antialiased strokes.
            // Uniform source regions still have zero gradient and remain charcoal.
            let coverage=v*v*(3.0-2.0*v);
            if subpixel && ridge {
                let left=mag[i-d].sqrt(); let right=mag[i+d].sqrt();
                let denominator=left-2.0*m+right;
                let offset=if denominator.abs()>0.0001 { (0.5*(left-right)/denominator).clamp(-0.5,0.5) } else { 0.0 };
                out[i]=out[i].max(coverage*(1.0-offset.abs()));
                let neighbor=if offset<0.0 { i-d } else { i+d };
                out[neighbor]=out[neighbor].max(coverage*offset.abs());
            } else {
                out[i]=out[i].max(coverage * if ridge { 1.0 } else { body });
            }
        }
    }}
}

impl Effect for LivingInk {
    fn id(&self) -> &'static str { "living_ink" }
    fn name(&self) -> &'static str { "Living Ink" }
    fn params(&self) -> Vec<ParamDef> { vec![
        ParamDef::new("fine_threshold","Fine threshold",2.0,64.0,10.0,1.0),
        ParamDef::new("broad_threshold","Broad threshold",2.0,64.0,9.0,1.0),
        ParamDef::new("fine_weight","Fine line weight",0.0,2.0,0.9,0.01),
        ParamDef::new("broad_weight","Broad line weight",0.0,1.0,0.25,0.01),
        ParamDef::new("subpixel","Subpixel contours",0.0,1.0,0.0,1.0),
        ParamDef::new("warp_amplitude","Flow amplitude",0.0,0.04,0.0,0.001),
        ParamDef::new("warp_frequency","Flow frequency",1.0,20.0,8.0,0.5),
        ParamDef::new("warp_period","Flow period (seconds)",2.0,30.0,8.0,0.5),
        ParamDef::new("stroke_body","Stroke body",0.0,1.0,0.0,0.05),
        ParamDef::new("smoothing","Smoothing radius",1.0,3.0,1.0,1.0),
        ParamDef::new("trail_strength","Trail strength",0.0,0.5,0.22,0.01),
        ParamDef::new("trail_half_life","Trail half-life (seconds)",0.1,1.0,0.4,0.05),
        ParamDef::new("charcoal","Charcoal",0.0,64.0,24.0,1.0),
        ParamDef::new("highlight","Highlight",128.0,255.0,235.0,1.0),
    ] }
    fn init(&mut self, w: u32, h: u32) {
        let n=(w*h) as usize;
        self.luma=vec![0.0;n]; self.smooth=vec![0.0;n]; self.broad=Vec::new();
        self.scratch=vec![0.0;n]; self.magnitude=vec![0.0;n]; self.direction=vec![0;n];
        self.fine_lines=vec![0.0;n]; self.broad_lines=Vec::new();
        self.previous=Vec::new(); self.trail=Vec::new(); self.has_previous=false;
    }
    fn clear_history(&mut self) { self.has_previous=false; self.trail.fill(0.0); }
    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, ctx: &FrameCtx) {
        let dt=if ctx.delta_seconds.is_finite() { ctx.delta_seconds.max(0.0) } else {1.0/30.0};
        self.apply_at_time(frame,params,ctx,ctx.frame_number as f64*dt as f64);
    }
}
impl LivingInk {
    /// Explicit source time keeps flow stable when an interactive preview skips frames.
    pub fn apply_at_time(&mut self, frame: &mut RawFrame, params: &ParamValues, ctx: &FrameCtx, seconds: f64) {
        let w=frame.width as usize; let h=frame.height as usize;
        if w==0 || h==0 { return; }
        if self.luma.len()!=w*h { self.init(frame.width,frame.height); }
        let defs=self.params();
        let get=|id: &str| { let d=defs.iter().find(|d| d.id==id).unwrap();
            params.get(id).copied().filter(|v| v.is_finite()).unwrap_or(d.default).clamp(d.min,d.max) };
        for (i,p) in frame.data.chunks_exact(3).enumerate() {
            self.luma[i]=p[0] as f32*0.299+p[1] as f32*0.587+p[2] as f32*0.114;
        }
        blur(&self.luma,&mut self.scratch,&mut self.smooth,w,h,get("smoothing") as usize);
        let broad=get("broad_weight");
        if broad > 0.0 {
            self.broad.resize(w*h,0.0);
            self.broad_lines.resize(w*h,0.0);
            blur(&self.smooth,&mut self.scratch,&mut self.broad,w,h,2);
            contours(&self.broad,&mut self.magnitude,&mut self.direction,&mut self.broad_lines,w,h,get("broad_threshold"),get("stroke_body"),get("subpixel")>0.5);
        } else { self.broad=Vec::new(); self.broad_lines=Vec::new(); }
        contours(&self.smooth,&mut self.magnitude,&mut self.direction,&mut self.fine_lines,w,h,get("fine_threshold"),get("stroke_body"),get("subpixel")>0.5);
        let dt=if ctx.delta_seconds.is_finite() { ctx.delta_seconds.max(0.0) } else { 1.0/30.0 };
        let fine=get("fine_weight");
        // Only combine scales when the optional broad pass is enabled.
        if broad > 0.0 {
            for (line,coarse) in self.fine_lines.iter_mut().zip(&self.broad_lines) {
                *line=(fine * *line).max(broad*coarse);
            }
        }
        let scale=if broad > 0.0 {1.0} else {fine};
        let strength=get("trail_strength"); let bg=get("charcoal"); let hi=get("highlight");
        if strength == 0.0 {
            for (pixel,fine_line) in frame.data.chunks_exact_mut(3).zip(&self.fine_lines) {
                let coverage=scale*fine_line;
                pixel.fill((bg+(hi-bg)*coverage).round() as u8);
            }
            self.has_previous=false;
            self.previous=Vec::new(); self.trail=Vec::new();
        } else {
        self.previous.resize(w*h,0.0); self.trail.resize(w*h,0.0);
        let decay=2.0_f32.powf(-dt/get("trail_half_life"));
        for (((pixel,previous),trail),fine_line) in frame.data.chunks_exact_mut(3)
            .zip(&mut self.previous).zip(&mut self.trail).zip(&self.fine_lines) {
            let line=scale*fine_line;
            let departed=if self.has_previous { (*previous-line).max(0.0) } else { 0.0 };
            *trail=(*trail*decay).max(departed);
            let coverage=line.max(strength * *trail);
            pixel.fill((bg+(hi-bg)*coverage).round() as u8);
            *previous=line;
        }
        self.has_previous=true;
        }
        self.warp.apply_monochrome(frame,get("warp_amplitude"),get("warp_frequency"),((seconds/get("warp_period") as f64) as f32)*std::f32::consts::TAU);
    }
}
crate::register_effect!(LivingInk);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{default_params, Effect, FrameCtx};
    use crate::effects::RawFrame;

    fn boundary(x: usize) -> RawFrame {
        let mut f = RawFrame::filled(64, 32, 20, 20, 20);
        for y in 0..32 { for xx in x..64 {
            f.data[(y*64+xx)*3..(y*64+xx)*3+3].fill(220);
        }}
        f
    }
    fn draw(ink: &mut LivingInk, f: &mut RawFrame, dt: f32) {
        ink.apply(f, &default_params(&ink.params()), &FrameCtx {
            frame_number: 0, width: f.width, height: f.height, delta_seconds: dt,
        });
    }
    #[test]
    fn cache_friendly_blur_matches_column_reference() {
        for (w,h,r) in [(2,2,3),(7,9,1),(64,32,2)] {
            let src: Vec<f32> = (0..w*h).map(|i| ((i*71)%256) as f32).collect();
            let mut tmp=vec![0.0;w*h]; let mut actual=vec![0.0;w*h];
            blur(&src,&mut tmp,&mut actual,w,h,r);
            let mut expected=vec![0.0;w*h]; let n=(2*r+1) as f32;
            for x in 0..w {
                let mut sum=(0..=2*r).map(|k|tmp[k.saturating_sub(r).min(h-1)*w+x]).sum::<f32>();
                for y in 0..h {
                    expected[y*w+x]=sum/n;
                    sum += tmp[(y+r+1).min(h-1)*w+x]-tmp[y.saturating_sub(r)*w+x];
                }
            }
            assert_eq!(actual,expected);
        }
    }
    #[test]
    fn explicit_source_time_matches_native_frame_clock() {
        let mut native=LivingInk::default();let mut timed=LivingInk::default();
        let mut params=default_params(&native.params());
        params.insert("warp_amplitude".into(),0.03);
        for index in 0..5 {
            let ctx=FrameCtx{frame_number:index,width:64,height:32,delta_seconds:1.0/30.0};
            let mut a=boundary(20+index as usize);let mut b=a.clone();
            native.apply(&mut a,&params,&ctx);
            timed.apply_at_time(&mut b,&params,&ctx,index as f64*ctx.delta_seconds as f64);
            assert_eq!(a.data,b.data);
        }
    }
    #[test]
    fn optional_buffers_follow_enabled_features() {
        let mut ink=LivingInk::default(); ink.init(64,32);
        let mut params=default_params(&ink.params());
        let ctx=FrameCtx {frame_number:0,width:64,height:32,delta_seconds:1.0/30.0};
        for enabled in [false,true,false,true] {
            params.insert("broad_weight".into(),if enabled {0.25} else {0.0});
            params.insert("trail_strength".into(),if enabled {0.22} else {0.0});
            let mut frame=boundary(24); ink.apply(&mut frame,&params,&ctx);
            let expected=if enabled {64*32} else {0};
            for buffer in [&ink.broad,&ink.broad_lines,&ink.previous,&ink.trail] {
                assert_eq!(buffer.len(),expected);
                if !enabled {assert_eq!(buffer.capacity(),0);}
            }
            assert!(frame.data.iter().any(|v|*v>100));
        }
    }
    #[test]
    fn subpixel_ridge_shares_coverage_between_pixels() {
        let (w,h)=(9,5);
        let src: Vec<_>=(0..w*h).map(|i|if i%w>=4 {200.0} else {0.0}).collect();
        let mut mag=vec![0.0;w*h]; let mut dir=vec![0;w*h]; let mut out=vec![0.0;w*h];
        contours(&src,&mut mag,&mut dir,&mut out,w,h,10.0,0.0,true);
        assert!((out[2*w+3]-0.5).abs()<0.0001);
        assert!((out[2*w+4]-0.5).abs()<0.0001);
        assert_eq!(out[2*w+6],0.0);
    }
    #[test]
    fn disabled_trails_do_not_reappear_after_reenabling() {
        let mut ink=LivingInk::default();
        let mut p=default_params(&ink.params()); p.insert("trail_strength".into(),0.0);
        let ctx=FrameCtx {frame_number:0,width:64,height:32,delta_seconds:1.0/30.0};
        let mut f=boundary(24); ink.apply(&mut f,&p,&ctx);
        f=boundary(40); ink.apply(&mut f,&p,&ctx);
        p.insert("trail_strength".into(),0.22);
        f=boundary(40); ink.apply(&mut f,&p,&ctx);
        assert_eq!(f.data[(16*64+23)*3],24);
    }
    #[test]
    fn full_strokes_keep_flat_regions_empty_and_add_soft_shoulders() {
        let mut thin = LivingInk::default();
        let mut full = LivingInk::default();
        let mut a = boundary(24);
        let mut b = a.clone();
        draw(&mut thin, &mut a, 1.0/30.0);
        let mut params = default_params(&full.params());
        params.insert("stroke_body".into(), 1.0);
        full.apply(&mut b, &params, &FrameCtx {
            frame_number: 0, width: 64, height: 32, delta_seconds: 1.0/30.0,
        });
        assert!(b.data.iter().filter(|v| **v > 24).count() > a.data.iter().filter(|v| **v > 24).count());
        assert_eq!(b.data[(16*64+50)*3], 24);
        assert_eq!(b.data[(16*64+4)*3], 24);
    }
    #[test]
    fn living_ink_uniform_input_is_charcoal() {
        let mut ink = LivingInk::default(); ink.init(64,32);
        let mut f = RawFrame::filled(64,32,180,180,180); draw(&mut ink,&mut f,1.0/30.0);
        assert!(f.data.iter().all(|v| *v == 24));
    }
    #[test]
    fn living_ink_contours_leave_no_source_fills() {
        let mut ink = LivingInk::default(); ink.init(64,32);
        let mut f = boundary(24); draw(&mut ink,&mut f,1.0/30.0);
        assert!(f.data.iter().any(|v| *v > 100));
        assert_eq!(f.data[(16*64+50)*3],24);
        assert_eq!(f.data[(16*64+4)*3],24);
    }
    #[test]
    fn living_ink_motion_decays_and_static_does_not_accumulate() {
        let mut ink = LivingInk::default(); ink.init(64,32);
        let mut first = boundary(24); draw(&mut ink,&mut first,1.0/30.0);
        let mut still = boundary(24); draw(&mut ink,&mut still,1.0/30.0);
        assert_eq!(first.data,still.data);
        let mut moved = boundary(40); draw(&mut ink,&mut moved,1.0/30.0);
        assert!((20..28).any(|x| moved.data[(16*64+x)*3] > 24));
        for _ in 0..60 { moved = boundary(40); draw(&mut ink,&mut moved,1.0/30.0); }
        assert!((20..28).all(|x| moved.data[(16*64+x)*3] <= 27));
        ink.init(64,32);
        let mut reset = boundary(40); draw(&mut ink,&mut reset,1.0/30.0);
        assert_eq!(reset.data[(16*64+23)*3],24);
    }
    #[test]
    fn living_ink_decay_is_time_based() {
        fn after(steps: usize) -> Vec<u8> {
            let mut ink = LivingInk::default(); ink.init(64,32);
            let mut f = boundary(24); draw(&mut ink,&mut f,1.0/30.0);
            f = boundary(40); draw(&mut ink,&mut f,1.0/30.0);
            for _ in 0..steps { f = boundary(40); draw(&mut ink,&mut f,1.0/steps as f32); }
            f.data
        }
        assert_eq!(after(30),after(60));
    }
}
