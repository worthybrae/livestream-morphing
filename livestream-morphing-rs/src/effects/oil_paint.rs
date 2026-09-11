//! Bounded, source-aware brushwork. Stroke identities and drawing order are
//! fixed in image space; the signature animation is applied by Editions once.
use super::{RawFrame, edition_forms::{knife_atmosphere, knife_day_tint,
    knife_highlight_table, knife_night_strength}};
use wide::{f32x4 as F, i32x4 as I, u32x4 as U, CmpLt};

#[derive(Clone, Copy)]
struct Site { x: usize, y: usize, seed: f32, variety: f32 }

#[derive(Default)]
pub(super) struct OilPaint {
    size: (usize, usize),
    sites: [Vec<Site>; 6],
    accents: Vec<Stroke>,
    frames: usize,
    source: Vec<u8>,
    paint: Vec<u32>,
    underpaint: Vec<u32>,
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
        self.size=(w,h); self.source.resize(w*h*3,0); self.paint.resize(w*h,0); self.underpaint.resize(w.div_ceil(2)*h.div_ceil(2),0);
        let scale=w as f32/1920.;
        for (coat,spacing) in [29.,13.,5.,8.,52.,22.].into_iter().enumerate() {
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
        let passages=(7..=11).contains(&variant);
        let form_scale=variant>=14;
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
        let tile=((if passages || form_scale {14.} else {6.})*scale).max(2.) as usize;
        if !form_scale { for y in (0..h).step_by(tile) { for x in (0..w).step_by(tile) {
            let c=sample_color(&self.source,w,h,(x+tile/2).min(w-1),(y+tile/2).min(h-1),tile/3);
            for yy in y..(y+tile).min(h) {for xx in x..(x+tile).min(w) {
                self.paint[yy*w+xx]=pack(c.map(|v|v*0.84));
            }}
        }}
        }
        let layered=variant>=5;
        let (lw,lh)=(w.div_ceil(2),h.div_ceil(2));
        if form_scale {pigment_wash(&self.source,&mut self.underpaint,w,h,tile);}
        else if layered {
            for yy in 0..lh {for xx in 0..lw {self.underpaint[yy*lw+xx]=self.paint[yy*2*w+xx*2];}}
        }
        self.frames+=1;
        let profile=self.frames==10 && std::env::var_os("OIL_PROFILE").is_some();
        // Each coat only repairs what larger strokes could not describe. The
        // smooth error response avoids stochastic births or threshold flashing.
        for coat in 0..3 {
            let started=std::time::Instant::now();
            if coat==2 && variant>=6 {
                self.accents.clear();
                for &site in &self.sites[3] {
                    if variant>=9 {plan_form_accents(&self.source,&self.paint,w,h,site,scale,variant,&mut self.accents);}
                    else {plan_accents(&self.source,&self.paint,w,h,site,scale,variant,&mut self.accents);}
                }
                for stroke in &self.accents {stroke.draw(&mut self.paint,w,h);}
                if profile {eprintln!("OIL accents={} ms={:.2}",self.accents.len(),started.elapsed().as_secs_f64()*1000.);}
                continue;
            }
            let spacing=if form_scale {[52.,22.,5.][coat]*scale} else {[29.,13.,5.][coat]*scale};
            let site_index=if form_scale && coat<2 {coat+4} else {coat};
            for &site in &self.sites[site_index] {
                let (x,y)=(site.x,site.y);
                let radius=if coat<2 {(spacing*0.16).max(1.) as usize} else {0};
                let color=sample_color(&self.source,w,h,x,y,radius);
                let old=unpack(if layered && coat<2 {self.underpaint[(y/2)*lw+x/2]} else {self.paint[y*w+x]});
                let err=difference(color,old);
                let alpha=if form_scale && coat==0 {0.88} else if passages && coat==0 {0.94} else if passages && coat==1 {
                    smooth((err-0.05)/0.13)
                } else {smooth((err-[0.012,0.028,0.035][coat])/[0.07,0.10,0.13][coat])};
                if alpha<0.025 {continue;}
                let d=(spacing*0.30).max(1.) as usize;
                let (dx,dy)=if passages {passage_gradient(luma,w,h,x,y,d)} else {gradient(luma,w,h,x,y,d)};
                let energy=(dx*dx+dy*dy).sqrt();
                // A regularized tensor aligns confident edges but allows a
                // restrained, stable hand direction in featureless passages.
                let base=(site.seed-0.5)*1.9;
                let (sn,cs)=base.sin_cos();
                let a=-dy*cs+dx*sn;let b=dy*sn+dx*cs;
                let angle=if passages {passage_direction(dx,dy,base)} else {base+0.95*(2.*a*b)/(a*a+b*b+0.004)};
                let (mut sn,mut cs)=angle.sin_cos();
                let brush_scale=if coat==2 {0.76} else if coat==1 {0.87} else {1.};
                let mut length=spacing*(1.1+site.variety*1.25)*brush_scale;
                let mut width=spacing*(0.33+site.seed*0.30)*brush_scale;
                if variant==1 {length*=1.45;width*=1.15;}
                if variant==2 {length*=1.05;width*=1.35;}
                if passages && coat==0 {length*=1.18;width*=1.14;}
                if variant==8 && coat==1 {width*=1.22;}
                if variant>=10 && passages {
                    let confidence=passage_confidence(dx,dy,base);let round=(length*width).sqrt();
                    length=round+(length-round)*confidence;width=round+(width-round)*confidence;
                }
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
                let (cx,cy)=if form_scale {
                    let neg=paint_extent(&self.source,w,h,x,y,-cs,-sn,length,color,scale);
                    let pos=paint_extent(&self.source,w,h,x,y,cs,sn,length,color,scale);
                    let left=paint_extent(&self.source,w,h,x,y,sn,-cs,width,color,scale);
                    let right=paint_extent(&self.source,w,h,x,y,-sn,cs,width,color,scale);
                    let shift=(pos-neg)*0.5;let cross=(right-left)*0.5;
                    length=(neg+pos)*0.5;width=(left+right)*0.5;
                    (x as f32+cs*shift-sn*cross,y as f32+sn*shift+cs*cross)
                } else {
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
                (x as f32+cs*shift,y as f32+sn*shift)
                };
                let bend=(site.variety-0.5)*0.30;
                let drag=if variant==11 || variant==13 || variant==14 {
                    let mut loaded=color;
                    for sign in [-1.,1.] {
                        let px=(cx+cs*length*0.45*sign).clamp(0.,(w-1) as f32) as usize;
                        let py=(cy+sn*length*0.45*sign).clamp(0.,(h-1) as f32) as usize;
                        let neighbor=sample_color(&self.source,w,h,px,py,1);
                        let mix=0.275*(1.-smooth((difference(color,neighbor)-0.035)/0.15));
                        for k in 0..3 {loaded[k]+=(neighbor[k]-color[k])*mix;}
                    }
                    loaded
                } else {color};
                let stroke=Stroke {cx,cy,cs,sn,length:length.max(0.8),width:width.max(0.65),drag,
                    seed:site.seed,bend,opacity:alpha,color,variant,coat};
                if layered && coat<2 {
                    let mut small=stroke;
                    small.cx*=0.5;small.cy*=0.5;small.length*=0.5;small.width*=0.5;
                    small.draw(&mut self.underpaint,lw,lh);
                } else {stroke.draw(&mut self.paint,w,h);}
            }
            if layered && coat==1 {expand_underpainting(&self.underpaint,&mut self.paint,w,h);}
            if profile {eprintln!("OIL coat={coat} ms={:.2}",started.elapsed().as_secs_f64()*1000.);}
        }
        for (out,&paint) in frame.data.chunks_exact_mut(3).zip(&self.paint) {
            out[0]=paint as u8;out[1]=(paint>>8) as u8;out[2]=(paint>>16) as u8;
        }
    }
}

// Plan the finishing marks against an immutable underpainting. Separate light
// and dark pigment so a person and the surrounding pavement cannot average
// each other away. Weighted moments give a continuous center and brush shape.
#[derive(Default)]
struct PigmentGroup {weight:f32,error:f32,x:f32,y:f32,xx:f32,yy:f32,xy:f32,color:[f32;3]}
impl PigmentGroup {
    fn add(&mut self,x:f32,y:f32,color:[f32;3],error:f32) {
        let weight=error*error;
        self.weight+=weight;self.error+=error*weight;
        self.x+=x*weight;self.y+=y*weight;
        self.xx+=x*x*weight;self.yy+=y*y*weight;self.xy+=x*y*weight;
        for k in 0..3 {self.color[k]+=color[k]*weight;}
    }
    fn stroke(&self,site:Site,scale:f32)->Option<Stroke> {
        if self.weight<0.00001 {return None;}
        let inv=1./self.weight;
        let opacity=smooth((self.error*inv-0.045)/0.12)*smooth(self.weight/0.022);
        if opacity<0.025 {return None;}
        let x=self.x*inv;let y=self.y*inv;
        let xx=(self.xx*inv-x*x).max(0.);let yy=(self.yy*inv-y*y).max(0.);
        let xy=self.xy*inv-x*y;
        let delta=((xx-yy)*(xx-yy)+4.*xy*xy).sqrt();
        let major=((xx+yy+delta)*0.5).max(0.);let minor=((xx+yy-delta)*0.5).max(0.);
        let angle=0.5*(2.*xy).atan2(xx-yy);let (sn,cs)=angle.sin_cos();
        let length=(major.sqrt()*1.5+0.75*scale).clamp(0.8,5.8*scale.max(0.2));
        let width=(minor.sqrt()*0.85+0.7*scale).clamp(0.65,2.8*scale.max(0.25));
        Some(Stroke {cx:site.x as f32+x,cy:site.y as f32+y,cs,sn,length,width,
            seed:site.seed,bend:(site.variety-0.5)*0.12,opacity,color:self.color.map(|v|v*inv),drag:self.color.map(|v|v*inv),variant:6,coat:2})
    }
}
fn plan_accents(source:&[u8],paint:&[u32],w:usize,h:usize,site:Site,scale:f32,variant:u32,out:&mut Vec<Stroke>) {
    let d=(2.8*scale).max(1.) as isize;
    let mut groups=[PigmentGroup::default(),PigmentGroup::default()];
    for oy in [-d,0,d] {for ox in [-d,0,d] {
        let x=(site.x as isize+ox).clamp(0,(w-1) as isize) as usize;
        let y=(site.y as isize+oy).clamp(0,(h-1) as isize) as usize;
        let color=pixel(source,w,x,y);let old=unpack(paint[y*w+x]);
        let error=difference(color,old);
        let signed=(color[0]-old[0])*0.3+(color[1]-old[1])*0.5+(color[2]-old[2])*0.2;
        groups[usize::from(signed>=0.)].add(x as f32-site.x as f32,y as f32-site.y as f32,color,error);
    }}
    for group in groups {if let Some(mut stroke)=group.stroke(site,scale) {
        // Broader passages keep only confident accents. Delicate local color
        // changes remain in the underpainting rather than becoming flecks.
        if variant>=7 {stroke.opacity*=smooth((group.error/group.weight-0.025)/0.10);}
        if variant==8 {stroke.opacity*=0.84;}
        if stroke.opacity>=0.025 {out.push(stroke);}
    }}
}

fn plan_form_accents(source:&[u8],paint:&[u32],w:usize,h:usize,site:Site,scale:f32,variant:u32,out:&mut Vec<Stroke>) {
    let d=(2.8*scale).max(1.) as isize;
    let samples:[(usize,usize,[f32;3]);9]=std::array::from_fn(|i| {
        let ox=(i%3) as isize-1;let oy=(i/3) as isize-1;
        let x=(site.x as isize+ox*d).clamp(0,(w-1) as isize) as usize;
        let y=(site.y as isize+oy*d).clamp(0,(h-1) as isize) as usize;
        (x,y,pixel(source,w,x,y))
    });
    let mean: [f32;3]=std::array::from_fn(|k|samples.iter().map(|s|s.2[k]).sum::<f32>()/9.);
    let mut groups=[PigmentGroup::default(),PigmentGroup::default()];
    for (x,y,color) in samples {
        // The source supplies the feature evidence. Texture in the painting
        // alone cannot request a correction and erase the broad brushwork.
        let feature=smooth((difference(color,mean)-0.015)/0.075);
        if feature<0.01 {continue;}
        let old=unpack(paint[y*w+x]);let error=difference(color,old)*feature;
        let signed=(color[0]-mean[0])*0.3+(color[1]-mean[1])*0.5+(color[2]-mean[2])*0.2;
        groups[usize::from(signed>=0.)].add(x as f32-site.x as f32,y as f32-site.y as f32,color,error);
    }
    for group in groups {if let Some(mut stroke)=group.stroke(site,scale) {stroke.variant=variant;out.push(stroke);}}
}

#[inline] fn pack(c:[f32;3])->u32 {(c[0] as u8 as u32)|((c[1] as u8 as u32)<<8)|((c[2] as u8 as u32)<<16)}
#[inline] fn unpack(p:u32)->[f32;3] {[(p&255) as f32,((p>>8)&255) as f32,((p>>16)&255) as f32]}

#[inline]
fn mix_pigment(a:u32,b:u32,t:u32)->u32 {
    let rb=(((a&0xff00ff)*(256-t)+(b&0xff00ff)*t)>>8)&0xff00ff;
    let g=(((a&0x00ff00)*(256-t)+(b&0x00ff00)*t)>>8)&0x00ff00;
    rb|g
}
fn pigment_wash(source:&[u8],low:&mut[u32],w:usize,h:usize,tile:usize) {
    let (gw,gh)=(w.div_ceil(tile),h.div_ceil(tile));
    let mut colors=Vec::with_capacity(gw*gh);
    for y in 0..gh {for x in 0..gw {
        colors.push(pack(sample_color(source,w,h,(x*tile+tile/2).min(w-1),(y*tile+tile/2).min(h-1),tile/3).map(|v|v*0.84)));
    }}
    let axis=|p:usize,n:usize| {
        let q=((p*2) as f32-tile as f32*0.5)/tile as f32;
        let q=q.clamp(0.,(n-1) as f32);let a=q as usize;
        (a,(a+1).min(n-1),((q-a as f32)*256.) as u32)
    };
    let (lw,lh)=(w.div_ceil(2),h.div_ceil(2));
    let xs:Vec<_>=(0..lw).map(|x|axis(x,gw)).collect();
    for y in 0..lh {
        let (a,b,t)=axis(y,gh);
        for (x,&(left,right,u)) in xs.iter().enumerate() {
            low[y*lw+x]=mix_pigment(mix_pigment(colors[a*gw+left],colors[a*gw+right],u),mix_pigment(colors[b*gw+left],colors[b*gw+right],u),t);
        }
    }
}

#[inline]
fn average_pigment(a:u32,b:u32)->u32 {
    ((a&0xfefefe)>>1)+((b&0xfefefe)>>1)+(a&b&0x010101)
}
fn expand_underpainting(low:&[u32],paint:&mut[u32],w:usize,h:usize) {
    let (lw,lh)=(w.div_ceil(2),h.div_ceil(2));
    for y in 0..h {
        let sy=y/2;let next=(sy+1).min(lh-1);
        for x in 0..lw {
            let nx=(x+1).min(lw-1);
            let (mut a,mut b)=(low[sy*lw+x],low[sy*lw+nx]);
            if y&1!=0 {a=average_pigment(a,low[next*lw+x]);b=average_pigment(b,low[next*lw+nx]);}
            paint[y*w+x*2]=a;
            if x*2+1<w {paint[y*w+x*2+1]=average_pigment(a,b);}
        }
    }
}

fn paint_extent(source:&[u8],w:usize,h:usize,x:usize,y:usize,dx:f32,dy:f32,limit:f32,color:[f32;3],scale:f32)->f32 {
    let mut survival=1.;let mut extent=0.;let mut previous=0.;
    let mut distance=(1.5*scale).max(0.8).min(limit);
    for _ in 0..9 {
        let px=(x as f32+dx*distance).clamp(0.,(w-1) as f32) as usize;
        let py=(y as f32+dy*distance).clamp(0.,(h-1) as f32) as usize;
        survival*=1.-smooth((difference(color,pixel(source,w,px,py))-0.045)/0.13);
        extent+=(distance-previous)*survival;
        if distance>=limit || survival<0.01 {break;}
        previous=distance;distance=(distance*2.).min(limit);
    }
    extent.max(0.65).min(limit)
}

fn passage_direction(dx:f32,dy:f32,base:f32)->f32 {
    let (sin,cos)=(2.*base).sin_cos();
    // Blend unoriented tangent tensors, not signed gradient angles. Strong
    // vertical and horizontal edges can both reach their true direction.
    0.5*(-2.*dx*dy+sin*0.003).atan2(dy*dy-dx*dx+cos*0.003)
}
fn passage_confidence(dx:f32,dy:f32,base:f32)->f32 {
    let (sin,cos)=(2.*base).sin_cos();
    let x=dy*dy-dx*dx+cos*0.003;let y=-2.*dx*dy+sin*0.003;
    smooth((x*x+y*y).sqrt()/(dx*dx+dy*dy+0.003))
}

fn passage_gradient(luma:&[f32],w:usize,h:usize,x:usize,y:usize,d:usize)->(f32,f32) {
    let r=(d/2).max(1);let mut sum=gradient(luma,w,h,x,y,d);
    for (xx,yy) in [(x.saturating_sub(r),y),((x+r).min(w-1),y),(x,y.saturating_sub(r)),(x,(y+r).min(h-1))] {
        let g=gradient(luma,w,h,xx,yy,d);sum.0+=g.0;sum.1+=g.1;
    }
    (sum.0*0.2,sum.1*0.2)
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

struct Stroke {cx:f32,cy:f32,cs:f32,sn:f32,length:f32,width:f32,seed:f32,bend:f32,opacity:f32,color:[f32;3],drag:[f32;3],variant:u32,coat:usize}
impl Stroke {
    fn draw(&self,paint:&mut[u32],w:usize,h:usize) {
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
            let row=&mut paint[y*w+start..y*w+end+1];
            for (batch,pixels) in row.chunks_mut(4).enumerate() {self.pixels(pixels,start+batch*4,y,inverse);}
        }
    }
    #[inline]
    fn pixels(&self,pixels:&mut[u32],x:usize,y:usize,inverse:[f32;2]) {
        let ox=F::new(std::array::from_fn(|k|(x+k) as f32-self.cx));let oy=y as f32-self.cy;
        let u=(ox*self.cs+F::splat(oy*self.sn))*inverse[0];
        let v=(-ox*self.sn+F::splat(oy*self.cs))*inverse[1];
        let belly=(F::ONE-u*u).max(F::ZERO);
        let v=if self.variant>=10 {v-u*belly*self.bend} else {v-belly*self.bend};
        if self.variant>=3 && self.coat<2 {
            self.atlas_pixels(pixels,u,v);
            return;
        }
        let tip=((u.abs()-0.65)*(1./0.35)).max(F::ZERO);
        let cap=(F::ONE-tip*tip).max(F::ZERO).sqrt();
        let pressure=(F::splat(0.80)-(if self.variant>=10 {u.abs()} else {u})*0.12+belly*0.12)*cap;
        let edge=(v.abs()-pressure).max(u.abs()-1.);
        if edge.cmp_lt(F::splat(inverse[1])).none() {return;}
        // Filaments vary along the brush travel, not as screen-space noise.
        let phase=(if self.variant>=10 {v.abs()} else {v})*(7.+self.seed*8.)+(if self.variant>=10 {u.abs()} else {u})*(0.22+self.seed*0.35)+self.seed*31.;
        let groove=((phase-phase.floor())-0.5).abs()*2.;
        let dry=((if self.variant>=10 {u.abs()} else {u})+0.1).max(F::ZERO)*(F::ONE-groove)*0.22;
        let coverage=(F::splat(0.5)-edge*self.width).max(F::ZERO).min(F::ONE)*(F::splat(0.97)-dry)*self.opacity;
        let shoulder=(F::ONE-(v+pressure*0.60).abs()*7.).max(F::ZERO);
        let valley=(F::ONE-(v-pressure*0.76).abs()*8.).max(F::ZERO);
        let grain=(groove-0.5)*belly;
        let relief=if self.variant==2 {23.} else if self.variant>=6 {8.} else {16.};
        let light=if self.variant>=10 {(F::ONE-v*v).max(F::ZERO)*3.+grain*2.+(self.seed-0.5)*3.} else {shoulder*relief-valley*(relief*0.72)+grain*4.+(self.seed-0.5)*5.};
        let mut packed=U::ZERO;
        let prior=U::new(std::array::from_fn(|n|pixels.get(n).copied().unwrap_or(0)));
        for k in 0..3 {
            let bits=(prior>>(k as u32*8))&U::splat(255);
            let old=I::new(bits.to_array().map(|v|v as i32)).round_float();
            // Pigment-to-pigment mixing, with a little underpainting exposed at
            // the dry tip. No synthetic white outlines or dark polygon borders.
            let color=(old+(F::splat(self.color[k])+light-old)*coverage).max(F::ZERO).min(F::splat(255.));
            packed=packed|(U::new(color.fast_trunc_int().to_array().map(|v|v as u32))<<(k as u32*8));
        }
        pixels.copy_from_slice(&packed.to_array()[..pixels.len()]);
    }
    #[inline]
    fn atlas_pixels(&self,pixels:&mut[u32],u:F,v:F) {
        if u.abs().max(v.abs()).cmp_lt(F::ONE).none() {return;}
        let level=if self.width>=24. {0} else if self.width>=12. {1} else if self.width>=6. {2} else if self.width>=3. {3} else {4};
        let brush=(self.seed*11.99) as usize;
        let tex=sample_atlas(u,v,brush,level,self.variant>=10);
        let inside=u.abs().max(v.abs()).cmp_lt(F::ONE);
        let cover=if self.variant==11 || self.variant==13 || self.variant==14 {
            let tip=((u.abs()-0.65)*(1./0.35)).max(F::ZERO);
            let pressure=(F::splat(0.82)-u.abs()*0.10)*(F::ONE-tip*tip).max(F::ZERO).sqrt();
            let body=(F::splat(0.5)+(pressure-v.abs())*self.width).max(F::ZERO).min(F::ONE);
            inside.blend((tex[0]*(0.45/255.)+body*0.55)*(self.opacity*0.97),F::ZERO)
        } else {inside.blend(tex[0]*(self.opacity*0.97/255.),F::ZERO)};
        let du=(tex[1]-127.)*(1./126.);let dv=(tex[2]-127.)*(1./126.);
        let gray=(self.color[0]*0.299+self.color[1]*0.587+self.color[2]*0.114)/255.;
        let strength=(if self.variant==11 || self.variant==13 || self.variant==14 {62.} else if self.variant==4 || (7..=11).contains(&self.variant) {28.} else {19.})*(0.3+gray*0.7);
        let light=(du*(self.cs*0.5+self.sn*0.7)+dv*(-self.sn*0.5+self.cs*0.7))*strength;
        let load=F::ONE+(tex[3]*(1./255.)-0.35)*(if self.variant==11 || self.variant==13 || self.variant==14 {0.13} else {0.055});
        let pigment_mix=F::splat(0.18)+(F::ONE-u*u).max(F::ZERO)*0.64;
        let mut packed=U::ZERO;
        let prior=U::new(std::array::from_fn(|n|pixels.get(n).copied().unwrap_or(0)));
        for k in 0..3 {
            let bits=(prior>>(k as u32*8))&U::splat(255);
            let old=I::new(bits.to_array().map(|v|v as i32)).round_float();
            let pigment=if self.variant==11 || self.variant==13 || self.variant==14 {F::splat(self.color[k])+pigment_mix*(self.drag[k]-self.color[k])} else {F::splat(self.color[k])};
            let color=(old+(load*pigment+light-old)*cover).max(F::ZERO).min(F::splat(255.));
            packed=packed|(U::new(color.fast_trunc_int().to_array().map(|v|v as u32))<<(k as u32*8));
        }
        pixels.copy_from_slice(&packed.to_array()[..pixels.len()]);
    }

}

// Twelve pressure/fiber deposits, each with five filtered mip levels. This
// compact immutable atlas is shared by every worker and adds no frame history.
const BRUSH_ATLAS:&[u8;523776]=include_bytes!("../../assets/oil_brushes.rgba");
const NEUTRAL_BRUSH_ATLAS:&[u8;523776]=include_bytes!("../../assets/oil_brushes_neutral.rgba");
fn sample_atlas(u:F,v:F,brush:usize,level:usize,neutral:bool)->[F;4] {
    let atlas=if neutral {NEUTRAL_BRUSH_ATLAS} else {BRUSH_ATLAS};
    let width=128usize>>level;let height=64usize>>level;
    let offset=brush*43648+[0,32768,40960,43008,43520][level];
    let x=((u+1.)*((width-1) as f32*0.5)).max(F::ZERO).min(F::splat((width-1) as f32));
    let y=((v+1.)*((height-1) as f32*0.5)).max(F::ZERO).min(F::splat((height-1) as f32));
    let fx=x-x.floor();let fy=y-y.floor();
    let xx=x.fast_trunc_int().to_array().map(|v|v as usize);
    let yy=y.fast_trunc_int().to_array().map(|v|v as usize);
    let indices=std::array::from_fn::<_,4,_>(|i| {
        let x1=(xx[i]+1).min(width-1);let y1=(yy[i]+1).min(height-1);
        [offset+(yy[i]*width+xx[i])*4,offset+(yy[i]*width+x1)*4,
         offset+(y1*width+xx[i])*4,offset+(y1*width+x1)*4]
    });
    // Load each complete texel once, then unpack its four material channels in
    // registers. The scalar byte sampler remains in the pixel-exact test oracle.
    let words: [U;4]=std::array::from_fn(|corner|U::new(indices.map(|p| {
        u32::from_le_bytes(atlas[p[corner]..p[corner]+4].try_into().unwrap())
    })));
    std::array::from_fn(|channel| {
        let values: [F;4]=words.map(|word| {
            let bytes=(word>>(channel as u32*8))&U::splat(255);
            I::new(bytes.to_array().map(|v|v as i32)).round_float()
        });
        let [a,b,c,d]=values;
        let top=a+(b-a)*fx;let bottom=c+(d-c)*fx;
        top+(bottom-top)*fy
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn wash_interpolation_keeps_channels_separate() {
        for i in 0..1024 {
            let a=hash(i,91)&0xffffff;let b=hash(i,777)&0xffffff;let t=(i%257) as u32;
            let expected=pack(std::array::from_fn(|k|((((a>>(k*8))&255)*(256-t)+((b>>(k*8))&255)*t)/256) as f32));
            assert_eq!(mix_pigment(a,b,t),expected);
        }
        let source=RawFrame::filled(31,19,100,100,100);let mut low=vec![0;16*10];
        pigment_wash(&source.data,&mut low,31,19,14);assert!(low.iter().all(|&p|p==pack([84.,84.,84.])));
    }
    #[test] fn broad_brush_stops_at_a_narrow_subject_in_both_axes() {
        let mut source=RawFrame::filled(128,96,145,145,145);
        for y in 38..59 {for x in 62..67 {source.data[(y*128+x)*3..(y*128+x)*3+3].copy_from_slice(&[30,35,40]);}}
        let c=[30.,35.,40.];
        let width=paint_extent(&source.data,128,96,64,48,1.,0.,25.,c,1.);
        let length=paint_extent(&source.data,128,96,64,48,0.,1.,60.,c,1.);
        assert!(width<=3. && length<=12. && length>width*2.,"{width} {length}");
        let open=paint_extent(&source.data,128,96,15,15,1.,0.,30.,[145.,145.,145.],1.);
        assert_eq!(open,30.);
    }
    #[test] fn neutral_paint_is_invariant_to_tangent_reversal() {
        for coat in [0,2] {for angle in [-1.55_f32,-0.7,0.,0.9,1.55] {
            let (sn,cs)=angle.sin_cos();let mut stroke=Stroke{cx:40.3,cy:30.7,cs,sn,length:20.,width:5.,seed:0.37,bend:0.13,opacity:0.9,color:[130.,75.,45.],drag:[130.,75.,45.],variant:10,coat};
            let mut a=vec![pack([70.,90.,110.]);80*60];let mut b=a.clone();
            stroke.draw(&mut a,80,60);stroke.cs=-cs;stroke.sn=-sn;stroke.draw(&mut b,80,60);
            let max=a.iter().zip(b).flat_map(|(&a,b)|unpack(a).into_iter().zip(unpack(b)).map(|(x,y)|(x-y).abs())).fold(0_f32,f32::max);
            assert!(max<=1.,"coat={coat} angle={angle} max={max}");
        }}
        assert!(passage_confidence(0.003_f32.sqrt(),0.,0.)<0.001);
    }
    #[test] fn paint_texture_alone_does_not_create_finishing_marks() {
        let source=RawFrame::filled(32,32,120,120,120);
        let paint:Vec<_>=(0..1024).map(|i|if i%2==0 {pack([75.,75.,75.])} else {pack([155.,155.,155.])}).collect();
        let mut strokes=Vec::new();
        plan_form_accents(&source.data,&paint,32,32,Site{x:16,y:16,seed:0.4,variety:0.6},1.,9,&mut strokes);
        assert!(strokes.is_empty());
        let mut source=source;
        for y in 12..21 {source.data[(y*32+16)*3..(y*32+16)*3+3].copy_from_slice(&[20,25,30]);}
        plan_form_accents(&source.data,&paint,32,32,Site{x:16,y:16,seed:0.4,variety:0.6},1.,9,&mut strokes);
        assert!(!strokes.is_empty());assert!(strokes.iter().any(|s|s.color[0]<40.));
    }
    #[test] fn passage_orientation_follows_both_edge_axes_without_polarity_flips() {
        let vertical=passage_direction(0.5,0.,0.2);
        let horizontal=passage_direction(0.,0.5,0.2);
        assert!(vertical.sin().abs()>0.99 && horizontal.cos().abs()>0.99);
        assert_eq!(vertical,passage_direction(-0.5,0.,0.2));
        assert!((passage_direction(0.,0.,0.2)-0.2).abs()<0.00001);
    }
    #[test] fn pigment_moments_describe_upright_and_compact_marks() {
        let site=Site{x:20,y:20,seed:0.4,variety:0.6};
        let mut upright=PigmentGroup::default();
        for y in [-3.,0.,3.] {upright.add(0.,y,[30.,40.,50.],0.5);}
        let stroke=upright.stroke(site,1.).unwrap();
        assert!(stroke.sn.abs()>0.99 && stroke.length>stroke.width*3.);
        assert!((stroke.cx-20.).abs()<0.001 && (stroke.cy-20.).abs()<0.001);
        let mut dot=PigmentGroup::default();dot.add(1.,2.,[220.,70.,30.],0.5);
        let stroke=dot.stroke(site,1.).unwrap();
        assert!(stroke.length<1. && stroke.width<1.);
        assert_eq!(stroke.color,[220.,70.,30.]);
    }
    #[test] fn packed_pigment_average_has_no_channel_carry() {
        for i in 0..1024 {
            let a=hash(i,73)&0xffffff;let b=hash(i,919)&0xffffff;
            let expected=pack(std::array::from_fn(|k|((((a>>(k*8))&255)+((b>>(k*8))&255))/2) as f32));
            assert_eq!(average_pigment(a,b),expected);
        }
    }
    #[test] fn underpainting_expansion_keeps_odd_edges_and_constant_color() {
        for (w,h) in [(1usize,1usize),(31,19),(64,36)] {
            let color=pack([130.,75.,210.]);let low=vec![color;w.div_ceil(2)*h.div_ceil(2)];let mut out=vec![0;w*h];
            expand_underpainting(&low,&mut out,w,h);assert!(out.iter().all(|&p|p==color));
        }
    }
    #[test] fn packed_surface_preserves_rgb_reference() {
        for (w,h) in [(96,54),(192,108)] {
            let mut source=RawFrame::new(w,h);
            for (i,p) in source.data.iter_mut().enumerate() {*p=((i*71+i/17)%256) as u8;}
            let luma:Vec<_>=source.data.chunks_exact(3).map(|p|(p[0] as f32*0.299+p[1] as f32*0.587+p[2] as f32*0.114)/255.).collect();
            for variant in 0..=4 {
                let mut actual=source.clone();let mut expected=source.clone();
                OilPaint::default().draw(&mut actual,&luma,[14.,0.3,0.],variant);
                super::super::oil_reference::OilPaint::default().draw(&mut expected,&luma,[14.,0.3,0.],variant);
                assert_eq!(actual.data,expected.data,"{w}x{h} variant={variant}");
            }
        }
    }
    #[test] fn clipped_strokes_match_full_rectangle_rasterization() {
        for angle in [-1.55_f32,-0.8,-0.05,0.,0.3,0.9,1.55] {
            for (length,width) in [(7.,1.),(23.,4.),(58.,17.)] {
                let (sn,cs)=angle.sin_cos();
                let stroke=Stroke {cx:43.3,cy:33.7,cs,sn,length,width,seed:0.73,bend:0.14,opacity:0.8,color:[160.,70.,110.],drag:[160.,70.,110.],variant:0,coat:0};
                let mut clipped=vec![pack([50.,60.,70.]);96*72];let mut brute=clipped.clone();
                stroke.draw(&mut clipped,96,72);
                let reach=width*(1.15+stroke.bend.abs());
                let bx=cs.abs()*length+sn.abs()*reach;let by=sn.abs()*length+cs.abs()*reach;
                let minx=(stroke.cx-bx-1.).max(0.) as usize;let maxx=(stroke.cx+bx+1.).min(95.) as usize;
                let miny=(stroke.cy-by-1.).max(0.) as usize;let maxy=(stroke.cy+by+1.).min(71.) as usize;
                for y in miny..=maxy {
                    for x in minx..=maxx {stroke.pixels(&mut brute[y*96+x..y*96+x+1],x,y,[1./length,1./width]);}
                }
                assert_eq!(clipped,brute,"angle={angle} length={length} width={width}");
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
            for variant in 0..=14 {painter.draw(&mut f,&luma,[14.,0.3,0.],variant);assert_eq!(f.data.len(),(w*h*3) as usize);}
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
        for variant in 0..=14 {
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
