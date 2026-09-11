use std::io::{self,Read,Write};
use stroke_study_raster::Painter;
fn main()->io::Result<()> {
    let args:Vec<_>=std::env::args().collect();
    let w:usize=args[1].parse().unwrap();let h:usize=args[2].parse().unwrap();
    let mut painter=Painter::new(w,h,args.get(3).map(String::as_str).unwrap_or("plain"))
        .ok_or_else(||io::Error::new(io::ErrorKind::InvalidInput,"Invalid raster configuration"))?;
    let mut input=io::stdin().lock();let mut output=io::stdout().lock();let mut header=[0;4];
    let mut raster_ms=0.;let mut frames=0;
    loop {
        match input.read_exact(&mut header) {Ok(())=>(),Err(e) if e.kind()==io::ErrorKind::UnexpectedEof=>break,Err(e)=>return Err(e)}
        let packet=u32::from_le_bytes(header);let append=packet&(1<<31)!=0;let count=(packet&!(1<<31)) as usize;
        if count>100_000 {return Err(io::Error::new(io::ErrorKind::InvalidData,"Unbounded stroke packet"));}
        let mut bytes=vec![0;count*48];input.read_exact(&mut bytes)?;
        let values:Vec<f32>=bytes.chunks_exact(4).map(|b|f32::from_le_bytes(b.try_into().unwrap())).collect();
        let start=std::time::Instant::now();
        let canvas=painter.paint(&values,append).ok_or_else(||io::Error::new(io::ErrorKind::InvalidData,"Invalid marks"))?;
        raster_ms+=start.elapsed().as_secs_f64()*1000.;frames+=1;
        output.write_all(canvas)?;output.flush()?;
    }
    eprintln!("{{\"painted_frames\":{frames},\"raster_ms\":{raster_ms}}}");Ok(())
}
