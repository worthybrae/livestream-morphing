//! Shared native brush raster for recorded studies and the live worker.
//! Input per frame: little-endian u32 count, then 12 f32 values per mark.
use std::hash::{BuildHasherDefault, Hasher};
// Brush identities are trusted, internally generated integers. A small integer
// mixer avoids repeating a keyed string hash for every mark and canvas band.
#[derive(Default)]
struct BrushHasher(u64);
impl Hasher for BrushHasher {
    fn finish(&self)->u64 { self.0 }
    fn write_u32(&mut self,value:u32) {
        let mut x=value as u64;
        x=(x^(x>>16)).wrapping_mul(0x45d9f3b);
        x=(x^(x>>16)).wrapping_mul(0x45d9f3b);
        self.0=x^(x>>16);
    }
    fn write(&mut self,bytes:&[u8]) {
        for &b in bytes { self.0=(self.0^b as u64).wrapping_mul(0x100000001b3); }
    }
}
type HashMap<K,V> = std::collections::HashMap<K,V,BuildHasherDefault<BrushHasher>>;
type HashSet<K> = std::collections::HashSet<K,BuildHasherDefault<BrushHasher>>;
mod material;
mod tiles;
pub use tiles::{TilePainter,TileResult};

struct Stamp { w:usize, h:usize, ox:i32, oy:i32, pixels:Vec<[i16;2]>, spans:Vec<(usize,usize)>, prepared:Vec<[i16;2]>, key:Option<[i32;3]>, pending:[i32;3] }
fn make_stamp(p:&[f32])->Stamp {
    let (length,width,angle,seed)=(p[3],p[4],p[5],p[6]);
    let (sn,cs)=angle.sin_cos();
    let bx=(cs.abs()*length+sn.abs()*width*1.4).ceil() as i32+2;
    let by=(sn.abs()*length+cs.abs()*width*1.4).ceil() as i32+2;
    let (w,h)=((bx*2+1) as usize,(by*2+1) as usize);
    let mut pixels=vec![[0,0];w*h];
    for y in -by..=by {for x in -bx..=bx {
        let u=(x as f32*cs+y as f32*sn)/length;
        let v=(-x as f32*sn+y as f32*cs)/width;
        if u.abs()>=1. {continue;}
        let belly=1.-u*u;
        let across=v-belly*(seed-0.5)*0.45;
        let cap=(1.-((u.abs()-0.50).max(0.)/0.5).powi(2)).max(0.).sqrt();
        let pressure=(0.78-u*0.13+belly*0.10)*cap;
        let bristle=((across*(7.+seed*9.)+seed*31.).fract().abs()-0.5).abs()*2.;
        let edge=(across*31.+u*11.+seed*99.).sin()*0.026;
        let cover=((pressure-across.abs()+edge)*width).clamp(0.,1.);
        let dry=(u+0.2).max(0.)*(1.-bristle)*0.23;
        let alpha=(cover*(0.98-dry)*255.).round() as i16;
        let light=((bristle-0.5)*5.*belly + (seed-0.5)*3.
            + ((across/pressure.max(0.05)).clamp(-1.,1.))*2.).round() as i16;
        pixels[(y+by) as usize*w+(x+bx) as usize]=[alpha,alpha*light];
    }}
    Stamp::new(w,h,-bx,-by,pixels)
}
impl Stamp {
    fn new(w:usize,h:usize,ox:i32,oy:i32,pixels:Vec<[i16;2]>)->Self {
        // A rotated thin brush has a mostly empty bounding rectangle. Include
        // both source rows and the extra column reached by subpixel sampling,
        // then never interpolate or blend outside that exact support.
        let rows:Vec<_>=pixels.chunks_exact(w).map(|row| {
            (row.iter().position(|p|p[0]!=0||p[1]!=0),row.iter().rposition(|p|p[0]!=0||p[1]!=0))
        }).collect();
        let mut spans=vec![(0,0);h];
        for y in 1..h {
            let first=[rows[y-1].0,rows[y].0].into_iter().flatten().min();
            let last=[rows[y-1].1,rows[y].1].into_iter().flatten().max();
            if let (Some(a),Some(b))=(first,last) {spans[y]=(a.max(1),(b+2).min(w));}
        }
        Self {w,h,ox,oy,pixels,spans,prepared:Vec::new(),key:None,pending:[0;3]}
    }
    fn prepare(&mut self,p:&[f32]) {
        self.set_phase(p);self.prepare_phase();
    }
    fn set_phase(&mut self,p:&[f32]) {
        let fx=((p[1]-p[1].floor())*256.) as i32;
        let fy=((p[2]-p[2].floor())*256.) as i32;
        let opacity=(p[10].clamp(0.,1.)*256.) as i32;
        self.pending=[fx,fy,opacity];
    }
    fn prepare_phase(&mut self) {
        let key=self.pending;let [fx,fy,opacity]=key;
        if self.key==Some(key) {return;}
        self.key=Some(key);
        self.prepared.resize(self.pixels.len(),[0,0]);
        let weights=[(256-fx)*(256-fy),fx*(256-fy),(256-fx)*fy,fx*fy];
        for sy in 1..self.h {
            let (start,end)=self.spans[sy];
            if start==end {continue;}
            let at=sy*self.w+start;let stop=sy*self.w+end;
            // Prove the bounds once per row so LLVM can vectorize adjacent
            // samples; preserve the integer interpolation and rounding exactly.
            let current=&self.pixels[at..stop];
            let left=&self.pixels[at-1..stop-1];
            let above=&self.pixels[at-self.w..stop-self.w];
            let diagonal=&self.pixels[at-self.w-1..stop-self.w-1];
            for ((((out,a),b),c),d) in self.prepared[at..stop].iter_mut()
                .zip(current).zip(left).zip(above).zip(diagonal) {
                for k in 0..2 {
                    let value=a[k] as i32*weights[0]+b[k] as i32*weights[1]
                        +c[k] as i32*weights[2]+d[k] as i32*weights[3];
                    out[k]=(((value>>16)*opacity)>>8) as i16;
                }
            }
        }
    }
}
#[cfg(test)]
fn draw_reference(canvas:&mut [u8], w:usize,h:usize,top:usize,p:&[f32],stamp:&Stamp) {
    let (cx,cy)=(p[1].floor() as i32,p[2].floor() as i32);
    let (fx,fy)=(((p[1]-p[1].floor())*256.) as i32,((p[2]-p[2].floor())*256.) as i32);
    let weights=[(256-fx)*(256-fy),fx*(256-fy),(256-fx)*fy,fx*fy];
    let opacity=(p[10].clamp(0.,1.)*256.) as i32;
    if opacity==0 {return;}
    let color=[p[7].clamp(0.,255.) as i32,p[8].clamp(0.,255.) as i32,p[9].clamp(0.,255.) as i32];
    for sy in 1..stamp.h {let y=cy+stamp.oy+sy as i32;
        if y<top as i32 || y>=(top+h) as i32 {continue;}
        for sx in 1..stamp.w {let x=cx+stamp.ox+sx as i32;
            if x<0 || x>=w as i32 {continue;}
            let index=sy*stamp.w+sx;
            let ids=[index,index-1,index-stamp.w,index-stamp.w-1];
            let mut alpha=0;let mut shade=0;
            for k in 0..4 {alpha+=stamp.pixels[ids[k]][0] as i32*weights[k];shade+=stamp.pixels[ids[k]][1] as i32*weights[k];}
            let alpha=((alpha>>16)*opacity)>>8;
            if alpha==0 {continue;}
            let shade=((shade>>16)*opacity)>>8;
            let at=((y as usize-top)*w+x as usize)*3;
            for k in 0..3 {canvas[at+k]=((canvas[at+k] as i32*(255-alpha)+color[k]*alpha+shade+127)/255).clamp(0,255) as u8;}
        }
    }
}
fn draw(canvas:&mut [u8],w:usize,h:usize,top:usize,p:&[f32],stamp:&Stamp) {
    let cx=p[1].floor() as i32+stamp.ox;
    let cy=p[2].floor() as i32+stamp.oy;
    let y0=(top as i32-cy).max(1) as usize;
    let y1=((top+h) as i32-cy).clamp(0,stamp.h as i32) as usize;
    let x0=(-cx).max(1) as usize;
    let x1=(w as i32-cx).clamp(0,stamp.w as i32) as usize;
    if x0>=x1 || y0>=y1 {return;}
    let color=[p[7].clamp(0.,255.) as i32,p[8].clamp(0.,255.) as i32,p[9].clamp(0.,255.) as i32,255];
    for sy in y0..y1 {
        let row=((cy+sy as i32) as usize-top)*w;
        let start=x0.max(stamp.spans[sy].0);let end=x1.min(stamp.spans[sy].1);
        if start>=end {continue;}
        let at=(row+(cx+start as i32) as usize)*4;
        let dest=&mut canvas[at..at+(end-start)*4];
        let coverage=&stamp.prepared[sy*stamp.w+start..sy*stamp.w+end];
        blend(dest,coverage,color);
    }
}
pub(crate) fn blend(dest:&mut [u8],coverage:&[[i16;2]],color:[i32;4]) {
    #[cfg(target_arch="x86_64")]
    if coverage.len()>=8 && std::arch::is_x86_feature_detected!("avx2") {
        unsafe {blend_avx2(dest,coverage,color);}return;
    }
    #[cfg(target_arch="aarch64")]
    if coverage.len()>=2 && std::arch::is_aarch64_feature_detected!("neon") {
        unsafe {blend_neon(dest,coverage,color);}return;
    }
    blend_scalar(dest,coverage,color);
}
pub(crate) fn blend_scalar(dest:&mut [u8],coverage:&[[i16;2]],color:[i32;4]) {
        for (pixel,&[a,b]) in dest.chunks_exact_mut(4).zip(coverage) {
            let alpha=a as i32;let shade=b as i32;
            if alpha==0 {continue;}
            for k in 0..4 {
                let sum=(pixel[k] as i32*(255-alpha)+color[k]*alpha+shade+127).clamp(0,65025);
                pixel[k]=((sum+1+(sum>>8))>>8) as u8;
            }
        }
}
#[cfg(target_arch="aarch64")]
#[target_feature(enable="neon")]
unsafe fn blend_neon(dest:&mut [u8],coverage:&[[i16;2]],color:[i32;4]) {
    use std::arch::aarch64::*;
    let pigment=vld1q_s32(color.as_ptr());let zero=vdupq_n_s32(0);let maximum=vdupq_n_s32(65025);
    let one=vdupq_n_s32(1);let pairs=coverage.len()/2;
    for n in 0..pairs {
        let [a,b]=coverage[n*2];let [c,d]=coverage[n*2+1];
        let b=if a==0 {0}else{b} as i32;let d=if c==0 {0}else{d} as i32;
        let ptr=dest.as_mut_ptr().add(n*8);let old=vmovl_u8(vld1_u8(ptr));
        let old0=vreinterpretq_s32_u32(vmovl_u16(vget_low_u16(old)));
        let old1=vreinterpretq_s32_u32(vmovl_u16(vget_high_u16(old)));
        let apply=|old,a:i32,shade:i32| {
            let base=vmulq_n_s32(old,255-a);
            let sum=vminq_s32(maximum,vmaxq_s32(zero,vaddq_s32(base,
                vaddq_s32(vmulq_n_s32(pigment,a),vdupq_n_s32(shade+127)))));
            vshrq_n_s32::<8>(vaddq_s32(vaddq_s32(sum,one),vshrq_n_s32::<8>(sum)))
        };
        let packed=vqmovn_u16(vcombine_u16(vqmovun_s32(apply(old0,a as i32,b)),
            vqmovun_s32(apply(old1,c as i32,d))));
        vst1_u8(ptr,packed);
    }
    blend_scalar(&mut dest[pairs*8..],&coverage[pairs*2..],color);
}
#[cfg(target_arch="x86_64")]
#[target_feature(enable="avx2")]
unsafe fn blend_avx2(dest:&mut [u8],coverage:&[[i16;2]],color:[i32;4]) {
    use std::arch::x86_64::*;
    let pigment_color=_mm256_broadcastsi128_si256(_mm_loadu_si128(color.as_ptr().cast()));
    let zero=_mm256_setzero_si256();let max=_mm256_set1_epi32(65025);
    let full=_mm256_set1_epi32(255);let round=_mm256_set1_epi32(127);let one=_mm256_set1_epi32(1);
    let pairs=coverage.len()/2;
    for n in 0..pairs {
        let [a,b]=coverage[n*2];let [c,d]=coverage[n*2+1];
        if a==0 && c==0 {continue;}
        let b=if a==0 {0} else {b} as i32;let d=if c==0 {0} else {d} as i32;
        let (a,c)=(a as i32,c as i32);
        let alpha=_mm256_setr_epi32(a,a,a,a,c,c,c,c);
        let shade=_mm256_setr_epi32(b,b,b,b,d,d,d,d);
        let ptr=dest.as_mut_ptr().add(n*8);
        let old=_mm256_cvtepu8_epi32(_mm_loadl_epi64(ptr.cast()));
        let base=_mm256_mullo_epi32(old,_mm256_sub_epi32(full,alpha));
        let pigment=_mm256_add_epi32(_mm256_mullo_epi32(pigment_color,alpha),shade);
        let sum=_mm256_min_epi32(max,_mm256_max_epi32(zero,_mm256_add_epi32(_mm256_add_epi32(base,pigment),round)));
        let result=_mm256_srli_epi32::<8>(_mm256_add_epi32(_mm256_add_epi32(sum,one),_mm256_srli_epi32::<8>(sum)));
        let packed=_mm_packus_epi32(_mm256_castsi256_si128(result),_mm256_extracti128_si256::<1>(result));
        _mm_storel_epi64(ptr.cast(),_mm_packus_epi16(packed,_mm_setzero_si128()));
    }
    blend_scalar(&mut dest[pairs*8..],&coverage[pairs*2..],color);
}
fn raster(canvas:&mut [u8],w:usize,h:usize,values:&[f32],cache:&HashMap<u32,Stamp>,threads:usize,clear:bool) {
    let rows=h.div_ceil(threads);
    // Resolve each identity once, in deposition order, rather than looking it
    // up independently in every worker's image band.
    let commands:Vec<_>=values.chunks_exact(12).filter_map(|p| {
        cache.get(&(p[0] as u32)).map(|stamp|(p,stamp))
    }).collect();
    std::thread::scope(|scope| {
        for (band,dst) in canvas.chunks_mut(rows*w*4).enumerate() {
            let commands=&commands;
            scope.spawn(move|| {
                let top=band*rows;let bh=dst.len()/(w*4);
                if clear {for pixel in dst.chunks_exact_mut(4) {pixel.copy_from_slice(&[81,83,74,255]);}}
                for &(p,stamp) in commands {
                    let sy=p[2].floor() as i32+stamp.oy;
                    if sy<=(top+bh) as i32 && sy+stamp.h as i32>=top as i32 {draw(dst,w,bh,top,p,stamp);}
                }
            });
        }
    });
}
pub struct Painter {
    w:usize,h:usize,canvas:Vec<u8>,rgb:Vec<u8>,cache:HashMap<u32,Stamp>,material:material::Material,
    incremental:TilePainter,
}
impl Painter {
    pub fn new(w:usize,h:usize,style:&str)->Option<Self> {
        if w==0 || h==0 || w>8192 || h>8192 || w*h>16_777_216 {return None;}
        Some(Self {w,h,canvas:vec![0;w*h*4],rgb:vec![0;w*h*3],cache:HashMap::default(),
            material:material::Material::parse(style)?,incremental:TilePainter::new(w,h,style)?})
    }
    pub fn paint(&mut self,values:&[f32],append:bool)->Option<&[u8]> {
        if values.len()%12!=0 || values.len()>100_000*12 {return None;}
        if values.chunks_exact(12).any(|p|!p.iter().all(|v|v.is_finite()) || p[3]<=0. || p[4]<=0. || p[3]>512. || p[4]>512.) {return None;}
        let mut active=HashSet::with_capacity_and_hasher(values.len()/12,Default::default());
        let mut missing=Vec::new();
        for p in values.chunks_exact(12) {
            let id=p[0] as u32;
            if !active.insert(id) {return None;}
            if let Some(stamp)=self.cache.get_mut(&id) {stamp.set_phase(p);}
            else {missing.push(p);}
        }
        if !append {self.cache.retain(|id,_|active.contains(id));}
        // Mask creation and resampling are independent for each brush. Preserve
        // deposition order, but prepare its immutable coverage in parallel.
        let material=self.material;
        let next=std::sync::atomic::AtomicUsize::new(0);
        let made=std::thread::scope(|scope| {
            let mut jobs=Vec::new();
            // Input is ordered by coat: a count-based split gives one worker
            // most of the large, expensive masks. Share individual new marks
            // instead; deposition order is still defined by the input packet.
            for _ in 0..missing.len().min(8) {
                let next=&next;let missing=&missing;
                jobs.push(scope.spawn(move|| {
                    let mut made=Vec::new();
                    while let Some(p)=missing.get(next.fetch_add(1,std::sync::atomic::Ordering::Relaxed)) {
                        let mut stamp=material::stamp(p,material);stamp.prepare(p);
                        made.push((p[0] as u32,stamp));
                    }
                    made
                }));
            }
            jobs.into_iter().flat_map(|j|j.join().expect("brush mask preparation")).collect::<Vec<_>>()
        });
        self.cache.extend(made);
        let mut changed:Vec<_>=self.cache.values_mut().filter(|s|s.key!=Some(s.pending)).collect();
        let size=changed.len().div_ceil(8).max(1);
        std::thread::scope(|scope| {for chunk in changed.chunks_mut(size) {
            scope.spawn(move|| {for stamp in chunk {stamp.prepare_phase();}});
        }});
        raster(&mut self.canvas,self.w,self.h,values,&self.cache,8,!append);
        for (dst,src) in self.rgb.chunks_exact_mut(3).zip(self.canvas.chunks_exact(4)) {dst.copy_from_slice(&src[..3]);}
        Some(&self.rgb)
    }
}

// Private local-process ABI. The caller supplies a valid contiguous f32 array
// of count*12 elements and does not retain canvas views after free/repaint.
#[no_mangle]
pub extern "C" fn paint_new(w:usize,h:usize,loaded:bool)->*mut Painter {
    Painter::new(w,h,if loaded {"loaded"} else {"plain"}).map(|p|Box::into_raw(Box::new(p))).unwrap_or(std::ptr::null_mut())
}
#[no_mangle]
pub unsafe extern "C" fn paint_frame(p:*mut Painter,marks:*const f32,count:usize,append:bool)->*const u8 {
    if p.is_null() || count>100_000 || (count>0 && marks.is_null()) {return std::ptr::null();}
    let values=if count==0 {&[]} else {std::slice::from_raw_parts(marks,count*12)};
    (*p).paint(values,append).map(|pixels|pixels.as_ptr()).unwrap_or(std::ptr::null())
}
#[repr(C)]
pub struct PaintFrameStats {
    pub mutated_pixels:u64,pub dirty_micros:u64,pub full_raster_micros:u64,pub full_fallback:u8,
}
#[no_mangle]
pub unsafe extern "C" fn paint_frame_incremental(p:*mut Painter,marks:*const f32,count:usize,stats:*mut PaintFrameStats)->*const u8 {
    if p.is_null() || stats.is_null() || count>100_000 || (count>0 && marks.is_null()) {return std::ptr::null();}
    let values=if count==0 {&[]} else {std::slice::from_raw_parts(marks,count*12)};
    let Some(result)=(*p).incremental.paint(values) else {return std::ptr::null();};
    *stats=PaintFrameStats {mutated_pixels:result.mutated_pixels,dirty_micros:result.dirty_micros,
        full_raster_micros:result.full_raster_micros,full_fallback:result.full_fallback as u8};
    result.rgb.as_ptr()
}
#[no_mangle]
pub unsafe extern "C" fn paint_clear(p:*mut Painter) {if !p.is_null() {(*p).cache.clear();(*p).incremental.clear();}}
#[no_mangle]
pub unsafe extern "C" fn paint_set_incremental_workers(p:*mut Painter,workers:usize) {
    if !p.is_null() {(*p).incremental.set_workers(workers);}
}
#[no_mangle]
pub unsafe extern "C" fn paint_free(p:*mut Painter) {if !p.is_null() {drop(Box::from_raw(p));}}

fn rgb_max_error_scalar(source:&[f32],canvas:&[f32],output:&mut [f32]) {
    for ((source,canvas),out) in source.chunks_exact(3).zip(canvas.chunks_exact(3)).zip(output) {
        *out=(source[0]-canvas[0]).abs().max((source[1]-canvas[1]).abs()).max((source[2]-canvas[2]).abs());
    }
}
#[cfg(target_arch="aarch64")]
#[target_feature(enable="neon")]
unsafe fn rgb_max_error_neon(source:&[f32],canvas:&[f32],output:&mut [f32]) {
    use std::arch::aarch64::*;
    let groups=output.len()/4;
    for group in 0..groups {
        let offset=group*12;
        let a=vld3q_f32(source.as_ptr().add(offset));let b=vld3q_f32(canvas.as_ptr().add(offset));
        let maximum=vmaxq_f32(vabdq_f32(a.0,b.0),vmaxq_f32(vabdq_f32(a.1,b.1),vabdq_f32(a.2,b.2)));
        vst1q_f32(output.as_mut_ptr().add(group*4),maximum);
    }
    rgb_max_error_scalar(&source[groups*12..],&canvas[groups*12..],&mut output[groups*4..]);
}
#[cfg(target_arch="x86_64")]
#[target_feature(enable="avx2")]
unsafe fn rgb_max_error_avx2(source:&[f32],canvas:&[f32],output:&mut [f32]) {
    use std::arch::x86_64::*;
    let indices=[_mm_setr_epi32(0,3,6,9),_mm_setr_epi32(1,4,7,10),_mm_setr_epi32(2,5,8,11)];
    let sign=_mm_set1_ps(-0.0);let groups=output.len()/4;
    for group in 0..groups {
        let offset=group*12;let a=source.as_ptr().add(offset);let b=canvas.as_ptr().add(offset);
        let difference=|index|_mm_andnot_ps(sign,_mm_sub_ps(_mm_i32gather_ps(a,indices[index],4),_mm_i32gather_ps(b,indices[index],4)));
        _mm_storeu_ps(output.as_mut_ptr().add(group*4),_mm_max_ps(difference(0),_mm_max_ps(difference(1),difference(2))));
    }
    rgb_max_error_scalar(&source[groups*12..],&canvas[groups*12..],&mut output[groups*4..]);
}
#[no_mangle]
pub unsafe extern "C" fn paint_rgb_max_error(source:*const f32,canvas:*const f32,output:*mut f32,count:usize) {
    if source.is_null() || canvas.is_null() || output.is_null() || count>16_777_216 {return;}
    let source=std::slice::from_raw_parts(source,count*3);let canvas=std::slice::from_raw_parts(canvas,count*3);
    let output=std::slice::from_raw_parts_mut(output,count);
    #[cfg(target_arch="aarch64")]
    if std::arch::is_aarch64_feature_detected!("neon") {rgb_max_error_neon(source,canvas,output);return;}
    #[cfg(target_arch="x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") {rgb_max_error_avx2(source,canvas,output);return;}
    rgb_max_error_scalar(source,canvas,output);
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn incremental_abi_reports_changes_and_rejects_invalid_packets() {
        let mut painter=Painter::new(79,61,"loaded").unwrap();
        let mark=[1.,25.25,22.75,18.,7.,0.31,0.17,160.,80.,190.,0.83,1.];
        let mut stats=PaintFrameStats {mutated_pixels:0,dirty_micros:0,full_raster_micros:0,full_fallback:0};
        let first=unsafe {paint_frame_incremental(&mut painter,mark.as_ptr(),1,&mut stats)};
        assert!(!first.is_null());assert_eq!(stats.full_fallback,1);assert!(stats.mutated_pixels>0);
        let second=unsafe {paint_frame_incremental(&mut painter,mark.as_ptr(),1,&mut stats)};
        assert!(!second.is_null());assert_eq!(stats.mutated_pixels,0);
        let duplicate=[mark,mark].concat();
        assert!(unsafe {paint_frame_incremental(&mut painter,duplicate.as_ptr(),2,&mut stats)}.is_null());
        assert!(unsafe {paint_frame_incremental(&mut painter,mark.as_ptr(),1,std::ptr::null_mut())}.is_null());
    }
    #[test] fn incremental_worker_selection_is_bounded() {
        let mut painter=Painter::new(79,61,"loaded").unwrap();
        unsafe {paint_set_incremental_workers(&mut painter,0)};
        assert_eq!(painter.incremental.worker_count_for_test(),1);
        unsafe {paint_set_incremental_workers(&mut painter,99)};
        assert_eq!(painter.incremental.worker_count_for_test(),8);
    }
    #[cfg(target_arch="x86_64")]
    #[test] fn vector_color_grade_preserves_day_night_and_rgb_tails() {
        if !std::arch::is_x86_feature_detected!("avx2") {return;}
        for count in [0,1,2,3,4,5,6,7,8,9,65539] {
            let input:Vec<u8>=(0..count*3).map(|i| {
                ((i as u32).wrapping_mul(1664525).wrapping_add(1013904223)>>16) as u8
            }).collect();
            for night in [0.,0.13,0.5,0.95,1.] {
                for color in [[-13.,-5.,27.],[32.,3.,12.],[37.,1.,-16.]] {
                    let mut expected=vec![0;input.len()];let mut actual=expected.clone();
                    grade_scalar(&input,&mut expected,night,&color);
                    unsafe {grade_avx2(&input,&mut actual,night,&color);}
                    let mismatch=expected.iter().zip(&actual).position(|(a,b)|a!=b);
                    assert_eq!(mismatch,None,"count={count}, night={night}");
                }
            }
        }
    }
    #[test] fn piles_lookup_matches_its_index_on_small_and_split_frames() {
        let table:Vec<u8>=(0..32768*3).map(|i|((i as u32).wrapping_mul(2654435761)>>24) as u8).collect();
        for count in [0usize,1,2,7,65539,1_000_003] {
            let input:Vec<u8>=(0..count*3).map(|i| {
                ((i as u32).wrapping_mul(1664525).wrapping_add(1013904223)>>16) as u8
            }).collect();
            let mut actual=vec![0u8;input.len()];
            unsafe {paint_piles(input.as_ptr(),actual.as_mut_ptr(),count,table.as_ptr());}
            for (pixel,(source,painted)) in input.chunks_exact(3).zip(actual.chunks_exact(3)).enumerate() {
                let at=((source[0] as usize/8)*1024+(source[1] as usize/8)*32+source[2] as usize/8)*3;
                assert_eq!(painted,&table[at..at+3],"count={count}, pixel={pixel}");
            }
        }
    }
    #[test] fn parallel_texture_generation_preserves_large_and_small_stroke_order() {
        let (w,h)=(129,97);
        let mut painter=Painter::new(w,h,"loaded").unwrap();
        let marks:Vec<[f32;12]>=(0..64).map(|i| {
            [i as f32+1.,(i%13) as f32*10.+0.23,(i%9) as f32*11.+0.77,
             if i<8 {37.} else {2.+(i%5) as f32},if i<8 {20.} else {1.+(i%3) as f32},
             i as f32*0.21,(i%23) as f32/23.,(i*43%256) as f32,(i*73%256) as f32,
             (i*31%256) as f32,0.8,0.]
        }).collect();
        let mut reference=vec![0;w*h*3];
        for pixel in reference.chunks_exact_mut(3) {pixel.copy_from_slice(&[81,83,74]);}
        for p in &marks {draw_reference(&mut reference,w,h,0,p,&material::stamp(p,material::Material::Loaded));}
        assert_eq!(reference,painter.paint(&marks.concat(),false).unwrap());
        // New texture jobs may complete in any order, but appends still layer
        // over the previous canvas in exactly the order the artist supplied.
        let mut accents=marks[..9].to_vec();
        for p in &mut accents {p[0]+=100.;p[1]+=0.19;p[10]=0.45;}
        for p in &accents {draw_reference(&mut reference,w,h,0,p,&material::stamp(p,material::Material::Loaded));}
        assert_eq!(reference,painter.paint(&accents.concat(),true).unwrap());
    }
    #[cfg(target_arch="x86_64")]
    #[test] fn avx2_matches_scalar_for_odd_rows_zero_alpha_and_saturated_pigment() {
        if !std::arch::is_x86_feature_detected!("avx2") {return;}
        for count in 0..35 {
            for phase in 0..256 {
                let mut expected:Vec<u8>=(0..count*4).map(|i|((i*73+phase*11)%256) as u8).collect();
                let mut actual=expected.clone();
                let coverage:Vec<[i16;2]>=(0..count).map(|i| {
                    let alpha=((i*61+phase)%256) as i16;
                    [alpha,if alpha==0 {17} else {alpha*((i%81) as i16-40)}]
                }).collect();
                let color=[phase as i32,255,0,255];
                blend_scalar(&mut expected,&coverage,color);
                unsafe {blend_avx2(&mut actual,&coverage,color);}
                assert_eq!(expected,actual,"count={count}, phase={phase}");
            }
        }
    }
    #[cfg(target_arch="aarch64")]
    #[test] fn neon_matches_scalar_for_odd_rows_zero_alpha_and_saturated_pigment() {
        for count in 0..35 {for phase in 0..256 {
            let mut expected:Vec<u8>=(0..count*4).map(|i|((i*73+phase*11)%256) as u8).collect();
            let mut actual=expected.clone();
            let coverage:Vec<[i16;2]>=(0..count).map(|i| {
                let alpha=((i*61+phase)%256) as i16;
                [alpha,if alpha==0 {17} else {alpha*((i%81) as i16-40)}]
            }).collect();
            let color=[phase as i32,255,0,255];
            blend_scalar(&mut expected,&coverage,color);
            unsafe {blend_neon(&mut actual,&coverage,color);}
            assert_eq!(expected,actual,"count={count}, phase={phase}");
        }}
    }
    #[test] fn sparse_blending_matches_scalar_paint_for_loaded_shadows_and_opacity() {
        for material in [material::Material::Loaded,material::Material::Scumble,material::Material::Swept] {
            for seed in 0..20 {
                let mut p=[1.,37.3,28.7,31.,13.,0.71,seed as f32/20.,0.,255.,170.,(seed+1) as f32/20.,0.];
                p[1]+=seed as f32*0.027;p[2]-=seed as f32*0.031;
                let mut stamp=material::stamp(&p,material);stamp.prepare(&p);
                let mut scalar=vec![0;79*61*3];let mut simd=vec![0;79*61*4];
                for (at,pixel) in scalar.chunks_exact_mut(3).enumerate() {pixel.copy_from_slice(&[(at%256) as u8,0,255]);}
                for (a,b) in simd.chunks_exact_mut(4).zip(scalar.chunks_exact(3)) {a[..3].copy_from_slice(b);a[3]=255;}
                draw_reference(&mut scalar,79,61,0,&p,&stamp);
                draw(&mut simd,79,61,0,&p,&stamp);
                assert_eq!(scalar,simd.chunks_exact(4).flat_map(|p|p[..3].to_vec()).collect::<Vec<_>>());
            }
        }
    }
    #[test] fn thread_bands_preserve_overlapping_subpixel_brushes_at_seams() {
        let p=[1.,37.3,28.7,31.,13.,0.71,0.32,150.,80.,170.,0.9,0.];
        let q=[2.,9.1,44.2,42.,8.,-0.7,0.8,95.,180.,40.,1.,1.];
        let mut cache=[(1,make_stamp(&p)),(2,make_stamp(&q))].into_iter().collect::<HashMap<_,_>>();
        cache.get_mut(&1).unwrap().prepare(&p);cache.get_mut(&2).unwrap().prepare(&q);
        let mut serial=vec![0;79*61*3];let mut parallel=vec![0;79*61*4];
        for pixel in serial.chunks_exact_mut(3) {pixel.copy_from_slice(&[81,83,74]);}
        draw_reference(&mut serial,79,61,0,&p,&cache[&1]);draw_reference(&mut serial,79,61,0,&q,&cache[&2]);
        raster(&mut parallel,79,61,&[p,q].concat(),&cache,4,true);
        assert_eq!(serial,parallel.chunks_exact(4).flat_map(|p|p[..3].to_vec()).collect::<Vec<_>>());
        // Refinement appends must preserve the entire existing canvas and
        // produce exactly the same pixels as a single ordered paint packet.
        let mut incremental=vec![0u8;79*61*4];
        raster(&mut incremental,79,61,&p,&cache,4,true);
        raster(&mut incremental,79,61,&q,&cache,4,false);
        assert_eq!(serial,incremental.chunks_exact(4).flat_map(|p|p[..3].to_vec()).collect::<Vec<_>>());
    }
}

/// Fused day/night grade avoids full-frame temporary arrays in the live worker.
/// The original palette anchors are supplied by the shared Python palette.
#[no_mangle]
pub unsafe extern "C" fn paint_grade(src:*const u8,dst:*mut u8,count:usize,night:f64,tint:*const f64) {
    if src.is_null() || dst.is_null() || tint.is_null() || count>4_147_200 {return;}
    let input=std::slice::from_raw_parts(src,count*3);
    let output=std::slice::from_raw_parts_mut(dst,count*3);
    let color=std::slice::from_raw_parts(tint,3);
    // The full-resolution target is independent per pixel. Split only large
    // targets so the small analysis image doesn't pay thread startup overhead.
    if count>=1_000_000 {
        let size=count.div_ceil(2)*3;
        std::thread::scope(|scope| {
            for (source,dest) in input.chunks(size).zip(output.chunks_mut(size)) {
                scope.spawn(move||grade_pixels(source,dest,night,color));
            }
        });
    } else {grade_pixels(input,output,night,color);}
}
/// Each pixel as the mixed pile a painter would reach for: a lookup in the
/// 32x32x32 table the Python `Piles` builds, by the top five bits of each channel.
#[no_mangle]
pub unsafe extern "C" fn paint_piles(src:*const u8,dst:*mut u8,count:usize,table:*const u8) {
    if src.is_null() || dst.is_null() || table.is_null() || count>4_147_200 {return;}
    let input=std::slice::from_raw_parts(src,count*3);
    let output=std::slice::from_raw_parts_mut(dst,count*3);
    let piles=std::slice::from_raw_parts(table,32768*3);
    if count>=1_000_000 {
        let size=count.div_ceil(2)*3;
        std::thread::scope(|scope| {
            for (source,dest) in input.chunks(size).zip(output.chunks_mut(size)) {
                scope.spawn(move||pile_pixels(source,dest,piles));
            }
        });
    } else {pile_pixels(input,output,piles);}
}
fn pile_pixels(input:&[u8],output:&mut [u8],piles:&[u8]) {
    for (source,dest) in input.chunks_exact(3).zip(output.chunks_exact_mut(3)) {
        let at=((source[0] as usize>>3)<<10|(source[1] as usize>>3)<<5|source[2] as usize>>3)*3;
        dest.copy_from_slice(&piles[at..at+3]);
    }
}
fn grade_pixels(input:&[u8],output:&mut [u8],night:f64,color:&[f64]) {
    #[cfg(target_arch="x86_64")]
    if input.len()>=18 && std::arch::is_x86_feature_detected!("avx2") {
        unsafe {grade_avx2(input,output,night,color);}
        return;
    }
    grade_scalar(input,output,night,color);
}
fn grade_scalar(input:&[u8],output:&mut [u8],night:f64,color:&[f64]) {
    let saturation=(1.4-night*0.98) as f32;
    let brightness=(1.04-night*0.42) as f32;
    let strength=0.75-night*0.68;
    for (p,q) in input.chunks_exact(3).zip(output.chunks_exact_mut(3)) {
        let rgb=[p[0] as f32,p[1] as f32,p[2] as f32];
        let gray=rgb[0]*0.299+rgb[1]*0.587+rgb[2]*0.114;
        let peak=rgb[0].max(rgb[1]).max(rgb[2]);
        let excess=(peak-150.).max(0.);
        let compressed=peak-excess+excess/(1.+excess/95.);
        let gain=1.+night as f32*(compressed/peak.max(1.)-1.);
        let weight=1.-night as f32*((peak-160.)/95.).clamp(0.,1.)*0.85;
        for k in 0..3 {
            let base=(gray+(rgb[k]-gray)*saturation)*gain*brightness;
            q[k]=(base as f64+color[k]*weight as f64*strength).clamp(0.,255.) as u8;
        }
    }
}

#[cfg(target_arch="x86_64")]
#[target_feature(enable="avx2")]
unsafe fn grade_avx2(input:&[u8],output:&mut [u8],night:f64,color:&[f64]) {
    use std::arch::x86_64::*;
    let zero=_mm_setzero_ps();let one=_mm_set1_ps(1.);let n=_mm_set1_ps(night as f32);
    let saturation=_mm_set1_ps((1.4-night*0.98) as f32);
    let brightness=_mm_set1_ps((1.04-night*0.42) as f32);
    let strength=_mm256_set1_pd(0.75-night*0.68);
    let byte_mask=_mm_set1_epi32(255);
    let unpack=_mm_setr_epi8(0,1,2,-128,3,4,5,-128,6,7,8,-128,9,10,11,-128);
    let pack=_mm_setr_epi8(0,1,2,4,5,6,8,9,10,12,13,14,-128,-128,-128,-128);
    let mut at=0;
    // Four RGB pixels occupy 12 bytes. Leave the tail to the scalar path so
    // each 16-byte unaligned load stays entirely within the supplied image.
    while at+16<=input.len() {
        let raw=_mm_shuffle_epi8(_mm_loadu_si128(input.as_ptr().add(at).cast()),unpack);
        let r=_mm_cvtepi32_ps(_mm_and_si128(raw,byte_mask));
        let g=_mm_cvtepi32_ps(_mm_and_si128(_mm_srli_epi32::<8>(raw),byte_mask));
        let b=_mm_cvtepi32_ps(_mm_and_si128(_mm_srli_epi32::<16>(raw),byte_mask));
        let gray=_mm_add_ps(_mm_add_ps(_mm_mul_ps(r,_mm_set1_ps(0.299)),_mm_mul_ps(g,_mm_set1_ps(0.587))),_mm_mul_ps(b,_mm_set1_ps(0.114)));
        let peak=_mm_max_ps(_mm_max_ps(r,g),b);
        let excess=_mm_max_ps(_mm_sub_ps(peak,_mm_set1_ps(150.)),zero);
        let compressed=_mm_add_ps(_mm_sub_ps(peak,excess),_mm_div_ps(excess,_mm_add_ps(one,_mm_div_ps(excess,_mm_set1_ps(95.)))));
        let gain=_mm_add_ps(one,_mm_mul_ps(n,_mm_sub_ps(_mm_div_ps(compressed,_mm_max_ps(peak,one)),one)));
        let level=_mm_min_ps(one,_mm_max_ps(zero,_mm_div_ps(_mm_sub_ps(peak,_mm_set1_ps(160.)),_mm_set1_ps(95.))));
        let weight=_mm256_cvtps_pd(_mm_sub_ps(one,_mm_mul_ps(_mm_mul_ps(n,level),_mm_set1_ps(0.85))));
        let mut channels=[_mm_setzero_si128();3];
        for (k,channel) in [r,g,b].into_iter().enumerate() {
            let base=_mm_mul_ps(_mm_mul_ps(_mm_add_ps(gray,_mm_mul_ps(_mm_sub_ps(channel,gray),saturation)),gain),brightness);
            let tint=_mm256_mul_pd(_mm256_mul_pd(_mm256_set1_pd(color[k]),weight),strength);
            let value=_mm256_add_pd(_mm256_cvtps_pd(base),tint);
            channels[k]=_mm256_cvttpd_epi32(_mm256_min_pd(_mm256_set1_pd(255.),_mm256_max_pd(_mm256_setzero_pd(),value)));
        }
        let rgba=_mm_or_si128(channels[0],_mm_or_si128(_mm_slli_epi32::<8>(channels[1]),_mm_slli_epi32::<16>(channels[2])));
        let rgb=_mm_shuffle_epi8(rgba,pack);
        let dest=output.as_mut_ptr().add(at);
        _mm_storel_epi64(dest.cast(),rgb);
        std::ptr::write_unaligned(dest.add(8).cast::<i32>(),_mm_extract_epi32::<2>(rgb));
        at+=12;
    }
    grade_scalar(&input[at..],&mut output[at..],night,color);
}

/// Sequential brush-effort acceptance against the same evolving float canvas.
/// Each candidate is x,y,half length,half width,angle,r,g,b (f64).
#[no_mangle]
pub unsafe extern "C" fn paint_select(work:*mut f32,target:*const f32,w:usize,h:usize,candidates:*const f64,count:usize,budget:usize,scale:f64,accepted:*mut u32)->usize {
    if work.is_null() || target.is_null() || candidates.is_null() || accepted.is_null() || w*h>4_147_200 || count>100_000 {return 0;}
    let working=std::slice::from_raw_parts_mut(work,w*h*3);
    let reference=std::slice::from_raw_parts(target,w*h*3);
    let marks=std::slice::from_raw_parts(candidates,count*8);
    let out=std::slice::from_raw_parts_mut(accepted,budget.min(count));
    let mut n=0;
    for (index,p) in marks.chunks_exact(8).enumerate() {
        if n>=out.len() {break;}
        let (sn,cs)=p[4].sin_cos();
        let bx=cs.abs()*p[2]+sn.abs()*p[3]*1.2+2.;
        let by=sn.abs()*p[2]+cs.abs()*p[3]*1.2+2.;
        let x0=((p[0]-bx) as i32).max(0) as usize;let x1=(((p[0]+bx) as i32+1).max(0) as usize).min(w);
        let y0=((p[1]-by) as i32).max(0) as usize;let y1=(((p[1]+by) as i32+1).max(0) as usize).min(h);
        let coverage=|x:usize,y:usize| {
            let dx=x as f64-p[0];let dy=y as f64-p[1];
            let u=(dx*cs+dy*sn)/p[2];if u.abs()>=1. {return 0.;}
            let v=(-dx*sn+dy*cs)/p[3];
            let cap=(1.-((u.abs()-0.5).max(0.)/0.5).powi(2)).max(0.).sqrt();
            ((cap*0.85-v.abs())*p[3]).clamp(0.,1.)*0.82
        };
        let mut gain=0.;let mut area=0.;
        for y in y0..y1 {for x in x0..x1 {
            let a=coverage(x,y);if a==0. {continue;}area+=a;
            let at=(y*w+x)*3;
            for k in 0..3 {
                let present=working[at+k] as f64;let target=reference[at+k] as f64;
                let predicted=present*(1.-a)+p[5+k]*a;
                gain+=(target-present).powi(2)-(target-predicted).powi(2);
            }
        }}
        if gain/3.<=120.*area+1200.*scale*scale {continue;}
        out[n]=index as u32;n+=1;
        for y in y0..y1 {for x in x0..x1 {
            let a=coverage(x,y);if a==0. {continue;}
            let at=(y*w+x)*3;
            for k in 0..3 {working[at+k]=(working[at+k] as f64*(1.-a)+p[5+k]*a) as f32;}
        }}
    }
    n
}
