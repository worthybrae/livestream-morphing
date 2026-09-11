//! Local prototype only. Rasterizes persistent marks; never reads the video.
//! Input per frame: little-endian u32 count, then 12 f32 values per mark.
use std::{collections::{HashMap, HashSet}, io::{self, Read, Write}};
mod material;

struct Stamp { w:usize, h:usize, ox:i32, oy:i32, pixels:Vec<[i16;2]> }
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
    Stamp {w,h,ox:-bx,oy:-by,pixels}
}
fn draw(canvas:&mut [u8], w:usize,h:usize,top:usize,p:&[f32],stamp:&Stamp) {
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
fn raster(canvas:&mut [u8],w:usize,h:usize,values:&[f32],cache:&HashMap<u32,Stamp>,threads:usize,clear:bool) {
    let rows=h.div_ceil(threads);
    std::thread::scope(|scope| {
        for (band,dst) in canvas.chunks_mut(rows*w*3).enumerate() {
            scope.spawn(move|| {
                let top=band*rows;let bh=dst.len()/(w*3);
                if clear {for pixel in dst.chunks_exact_mut(3) {pixel.copy_from_slice(&[81,83,74]);}}
                for p in values.chunks_exact(12) {
                    if let Some(stamp)=cache.get(&(p[0] as u32)) {
                        let sy=p[2].floor() as i32+stamp.oy;
                        if sy<=(top+bh) as i32 && sy+stamp.h as i32>=top as i32 {draw(dst,w,bh,top,p,stamp);}
                    }
                }
            });
        }
    });
}
fn main()->io::Result<()> {
    let args:Vec<_>=std::env::args().collect();
    let w:usize=args[1].parse().unwrap();let h:usize=args[2].parse().unwrap();
    let material=material::Material::parse(args.get(3).map(String::as_str).unwrap_or("plain"))
        .ok_or_else(||io::Error::new(io::ErrorKind::InvalidInput,"Unknown paint material"))?;
    let mut input=io::stdin().lock();let mut output=io::stdout().lock();
    let mut cache=HashMap::<u32,Stamp>::new();let mut header=[0;4];
    let mut canvas=vec![0u8;w*h*3];let mut raster_ms=0.;let mut frames=0;
    loop {
        match input.read_exact(&mut header) {Ok(())=>(),Err(e) if e.kind()==io::ErrorKind::UnexpectedEof=>break,Err(e)=>return Err(e)}
        let packet=u32::from_le_bytes(header);
        let append=packet & (1<<31)!=0;
        let count=(packet & !(1<<31)) as usize;
        if count>100_000 {return Err(io::Error::new(io::ErrorKind::InvalidData,"Unbounded stroke packet"));}
        let mut bytes=vec![0;count*12*4];input.read_exact(&mut bytes)?;
        let values:Vec<f32>=bytes.chunks_exact(4).map(|b|f32::from_le_bytes(b.try_into().unwrap())).collect();
        let start=std::time::Instant::now();let mut active=HashSet::with_capacity(count);
        for p in values.chunks_exact(12) {
            if !p.iter().all(|v|v.is_finite()) || p[3]<=0. || p[4]<=0. || p[3]>512. || p[4]>512. {continue;}
            let id=p[0] as u32;active.insert(id);
            cache.entry(id).or_insert_with(||material::stamp(p,material));
        }
        if !append {cache.retain(|id,_|active.contains(id));}
        raster(&mut canvas,w,h,&values,&cache,4,!append);
        raster_ms+=start.elapsed().as_secs_f64()*1000.;frames+=1;
        output.write_all(&canvas)?;output.flush()?;
    }
    eprintln!("{{\"painted_frames\":{frames},\"raster_ms\":{raster_ms},\"cached_strokes\":{}}}",cache.len());Ok(())
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn thread_bands_preserve_overlapping_subpixel_brushes_at_seams() {
        let p=[1.,37.3,28.7,31.,13.,0.71,0.32,150.,80.,170.,0.9,0.];
        let q=[2.,9.1,44.2,42.,8.,-0.7,0.8,95.,180.,40.,1.,1.];
        let cache=HashMap::from([(1,make_stamp(&p)),(2,make_stamp(&q))]);
        let mut serial=vec![0;79*61*3];let mut parallel=serial.clone();
        for pixel in serial.chunks_exact_mut(3) {pixel.copy_from_slice(&[81,83,74]);}
        draw(&mut serial,79,61,0,&p,&cache[&1]);draw(&mut serial,79,61,0,&q,&cache[&2]);
        raster(&mut parallel,79,61,&[p,q].concat(),&cache,4,true);
        assert_eq!(serial,parallel);
        // Refinement appends must preserve the entire existing canvas and
        // produce exactly the same pixels as a single ordered paint packet.
        let mut incremental=vec![0u8;79*61*3];
        raster(&mut incremental,79,61,&p,&cache,4,true);
        raster(&mut incremental,79,61,&q,&cache,4,false);
        assert_eq!(serial,incremental);
    }
}
