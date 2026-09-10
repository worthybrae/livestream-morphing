use crate::effects::RawFrame;
use crate::registry::{Effect, FrameCtx, ParamDef, ParamValues};

/// A single native renderer shared by production and local replay.
#[derive(Default)]
pub struct LivingInk {
    luma: Vec<f32>, smooth: Vec<f32>, broad: Vec<f32>, scratch: Vec<f32>,
    magnitude: Vec<f32>, direction: Vec<u8>, fine_lines: Vec<f32>,
    broad_lines: Vec<f32>, previous: Vec<f32>, trail: Vec<f32>,
    has_previous: bool,
}

// Separable box smoothing, O(pixels), independent of radius.
fn blur(src: &[f32], tmp: &mut [f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
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

fn contours(src: &[f32], mag: &mut [f32], dir: &mut [u8], out: &mut [f32], w: usize, h: usize, threshold: f32) {
    mag.fill(0.0); out.fill(0.0);
    if w < 3 || h < 3 { return; }
    for y in 1..h-1 { for x in 1..w-1 {
        let i = y*w+x;
        let gx = -src[i-w-1]+src[i-w+1]-2.0*src[i-1]+2.0*src[i+1]-src[i+w-1]+src[i+w+1];
        let gy = -src[i-w-1]-2.0*src[i-w]-src[i-w+1]+src[i+w-1]+2.0*src[i+w]+src[i+w+1];
        mag[i] = (gx*gx+gy*gy).sqrt()/4.0;
        dir[i] = if gy.abs() < gx.abs()*0.4142 { 0 }
            else if gx.abs() < gy.abs()*0.4142 { 1 }
            else if gx*gy > 0.0 { 2 } else { 3 };
    }}
    for y in 1..h-1 { for x in 1..w-1 {
        let i=y*w+x;
        let d = match dir[i] { 0 => 1, 1 => w, 2 => w+1, _ => w-1 };
        let m = mag[i];
        // Keep a single ridge even when two pixels have identical strength.
        if m >= mag[i-d] && m > mag[i+d] {
            let v = ((m-threshold)/(threshold*1.5).max(8.0)).clamp(0.0,1.0);
            out[i] = v*v*(3.0-2.0*v);
        }
    }}
}

impl Effect for LivingInk {
    fn id(&self) -> &'static str { "living_ink" }
    fn name(&self) -> &'static str { "Living Ink" }
    fn params(&self) -> Vec<ParamDef> { vec![
        ParamDef::new("fine_threshold","Fine threshold",2.0,64.0,10.0,1.0),
        ParamDef::new("broad_threshold","Broad threshold",2.0,64.0,9.0,1.0),
        ParamDef::new("fine_weight","Fine line weight",0.0,1.0,0.9,0.01),
        ParamDef::new("broad_weight","Broad line weight",0.0,1.0,0.25,0.01),
        ParamDef::new("smoothing","Smoothing radius",1.0,3.0,1.0,1.0),
        ParamDef::new("trail_strength","Trail strength",0.0,0.5,0.22,0.01),
        ParamDef::new("trail_half_life","Trail half-life (seconds)",0.1,1.0,0.4,0.05),
        ParamDef::new("charcoal","Charcoal",0.0,64.0,24.0,1.0),
        ParamDef::new("highlight","Highlight",128.0,255.0,235.0,1.0),
    ] }
    fn init(&mut self, w: u32, h: u32) {
        let n=(w*h) as usize;
        self.luma=vec![0.0;n]; self.smooth=vec![0.0;n]; self.broad=vec![0.0;n];
        self.scratch=vec![0.0;n]; self.magnitude=vec![0.0;n]; self.direction=vec![0;n];
        self.fine_lines=vec![0.0;n]; self.broad_lines=vec![0.0;n];
        self.previous=vec![0.0;n]; self.trail=vec![0.0;n]; self.has_previous=false;
    }
    fn apply(&mut self, frame: &mut RawFrame, params: &ParamValues, ctx: &FrameCtx) {
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
        blur(&self.smooth,&mut self.scratch,&mut self.broad,w,h,2);
        contours(&self.smooth,&mut self.magnitude,&mut self.direction,&mut self.fine_lines,w,h,get("fine_threshold"));
        contours(&self.broad,&mut self.magnitude,&mut self.direction,&mut self.broad_lines,w,h,get("broad_threshold"));
        let dt=if ctx.delta_seconds.is_finite() { ctx.delta_seconds.max(0.0) } else { 1.0/30.0 };
        let decay=2.0_f32.powf(-dt/get("trail_half_life"));
        let fine=get("fine_weight"); let broad=get("broad_weight");
        let strength=get("trail_strength"); let bg=get("charcoal"); let hi=get("highlight");
        for ((((pixel,previous),trail),fine_line),broad_line) in frame.data.chunks_exact_mut(3)
            .zip(&mut self.previous).zip(&mut self.trail).zip(&self.fine_lines).zip(&self.broad_lines) {
            let line=(fine*fine_line).max(broad*broad_line);
            let departed=if self.has_previous { (*previous-line).max(0.0) } else { 0.0 };
            *trail=(*trail*decay).max(departed);
            let coverage=line.max(strength * *trail);
            pixel.fill((bg+(hi-bg)*coverage).round() as u8);
            *previous=line;
        }
        self.has_previous=true;
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
