//! Persistent, bounded local paint worker. HLS timestamps and wave stay in Rust.
use std::{io::{Read,Write}, os::fd::OwnedFd, os::unix::net::UnixStream,
    process::{Child,Command,Stdio},time::{Duration,Instant}};
use crate::{codec,effects::RawFrame,pipeline::Pipeline,renderer::Rendered};
type Error=Box<dyn std::error::Error+Send+Sync>;

struct Worker { child:Child,socket:UnixStream,width:u32,height:u32 }
impl Drop for Worker {
    fn drop(&mut self) {let _=self.child.kill();let _=self.child.wait();}
}
pub struct LoadedPaint { worker:Option<Worker>,pub hour:f64,pub stride:f64,min_stride:f64,quiet_segments:u8,recent_costs:std::collections::VecDeque<(f64,f64)>,held:Option<RawFrame>,pub output_frames:u64 }
impl LoadedPaint {
    pub fn new()->Self {
        let stride=std::env::var("LOADED_PAINT_STRIDE").ok().and_then(|v|v.parse::<f64>().ok()).filter(|v|v.is_finite()).unwrap_or(3.).clamp(2.,12.);
        Self {worker:None,hour:15.,stride,min_stride:stride,quiet_segments:0,recent_costs:std::collections::VecDeque::new(),held:None,output_frames:0}
    }
    pub fn reset(&mut self) {self.worker=None;self.held=None;self.output_frames=0;self.quiet_segments=0;self.recent_costs.clear();}
    pub fn adapt(&mut self,seconds:f64,duration:f64) {
        if !seconds.is_finite() || !duration.is_finite() || seconds<=0. || duration<=0. {return;}
        // Keep the texture/resolution and display clock fixed. Only the next
        // segment's paint sampling may relax when compute approaches playback.
        // The rolling playback buffer absorbs isolated scheduling/traffic
        // spikes. Measure sustained pressure across up to five segments, but
        // still react immediately when a segment takes 25% too long.
        self.recent_costs.push_back((seconds,duration));
        if self.recent_costs.len()>5 {self.recent_costs.pop_front();}
        let (cost,playback)=self.recent_costs.iter().fold((0.,0.),|(a,b),(c,d)|(a+c,b+d));
        // Near 10–15 FPS, fractional steps can use more of the existing
        // playback budget. Keep at least 5% average headroom and let the
        // bounded segment buffer absorb short spikes. Slower rates retain
        // the larger margin because their next step is proportionally bigger.
        let pressure=if self.stride<=3. {0.95} else {0.90};
        let recovery=if self.stride<=3. {0.78} else {0.75};
        if seconds>duration*1.25 || (self.recent_costs.len()>=3 && cost>playback*pressure) {
            self.stride=if self.stride<3. {(self.stride+0.5).min(3.)} else {(self.stride+1.).min(12.)};self.quiet_segments=0;
            self.recent_costs.clear();
        } else if cost<playback*recovery {
            self.quiet_segments+=1;
            if self.quiet_segments>=6 {
                self.stride=(self.stride-if self.stride<=3. {0.5} else {1.}).max(self.min_stride);self.quiet_segments=0;
                self.recent_costs.clear();
            }
        } else {self.quiet_segments=0;}
    }
    fn paint_due(&self)->bool {
        // Fractional intervals provide 12 FPS between the 10/15 FPS settings.
        // Use the continuous output clock, including across segment boundaries;
        // held frames still receive a fresh wave and encoding at source cadence.
        self.output_frames==0 || (self.output_frames as f64/self.stride).floor()
            !=((self.output_frames-1) as f64/self.stride).floor()
    }
    fn connect(mut command:Command,width:u32,height:u32,timeout:Duration)->Result<Worker,Error> {
        let (mut socket,peer)=UnixStream::pair()?;
        socket.set_read_timeout(Some(timeout))?;socket.set_write_timeout(Some(timeout))?;
        let fd:OwnedFd=peer.into();
        let child=command.stdin(Stdio::from(fd)).stdout(Stdio::null()).stderr(Stdio::inherit()).spawn()?;
        let mut worker=Worker {child,socket:socket.try_clone()?,width,height};
        let mut ready=[0u8;6];socket.read_exact(&mut ready)?;
        if &ready!=b"PAINT1" {return Err("Paint worker handshake failed".into());}
        worker.socket.set_read_timeout(Some(Duration::from_secs(12)))?;
        worker.socket.set_write_timeout(Some(Duration::from_secs(12)))?;
        Ok(worker)
    }
    fn spawn(width:u32,height:u32)->Result<Worker,Error> {
        let python=std::env::var("LOADED_PYTHON").unwrap_or_else(|_|"/opt/paint/bin/python".into());
        let script=std::env::var("LOADED_WORKER").unwrap_or_else(|_|"/opt/paint-worker/live.py".into());
        let mut command=Command::new(python);
        command.arg("-u").arg(script).arg(width.to_string()).arg(height.to_string())
            .env("OPENBLAS_NUM_THREADS","1").env("OMP_NUM_THREADS","2");
        Self::connect(command,width,height,Duration::from_secs(20))
    }
    fn request(&mut self,frame:&RawFrame)->Result<(),Error> {
        if self.worker.as_ref().map(|w|(w.width,w.height))!=Some((frame.width,frame.height)) {
            self.reset();self.worker=Some(Self::spawn(frame.width,frame.height)?);
        }
        let worker=self.worker.as_mut().unwrap();
        worker.socket.write_all(&self.hour.to_le_bytes())?;
        worker.socket.write_all(&frame.data)?;
        Ok(())
    }
    fn receive(&mut self)->Result<RawFrame,Error> {
        let worker=self.worker.as_mut().ok_or("No paint worker")?;
        let mut painted=RawFrame::new(worker.width,worker.height);
        worker.socket.read_exact(&mut painted.data)?;
        Ok(painted)
    }

}

fn emit(held:&RawFrame,count:usize,first:u64,fps:f64,repaint:&mut crate::repaint::Repaint,encoder:&mut codec::TsEncoder,encode_ms:&mut f64,post_ms:&mut f64,
    #[cfg(test)] painted_frames:&mut Vec<Vec<u8>>) -> Result<(),Error> {
    for i in 0..count {
        let mut output=held.clone();let t=Instant::now();
        repaint.move_canvas(&mut output,(first+i as u64) as u32,(1./fps) as f32);
        *post_ms+=t.elapsed().as_secs_f64()*1000.;
        #[cfg(test)] painted_frames.push(output.data.clone());
        let t=Instant::now();encoder.push(&output)?;*encode_ms+=t.elapsed().as_secs_f64()*1000.;
    }
    Ok(())
}

pub fn render(input:&[u8],width:u32,step:usize,pts:i64,offset:u64,pipeline:&mut Pipeline,limit:usize)->Result<Rendered,Error> {
    let mut encoder=None;let mut times=Vec::new();let mut encode_ms=0.;let mut callback_ms=0.;let mut post_ms=0.;
    let mut pending=0usize;let mut emitted=0u64;
    let mut dimensions=(0,0);let started=Instant::now();
    #[cfg(test)] let mut painted_frames=Vec::new();
    let result=(|| -> Result<Rendered,Error> {
    let info=codec::decode_visit_lazy(input,width,step,limit,|convert,size,fps,_,first| {
        let t=Instant::now();let out_fps=fps/step as f64;
        if encoder.is_none() {
            if let (Some(last),Some(now))=(pipeline.source_end,first) {
                // A skipped live segment must not repeatedly destroy the
                // expensive brush cache during catch-up. Motion confidence
                // retires displaced marks; a restart or long outage resets all.
                if now<last-2./fps || now-last>30. {pipeline.reset();}
            }
            dimensions=size;
            let t=Instant::now();encoder=Some(codec::TsEncoder::new(size.0,size.1,out_fps,pts)?);
            encode_ms+=t.elapsed().as_secs_f64()*1000.;
        }
        let paint=pipeline.loaded.as_mut().ok_or("Loaded paint missing")?;
        if paint.held.as_ref().map(|f|(f.width,f.height))!=Some(dimensions) || paint.paint_due() {
            let t=Instant::now();paint.request(&convert()?)?;
            // The worker paints the next target while Rust waves/encodes the
            // preceding held canvas. Neither side owns an unbounded frame queue.
            if pending>0 {
                emit(paint.held.as_ref().unwrap(),pending,offset+emitted,out_fps,&mut pipeline.repaint,encoder.as_mut().unwrap(),&mut encode_ms,&mut post_ms,
                    #[cfg(test)] &mut painted_frames)?;
                emitted+=pending as u64;pending=0;
            }
            paint.held=Some(paint.receive()?);times.push(t.elapsed().as_secs_f64()*1000.);
        }
        paint.output_frames+=1;pending+=1;
        callback_ms+=t.elapsed().as_secs_f64()*1000.;Ok(())
    })?;
    let decode_ms=(started.elapsed().as_secs_f64()*1000.-callback_ms).max(0.);
    let fps=info.fps/step as f64;
    if pending>0 {
        emit(pipeline.loaded.as_ref().unwrap().held.as_ref().unwrap(),pending,offset+emitted,fps,&mut pipeline.repaint,encoder.as_mut().ok_or("No encoder")?,&mut encode_ms,&mut post_ms,
            #[cfg(test)] &mut painted_frames)?;
    }
    let t=Instant::now();let bytes=encoder.ok_or("No decoded frames")?.finish()?;
    encode_ms+=t.elapsed().as_secs_f64()*1000.;
    pipeline.source_end=info.first_seconds.map(|first|first+info.frames as f64/fps);
    let (_,warp_ms)=pipeline.repaint.take_timings();
    Ok(Rendered {#[cfg(test)] painted_frames,bytes,fps,paint_fps:fps/pipeline.loaded.as_ref().unwrap().stride,frames:info.frames,width:dimensions.0,height:dimensions.1,
        first_seconds:info.first_seconds,times,decode_ms,encode_ms,post_ms,repaint_ms:0.,warp_ms})
    })();
    if result.is_err() {pipeline.reset();}
    result
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn faster_paint_uses_sustainable_headroom_without_ignoring_overload() {
        let mut paint=LoadedPaint::new();paint.min_stride=2.5;paint.stride=2.5;
        for seconds in [5.2,5.4,5.5,5.6,5.5,5.4,5.6,5.5] {
            paint.adapt(seconds,6.);assert_eq!(paint.stride,2.5);
        }
        for _ in 0..5 {paint.adapt(5.9,6.);}
        assert_eq!(paint.stride,3.);
        // One slightly busier segment must not erase the otherwise sustained
        // recovery window. Six quiet windows are still required after a drop.
        paint.recent_costs.clear();paint.quiet_segments=0;
        for seconds in [4.55,4.5,4.7,4.55,4.6] {
            paint.adapt(seconds,6.);assert_eq!(paint.stride,3.);
        }
        paint.adapt(4.55,6.);assert_eq!(paint.stride,2.5);
        paint.adapt(8.,6.);assert_eq!(paint.stride,3.);
    }
    #[test] fn fractional_cadence_fills_the_gap_between_ten_and_fifteen() {
        let mut paint=LoadedPaint::new();paint.min_stride=2.;paint.stride=2.;
        for _ in 0..3 {paint.adapt(5.8,6.);}
        assert_eq!(paint.stride,2.5);
        let sampled:Vec<_>=(0..30).filter(|n| {paint.output_frames=*n;paint.paint_due()}).collect();
        assert_eq!(sampled,vec![0,3,5,8,10,13,15,18,20,23,25,28]);
        for _ in 0..3 {paint.adapt(5.8,6.);}
        assert_eq!(paint.stride,3.);
        for _ in 0..6 {paint.adapt(3.,6.);}
        assert_eq!(paint.stride,2.5);
        for _ in 0..6 {paint.adapt(3.,6.);}
        assert_eq!(paint.stride,2.);
        paint.min_stride=2.5;paint.stride=3.;
        for _ in 0..18 {paint.adapt(3.,6.);}
        assert_eq!(paint.stride,2.5);
    }

    #[test] fn twelve_fps_holds_and_continues_across_short_segments() {
        codec::init();let mut p=Pipeline::new();p.loaded=Some(fake());
        p.loaded.as_mut().unwrap().stride=2.5;
        let input=codec::encode_segment(&vec![RawFrame::filled(64,36,100,100,100);5],30,0).unwrap();
        let a=render(&input,64,1,0,0,&mut p,720).unwrap();
        assert_eq!(a.paint_fps,12.);
        assert_eq!(a.painted_frames.iter().map(|f|f[100]).collect::<Vec<_>>(),vec![30,30,30,60,60]);
        p.source_end=codec::decode_sized(&input,64).unwrap().first_seconds;
        let b=render(&input,64,1,15000,5,&mut p,720).unwrap();
        assert_eq!(b.frames,5);
        assert_eq!(b.painted_frames.iter().map(|f|f[100]).collect::<Vec<_>>(),vec![90,90,90,120,120]);
        assert_eq!(p.loaded.as_ref().unwrap().output_frames,10);
    }

    #[test] fn isolated_processing_spike_does_not_lower_sustainable_paint_rate() {
        let mut paint=LoadedPaint::new();paint.min_stride=3.;paint.stride=3.;
        for seconds in [4.5,4.5,4.5,6.2,4.8,4.8,4.8,4.8,4.8] {
            paint.adapt(seconds,6.);
            assert_eq!(paint.stride,3.);
        }
    }
    #[test] fn sustained_pressure_and_severe_stalls_still_reduce_paint_rate() {
        let mut paint=LoadedPaint::new();paint.min_stride=3.;paint.stride=3.;
        paint.adapt(5.8,6.);paint.adapt(5.8,6.);
        assert_eq!(paint.stride,3.);
        paint.adapt(5.8,6.);assert_eq!(paint.stride,4.);
        paint.adapt(8.,6.);assert_eq!(paint.stride,5.);
        for _ in 0..6 {paint.adapt(3.,6.);}
        assert_eq!(paint.stride,4.);
        paint.reset();
        assert_eq!(paint.stride,4.);
        paint.adapt(5.8,6.);assert_eq!(paint.stride,4.);
    }
    #[test] fn cadence_leaves_headroom_and_recovers_slowly_within_bounds() {
        let mut paint=LoadedPaint::new();paint.min_stride=8.;paint.stride=8.;
        paint.adapt(8.,6.);assert_eq!(paint.stride,9.);
        for _ in 0..20 {paint.adapt(8.,6.);}
        assert_eq!(paint.stride,12.);
        for _ in 0..5 {paint.adapt(3.,6.);}
        assert_eq!(paint.stride,12.);
        paint.adapt(3.,6.);assert_eq!(paint.stride,11.);
        for _ in 0..100 {paint.adapt(3.,6.);}
        assert_eq!(paint.stride,8.);
        paint.adapt(f64::NAN,6.);paint.adapt(1.,0.);assert_eq!(paint.stride,8.);
    }
    fn fake()->LoadedPaint {
        let mut command=Command::new("python3");
        command.arg("-c").arg(r#"
import socket
s=socket.socket(fileno=0)
s.sendall(b'PAINT1')
n=0
while True:
    left=8+64*36*3
    while left:
        chunk=s.recv(left)
        if not chunk: raise SystemExit(0)
        left-=len(chunk)
    n+=1
    s.sendall(bytes([n*30])*64*36*3)
"#);
        let worker=LoadedPaint::connect(command,64,36,Duration::from_secs(2)).unwrap();
        let mut paint=LoadedPaint::new();paint.stride=3.;paint.min_stride=3.;paint.worker=Some(worker);paint
    }
    #[test] fn held_canvas_and_worker_survive_an_uneven_segment_boundary() {
        codec::init();let mut p=Pipeline::new();p.loaded=Some(fake());
        let input=codec::encode_segment(&vec![RawFrame::filled(64,36,100,100,100);5],30,0).unwrap();
        let a=render(&input,64,1,0,0,&mut p,720).unwrap();
        assert_eq!(a.frames,5);assert_eq!(a.paint_fps,10.);
        assert_eq!(a.painted_frames.iter().map(|f|f[100]).collect::<Vec<_>>(),vec![30,30,30,60,60]);
        // The fixture has its own input timestamp origin. Set contiguous end
        // explicitly to exercise the held-canvas continuation rather than gap reset.
        let input=codec::encode_segment(&vec![RawFrame::filled(64,36,100,100,100);4],30,15000).unwrap();
        let start=codec::decode_sized(&input,64).unwrap().first_seconds;
        p.source_end=start;
        let b=render(&input,64,1,15000,5,&mut p,720).unwrap();
        assert_eq!(b.painted_frames.iter().map(|f|f[100]).collect::<Vec<_>>(),vec![60,90,90,90]);
        assert_eq!(p.loaded.as_ref().unwrap().output_frames,9);
        // Skipping one six-second upstream segment must keep the costly cache
        // alive, so startup catch-up does not repeatedly become another startup.
        p.source_end=start.map(|v|v-6.006);
        let c=render(&input,64,1,30000,9,&mut p,720).unwrap();
        assert_eq!(c.painted_frames.iter().map(|f|f[100]).collect::<Vec<_>>(),vec![120,120,120,150]);
        assert_eq!(p.loaded.as_ref().unwrap().output_frames,13);
        p.loaded.as_mut().unwrap().stride=4.;p.source_end=start;
        let d=render(&input,64,1,45000,13,&mut p,720).unwrap();
        assert_eq!(d.frames,4);assert_eq!(d.paint_fps,7.5);
        assert_eq!(d.painted_frames.iter().map(|f|f[100]).collect::<Vec<_>>(),vec![150,150,150,180]);
        p.reset();assert!(p.loaded.as_ref().unwrap().worker.is_none());assert!(p.loaded.as_ref().unwrap().held.is_none());
    }
    #[test] fn stalled_or_invalid_worker_cannot_hang_startup() {
        let started=Instant::now();
        assert!(LoadedPaint::connect(Command::new("cat"),64,36,Duration::from_millis(50)).is_err());
        assert!(started.elapsed()<Duration::from_secs(2));
        let mut command=Command::new("sh");command.arg("-c").arg("printf WRONG! >&0");
        assert!(LoadedPaint::connect(command,64,36,Duration::from_secs(1)).is_err());
    }
}
