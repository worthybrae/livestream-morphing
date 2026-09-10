//! Bounded, source-aware brushwork. Stroke identities and drawing order are
//! fixed in image space; the signature animation is applied by Editions once.
use super::{RawFrame, edition_forms::{knife_atmosphere, knife_day_tint,
    knife_highlight_table, knife_night_strength}};
use wide::{f32x8 as F, CmpLt};

#[derive(Clone, Copy)]
struct Site { x: usize, y: usize, seed: f32, variety: f32 }

#[derive(Default)]
pub(super) struct OilPaint {
    size: (usize, usize),
    sites: [Vec<Site>; 3],
    frames: usize,
    source: Vec<u8>,
}

fn hash(x: usize, y: usize) -> u32 {
    let v=(x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263);
    let v=(v^(v>>13)).wrapping_mul(1274126177); v^(v>>16)
}
fn random(x: usize,y: usize)->f32 { (hash(x,y)&65535) as f32/65535. }
fn smooth(t:f32)->f32 { let t=t.clamp(0.,1.);t*t*(3.-2.*t) }
fn difference(a:[f32;3],b:[f32;3])->f32 {
    ((a[0]-b[0]).abs()*0.3+(a[1]-b[1]).abs()*0.5+(a[2]-b[2]).abs()*0.2)/255.
}
fn pixel(src:&[u8],w:usize,x:usize,y:usize)->[f32;3] {
    let i=(y*w+x)*3; [src[i] as f32,src[i+1] as f32,src[i+2] as f32]
}

impl OilPaint {
    fn prepare(&mut self,w:usize,h:usize) {
        if self.size==(w,h) {return;}
        self.size=(w,h); self.source.resize(w*h*3,0);
        let scale=w as f32/1920.;
        for (coat,spacing) in [29.,13.,5.].into_iter().enumerate() {
            let step=(spacing*scale).max(2.) as usize;
            let sites=&mut self.sites[coat];sites.clear();
            for y in (0..h).step_by(step) { for x in (0..w).step_by(step) {
                let seed=random(x+coat*71,y+37);let variety=random(x+291,y+coat*53);
                sites.push(Site { x:(x as f32+(seed-0.5)*step as f32*0.8).clamp(0.,(w-1) as f32) as usize,
                    y:(y as f32+(variety-0.5)*step as f32*0.8).clamp(0.,(h-1) as f32) as usize,seed,variety });
            }}
            // Avoid raster-order combing: deterministic order, independent of frame.
            sites.sort_unstable_by_key(|s|hash(s.x+coat*733,s.y+941));
        }
    }
    pub(super) fn draw(&mut self,frame:&mut RawFrame,luma:&[f32],environment:[f32;3],variant:u32) {
        let (w,h)=(frame.width as usize,frame.height as usize);
        self.prepare(w,h);
        let scale=w as f32/1920.;
        let night=knife_night_strength(environment[0]);
        let grade=[1.4-night*1.18,1.04-night*0.42,0.75-night*0.68];
        let tint=knife_day_tint(environment);let highlights=knife_highlight_table(night);
        for (dst,src) in self.source.chunks_exact_mut(3).zip(frame.data.chunks_exact(3)) {
            let (gain,weight)=highlights[*src.iter().max().unwrap() as usize];
            let c=knife_atmosphere([src[0] as f32,src[1] as f32,src[2] as f32],grade,gain,weight,tint);
            for k in 0..3 {dst[k]=c[k].clamp(0.,255.) as u8;}
        }
        // A coarse pigment ground replaces photographic gaps between strokes.
        // Each tile loads color from its own center, preserving local color
        // rather than averaging across high-contrast boundaries.
        let tile=(6.*scale).max(2.) as usize;
        for y in (0..h).step_by(tile) { for x in (0..w).step_by(tile) {
            let c=sample_color(&self.source,w,h,(x+tile/2).min(w-1),(y+tile/2).min(h-1),tile/3);
            for yy in y..(y+tile).min(h) {for xx in x..(x+tile).min(w) {
                let i=(yy*w+xx)*3;
                for k in 0..3 {frame.data[i+k]=(c[k]*0.84) as u8;}
            }}
        }}
        self.frames+=1;
        let profile=self.frames==10 && std::env::var_os("OIL_PROFILE").is_some();
        // Each coat only repairs what larger strokes could not describe. The
        // smooth error response avoids stochastic births or threshold flashing.
        for coat in 0..3 {
            let started=std::time::Instant::now();
            let spacing=[29.,13.,5.,2.8][coat]*scale;
            for &site in &self.sites[coat] {
                let (x,y)=(site.x,site.y);
                let radius=if coat<2 {(spacing*0.16).max(1.) as usize} else {0};
                let color=sample_color(&self.source,w,h,x,y,radius);
                let old=pixel(&frame.data,w,x,y);
                let err=difference(color,old);
                let alpha=smooth((err-[0.012,0.028,0.035][coat])/[0.07,0.10,0.13][coat]);
                if alpha<0.025 {continue;}
                let d=(spacing*0.30).max(1.) as usize;
                let (dx,dy)=gradient(luma,w,h,x,y,d);
                let energy=(dx*dx+dy*dy).sqrt();
                // A regularized tensor aligns confident edges but allows a
                // restrained, stable hand direction in featureless passages.
                let base=(site.seed-0.5)*1.9;
                let (sn,cs)=base.sin_cos();
                let a=-dy*cs+dx*sn;let b=dy*sn+dx*cs;
                let angle=base+0.95*(2.*a*b)/(a*a+b*b+0.004);
                let (mut sn,mut cs)=angle.sin_cos();
                let brush_scale=if coat==2 {0.76} else if coat==1 {0.87} else {1.};
                let mut length=spacing*(1.1+site.variety*1.25)*brush_scale;
                let mut width=spacing*(0.33+site.seed*0.30)*brush_scale;
                if variant==1 {length*=1.45;width*=1.15;}
                if variant==2 {length*=1.05;width*=1.35;}
                if coat>=2 {
                    // Compact islands (heads, lights, color accents) become
                    // short dabs. Confident line segments get a fine brush.
                    let center=luma[y*w+x];
                    let xx=luma[y*w+x.saturating_sub(d)]+luma[y*w+(x+d).min(w-1)]-2.*center;
                    let yy=luma[y.saturating_sub(d)*w+x]+luma[(y+d).min(h-1)*w+x]-2.*center;
                    let blob=(xx*yy).max(0.)/(xx*xx+yy*yy+0.0001);
                    let compact=smooth((blob-0.12)/0.25);
                    length*=1.-compact*0.65;
                    if energy>0.06 && compact<0.3 {
                        // A signless tangent: changing contrast polarity does
                        // not reverse an asymmetric stroke.
                        let norm=(dx*dx+dy*dy).sqrt();
                        cs=-dy/norm;sn=dx/norm;
                        if cs<0. {cs=-cs;sn=-sn;}
                    }
                    width*=0.85;
                }
                // Measure how far this pigment can travel before crossing into
                // another form. A narrow head cannot become a 30px streak.
                let mut negative=length;let mut positive=length;
                for sign in [-1.,1.] {
                    let mut support=0.;
                    let samples=if coat==2 {2} else {4};
                    for k in 1..=samples {
                        let dist=length*k as f32/samples as f32;
                        let xx=(x as f32+cs*dist*sign).clamp(0.,(w-1) as f32) as usize;
                        let yy=(y as f32+sn*dist*sign).clamp(0.,(h-1) as f32) as usize;
                        let delta=difference(color,pixel(&self.source,w,xx,yy));
                        support+=1.-smooth((delta-0.055)/0.12);
                    }
                    let extent=length*(0.22+support*0.78/samples as f32);
                    if sign<0. {negative=extent;} else {positive=extent;}
                }
                let shift=(positive-negative)*0.5;
                length=(positive+negative)*0.5;
                let cx=x as f32+cs*shift;let cy=y as f32+sn*shift;
                let bend=(site.variety-0.5)*0.30;
                let stroke=Stroke {cx,cy,cs,sn,length:length.max(0.8),width:width.max(0.65),
                    seed:site.seed,bend,opacity:alpha,color,variant};
                stroke.draw(frame);
            }
            if profile {eprintln!("OIL coat={coat} ms={:.2}",started.elapsed().as_secs_f64()*1000.);}
        }
    }
}

fn gradient(luma:&[f32],w:usize,h:usize,x:usize,y:usize,d:usize)->(f32,f32) {
    (luma[y*w+(x+d).min(w-1)]-luma[y*w+x.saturating_sub(d)],
     luma[(y+d).min(h-1)*w+x]-luma[y.saturating_sub(d)*w+x])
}

// Bilateral pigment loading keeps a red jacket red next to gray pavement.
fn sample_color(src:&[u8],w:usize,h:usize,x:usize,y:usize,r:usize)->[f32;3] {
    let center=pixel(src,w,x,y);if r==0{return center;}
    let mut sum=center;let mut total=1.;
    for (xx,yy) in [(x.saturating_sub(r),y),((x+r).min(w-1),y),(x,y.saturating_sub(r)),(x,(y+r).min(h-1))] {
        let p=pixel(src,w,xx,yy);let weight=1./(1.+difference(center,p)*35.);
        total+=weight;for k in 0..3 {sum[k]+=p[k]*weight;}
    }
    sum.map(|v|v/total)
}

struct Stroke {cx:f32,cy:f32,cs:f32,sn:f32,length:f32,width:f32,seed:f32,bend:f32,opacity:f32,color:[f32;3],variant:u32}
impl Stroke {
    fn draw(&self,frame:&mut RawFrame) {
        let (w,h)=(frame.width as usize,frame.height as usize);
        let reach=self.width*(1.15+self.bend.abs());
        let bx=self.cs.abs()*self.length+self.sn.abs()*reach;
        let by=self.sn.abs()*self.length+self.cs.abs()*reach;
        let minx=(self.cx-bx-1.).max(0.) as usize;let maxx=(self.cx+bx+1.).min((w-1) as f32) as usize;
        let miny=(self.cy-by-1.).max(0.) as usize;let maxy=(self.cy+by+1.).min((h-1) as f32) as usize;
        if minx>maxx || miny>maxy {return;}
        let inverse=[1./self.length,1./self.width];
        let inv_cs=if self.cs.abs()>1e-6 {Some(1./self.cs)} else {None};
        let inv_sn=if self.sn.abs()>1e-6 {Some(1./self.sn)} else {None};
        for y in miny..=maxy {
            // Clip to the rotated brush rectangle before shading. Long diagonal
            // strokes otherwise shade several times their visible area.
            let dy=y as f32-self.cy;
            let mut left=minx as f32;let mut right=maxx as f32;
            if let Some(inv)=inv_cs {
                let center=self.cx-self.sn*dy*inv;
                let radius=(self.length+self.length/self.width*0.5+1.)*inv.abs();
                left=left.max(center-radius);right=right.min(center+radius);
            }
            if let Some(inv)=inv_sn {
                let center=self.cx+self.cs*dy*inv;
                let radius=(reach+1.5)*inv.abs();
                left=left.max(center-radius);right=right.min(center+radius);
            }
            let start=left.floor().max(minx as f32) as usize;
            let end=right.ceil().min(maxx as f32) as usize;
            if start>end {continue;}
            let row=&mut frame.data[(y*w+start)*3..(y*w+end+1)*3];
            for (batch,pixels) in row.chunks_mut(24).enumerate() {self.pixels(pixels,start+batch*8,y,inverse);}
        }
    }
    #[inline]
    fn pixels(&self,pixels:&mut[u8],x:usize,y:usize,inverse:[f32;2]) {
        let ox=F::new(std::array::from_fn(|k|(x+k) as f32-self.cx));let oy=y as f32-self.cy;
        let u=(ox*self.cs+F::splat(oy*self.sn))*inverse[0];
        let v=(-ox*self.sn+F::splat(oy*self.cs))*inverse[1];
        let belly=(F::ONE-u*u).max(F::ZERO);
        let v=v-belly*self.bend;
        let tip=((u.abs()-0.65)*(1./0.35)).max(F::ZERO);
        let cap=(F::ONE-tip*tip).max(F::ZERO).sqrt();
        let pressure=(F::splat(0.80)-u*0.12+belly*0.12)*cap;
        let edge=(v.abs()-pressure).max(u.abs()-1.);
        if edge.cmp_lt(F::splat(inverse[1])).none() {return;}
        // Filaments vary along the brush travel, not as screen-space noise.
        let phase=v*(7.+self.seed*8.)+u*(0.22+self.seed*0.35)+self.seed*31.;
        let groove=((phase-phase.floor())-0.5).abs()*2.;
        let dry=(u+0.1).max(F::ZERO)*(F::ONE-groove)*0.22;
        let coverage=(F::splat(0.5)-edge*self.width).max(F::ZERO).min(F::ONE)*(F::splat(0.97)-dry)*self.opacity;
        let shoulder=(F::ONE-(v+pressure*0.60).abs()*7.).max(F::ZERO);
        let valley=(F::ONE-(v-pressure*0.76).abs()*8.).max(F::ZERO);
        let grain=(groove-0.5)*belly;
        let relief=if self.variant==2 {23.} else {16.};
        let light=shoulder*relief-valley*(relief*0.72)+grain*4.+(self.seed-0.5)*5.;
        let mut colors=[[0.;8];3];
        for k in 0..3 {
            let old=if pixels.len()==24 {
                F::new([pixels[k] as f32,pixels[3+k] as f32,pixels[6+k] as f32,pixels[9+k] as f32,pixels[12+k] as f32,pixels[15+k] as f32,pixels[18+k] as f32,pixels[21+k] as f32])
            } else {F::new(std::array::from_fn(|n|pixels.get(n*3+k).copied().unwrap_or(0) as f32))};
            // Pigment-to-pigment mixing, with a little underpainting exposed at
            // the dry tip. No synthetic white outlines or dark polygon borders.
            colors[k]=(old+(F::splat(self.color[k])+light-old)*coverage).max(F::ZERO).min(F::splat(255.)).to_array();
        }
        for (n,p) in pixels.chunks_exact_mut(3).enumerate() {for k in 0..3 {p[k]=colors[k][n] as u8;}}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn clipped_strokes_match_full_rectangle_rasterization() {
        for angle in [-1.55_f32,-0.8,-0.05,0.,0.3,0.9,1.55] {
            for (length,width) in [(7.,1.),(23.,4.),(58.,17.)] {
                let (sn,cs)=angle.sin_cos();
                let stroke=Stroke {cx:43.3,cy:33.7,cs,sn,length,width,seed:0.73,bend:0.14,opacity:0.8,color:[160.,70.,110.],variant:0};
                let mut clipped=RawFrame::filled(96,72,50,60,70);let mut brute=clipped.clone();
                stroke.draw(&mut clipped);
                let reach=width*(1.15+stroke.bend.abs());
                let bx=cs.abs()*length+sn.abs()*reach;let by=sn.abs()*length+cs.abs()*reach;
                let minx=(stroke.cx-bx-1.).max(0.) as usize;let maxx=(stroke.cx+bx+1.).min(95.) as usize;
                let miny=(stroke.cy-by-1.).max(0.) as usize;let maxy=(stroke.cy+by+1.).min(71.) as usize;
                for y in miny..=maxy {
                    for x in minx..=maxx {stroke.pixels(&mut brute.data[(y*96+x)*3..(y*96+x)*3+3],x,y,[1./length,1./width]);}
                }
                assert_eq!(clipped.data,brute.data,"angle={angle} length={length} width={width}");
            }
        }
    }
    #[test] fn pigment_does_not_average_away_small_red_forms() {
        let mut f=RawFrame::filled(9,9,100,100,100);
        f.data[(4*9+4)*3..(4*9+4)*3+3].copy_from_slice(&[230,20,20]);
        let c=sample_color(&f.data,9,9,4,4,2);
        assert!(c[0]>180. && c[1]<55.,"{c:?}");
    }
    #[test] fn stroke_geometry_is_bounded_on_tiny_frames() {
        for (w,h) in [(1,1),(2,3),(64,36)] {
            let mut f=RawFrame::filled(w,h,90,100,110);let luma=vec![0.4;(w*h) as usize];
            let mut painter=OilPaint::default();
            for variant in 0..3 {painter.draw(&mut f,&luma,[14.,0.3,0.],variant);assert_eq!(f.data.len(),(w*h*3) as usize);}
        }
    }
    #[test] fn small_figures_remain_separate_from_quiet_ground() {
        let (w,h)=(512,288);
        let mut source=RawFrame::filled(w,h,135,130,125);
        for center in [180,240,300] {
            for y in 130..140 {for x in center-2..=center+2 {
                source.data[(y*w as usize+x)*3..(y*w as usize+x)*3+3].copy_from_slice(&[30,35,40]);
            }}
        }
        let luma:Vec<_>=source.data.chunks_exact(3).map(|p|(p[0] as f32*0.299+p[1] as f32*0.587+p[2] as f32*0.114)/255.).collect();
        for variant in 0..3 {
            let mut f=source.clone();OilPaint::default().draw(&mut f,&luma,[14.,0.3,0.],variant);
            let gray=|x:usize,y:usize| {let p=pixel(&f.data,w as usize,x,y);(p[0]+p[1]+p[2])/3.};
            let ground=(110..160).flat_map(|y|(350..400).map(move|x|(x,y))).map(|(x,y)|gray(x,y)).sum::<f32>()/2500.;
            for center in [180,240,300] {
                let figure=(131..139).flat_map(|y|(center-1..=center+1).map(move|x|(x,y))).map(|(x,y)|gray(x,y)).sum::<f32>()/24.;
                assert!(ground-figure>45.,"variant={variant} ground={ground} figure={figure}");
            }
        }
    }
    #[test] fn repeated_frames_have_identical_stroke_placement() {
        let mut source=RawFrame::filled(128,72,80,90,100);
        for y in 15..60 {for x in 40..70 {source.data[(y*128+x)*3..(y*128+x)*3+3].copy_from_slice(&[190,30,50]);}}
        let luma:Vec<_>=source.data.chunks_exact(3).map(|p|p[0] as f32/255.).collect();
        let mut a=source.clone();let mut b=source.clone();let mut painter=OilPaint::default();
        painter.draw(&mut a,&luma,[14.,0.3,0.],0);painter.draw(&mut b,&luma,[14.,0.3,0.],0);
        assert_eq!(a.data,b.data);
    }
}
