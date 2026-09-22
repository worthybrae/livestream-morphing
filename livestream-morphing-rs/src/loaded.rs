//! Persistent, bounded local paint worker. HLS timestamps and wave stay in Rust.
use std::{io::{Read,Write}, os::fd::OwnedFd, os::unix::net::UnixStream,
    process::{Child,Command,Stdio},time::{Duration,Instant}};
use crate::{codec,effects::RawFrame,pipeline::Pipeline,renderer::Rendered};
type Error=Box<dyn std::error::Error+Send+Sync>;

#[derive(Clone,Copy,Default,Debug)]
struct PaintTelemetry {
    planner:bool,mutated_pixels:u64,transport_ms:f64,planner_ms:f64,
    dirty_ms:f64,worker_cpu_ms:f64,full_raster_ms:f64,worker_rss_bytes:u64,
}
impl PaintTelemetry {
    fn decode(packet:[u8;48])->Result<Self,Error> {
        if &packet[..4]!=b"PNT2" || packet[4]>1 {return Err("Invalid paint telemetry header".into());}
        let u64_at=|at:usize|u64::from_le_bytes(packet[at..at+8].try_into().unwrap());
        let f32_at=|at:usize|f32::from_le_bytes(packet[at..at+4].try_into().unwrap()) as f64;
        let values=[f32_at(16),f32_at(20),f32_at(24),f32_at(28),f32_at(32)];
        if values.iter().any(|value|!value.is_finite() || *value<0.) {
            return Err("Invalid paint telemetry timing".into());
        }
        Ok(Self {planner:packet[4]!=0,mutated_pixels:u64_at(8),transport_ms:values[0],
            planner_ms:values[1],dirty_ms:values[2],worker_cpu_ms:values[3],
            full_raster_ms:values[4],worker_rss_bytes:u64_at(40)})
    }
}

#[derive(Clone,Copy,Debug,PartialEq)]
enum Protocol { Paint1,Paint2 }
struct Worker { child:Child,socket:UnixStream,width:u32,height:u32,protocol:Protocol }
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
        let mut ready=[0u8;6];socket.read_exact(&mut ready)?;
        let protocol=match &ready {b"PAINT1"=>Protocol::Paint1,b"PAINT2"=>Protocol::Paint2,
            _=>return Err("Paint worker handshake failed".into())};
        let worker=Worker {child,socket:socket.try_clone()?,width,height,protocol};
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
    fn receive(&mut self)->Result<(RawFrame,PaintTelemetry),Error> {
        let worker=self.worker.as_mut().ok_or("No paint worker")?;
        let telemetry=if worker.protocol==Protocol::Paint2 {
            let mut packet=[0u8;48];worker.socket.read_exact(&mut packet)?;
            PaintTelemetry::decode(packet)?
        } else {PaintTelemetry::default()};
        let mut painted=RawFrame::new(worker.width,worker.height);
        worker.socket.read_exact(&mut painted.data)?;
        Ok((painted,telemetry))
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
    let mut paint_updates=0u64;let mut planner_updates=0u64;let mut telemetry=PaintTelemetry::default();
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
            let (frame,update)=paint.receive()?;paint.held=Some(frame);paint_updates+=1;
            if update.planner {planner_updates+=1;}
            telemetry.mutated_pixels+=update.mutated_pixels;
            telemetry.transport_ms+=update.transport_ms;telemetry.planner_ms+=update.planner_ms;
            telemetry.dirty_ms+=update.dirty_ms;telemetry.worker_cpu_ms+=update.worker_cpu_ms;
            telemetry.full_raster_ms+=update.full_raster_ms;
            telemetry.worker_rss_bytes=telemetry.worker_rss_bytes.max(update.worker_rss_bytes);
            times.push(t.elapsed().as_secs_f64()*1000.);
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
    Ok(Rendered {#[cfg(test)] painted_frames,bytes,fps,paint_fps:paint_updates as f64*fps/info.frames.max(1) as f64,frames:info.frames,width:dimensions.0,height:dimensions.1,
        first_seconds:info.first_seconds,times,decode_ms,encode_ms,post_ms,repaint_ms:0.,warp_ms,
        paint_updates,planner_updates,mutated_pixels:telemetry.mutated_pixels,
        transport_ms:telemetry.transport_ms,planner_ms:telemetry.planner_ms,dirty_ms:telemetry.dirty_ms,
        worker_cpu_ms:telemetry.worker_cpu_ms,full_raster_ms:telemetry.full_raster_ms,worker_rss_bytes:telemetry.worker_rss_bytes})
    })();
    if result.is_err() {pipeline.reset();}
    result
}

#[cfg(test)] mod tests {
    use super::*;
    fn paint2_packet()->[u8;48] {
        let mut packet=[0u8;48];
        packet[..4].copy_from_slice(b"PNT2");
        packet[4]=1;
        packet[8..16].copy_from_slice(&1234u64.to_le_bytes());
        for (offset,value) in [(16,1.5f32),(20,2.5),(24,3.5),(28,4.5),(32,5.5)] {
            packet[offset..offset+4].copy_from_slice(&value.to_le_bytes());
        }
        packet[40..48].copy_from_slice(&4096u64.to_le_bytes());
        packet
    }
    #[test] fn paint2_decodes_python_telemetry_packet() {
        let telemetry=PaintTelemetry::decode(paint2_packet()).unwrap();
        assert!(telemetry.planner);
        assert_eq!(telemetry.mutated_pixels,1234);
        assert_eq!(telemetry.transport_ms,1.5);
        assert_eq!(telemetry.planner_ms,2.5);
        assert_eq!(telemetry.dirty_ms,3.5);
        assert_eq!(telemetry.worker_cpu_ms,4.5);
        assert_eq!(telemetry.full_raster_ms,5.5);
        assert_eq!(telemetry.worker_rss_bytes,4096);
    }
    #[test] fn paint2_rejects_wrong_magic_and_non_finite_timings() {
        let mut wrong=paint2_packet();wrong[..4].copy_from_slice(b"NOPE");
        assert!(PaintTelemetry::decode(wrong).is_err());
        let mut invalid=paint2_packet();invalid[16..20].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(PaintTelemetry::decode(invalid).is_err());
    }
    #[test] fn paint2_worker_returns_telemetry_before_pixels() {
        let mut command=Command::new("python3");
        command.arg("-c").arg(r#"
import socket,struct
s=socket.socket(fileno=0)
s.sendall(b'PAINT2')
left=8+64*36*3
while left:
    chunk=s.recv(left)
    if not chunk: raise SystemExit(1)
    left-=len(chunk)
s.sendall(struct.pack('<4sB3xQ5f4xQ',b'PNT2',1,1234,1.5,2.5,3.5,4.5,5.5,4096))
s.sendall(bytes([77])*64*36*3)
"#);
        let worker=LoadedPaint::connect(command,64,36,Duration::from_secs(2)).unwrap();
        let mut paint=LoadedPaint::new();paint.worker=Some(worker);
        paint.request(&RawFrame::filled(64,36,1,2,3)).unwrap();
        let (frame,telemetry)=paint.receive().unwrap();
        assert_eq!(frame.data[100],77);
        assert!(telemetry.planner);
        assert_eq!(telemetry.mutated_pixels,1234);
    }
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
    fn fake2()->LoadedPaint {
        let mut command=Command::new("python3");
        command.arg("-c").arg(r#"
import socket,struct
s=socket.socket(fileno=0)
s.sendall(b'PAINT2')
n=0
while True:
    left=8+64*36*3
    while left:
        chunk=s.recv(left)
        if not chunk: raise SystemExit(0)
        left-=len(chunk)
    n+=1
    s.sendall(struct.pack('<4sB3xQ5f4xQ',b'PNT2',n%3==1,n,1.,2.,3.,4.,5.,4096+n))
    s.sendall(bytes([n*30])*64*36*3)
"#);
        let worker=LoadedPaint::connect(command,64,36,Duration::from_secs(2)).unwrap();
        let mut paint=LoadedPaint::new();paint.stride=1.;paint.min_stride=1.;paint.worker=Some(worker);paint
    }
    #[test] fn paint2_segment_aggregates_transactions_planners_timings_and_rss() {
        codec::init();let mut p=Pipeline::new();p.loaded=Some(fake2());
        let input=codec::encode_segment(&vec![RawFrame::filled(64,36,100,100,100);5],30,0).unwrap();
        let rendered=render(&input,64,1,0,0,&mut p,720).unwrap();
        assert_eq!(rendered.paint_updates,5);
        assert_eq!(rendered.planner_updates,2);
        assert_eq!(rendered.mutated_pixels,15);
        assert_eq!(rendered.transport_ms,5.);
        assert_eq!(rendered.planner_ms,10.);
        assert_eq!(rendered.dirty_ms,15.);
        assert_eq!(rendered.worker_cpu_ms,20.);
        assert_eq!(rendered.full_raster_ms,25.);
        assert_eq!(rendered.worker_rss_bytes,4101);
    }
    #[test] fn held_canvas_and_worker_survive_an_uneven_segment_boundary() {
        codec::init();let mut p=Pipeline::new();p.loaded=Some(fake());
        let input=codec::encode_segment(&vec![RawFrame::filled(64,36,100,100,100);5],30,0).unwrap();
        let a=render(&input,64,1,0,0,&mut p,720).unwrap();
        assert_eq!(a.frames,5);assert_eq!(a.paint_fps,12.);
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
