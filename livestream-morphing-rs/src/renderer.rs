//! Bounded frame processing shared by native replay and live ingestion.
use crate::{codec, effects::RawFrame, pipeline::Pipeline};
use std::{sync::mpsc, time::Instant};
type Error=Box<dyn std::error::Error+Send+Sync>;
pub struct Rendered {
    #[cfg(test)]
    pub painted_frames: Vec<Vec<u8>>,
    pub bytes:Vec<u8>, pub fps:f64, pub frames:usize, pub width:u32, pub height:u32,
    pub first_seconds:Option<f64>, pub times:Vec<f64>, pub decode_ms:f64, pub encode_ms:f64,
}
struct Job { frame:RawFrame, number:u32, dt:f32 }
struct Done { frame:RawFrame, milliseconds:f64 }
struct Worker { input:mpsc::SyncSender<Job>, output:mpsc::Receiver<Done> }

pub fn render(input:&[u8],width:u32,step:usize,pts:i64,offset:u64,pipeline:&mut Pipeline,requested_workers:usize)->Result<Rendered,Error> {
    render_bounded(input,width,step,pts,offset,pipeline,requested_workers,720)
}

/// Longer local artwork exports keep the same bounded number of frames in flight.
/// Public ingestion continues to use `render`, with the original segment limit.
pub fn render_bounded(input:&[u8],width:u32,step:usize,pts:i64,offset:u64,pipeline:&mut Pipeline,requested_workers:usize,frame_limit:usize)->Result<Rendered,Error> {
    let step=step.max(1);
    // Effects with temporal state must stay serial. This whitelist is intentionally narrow.
    let config=pipeline.view();
    let independent=config.iter().filter(|s|s.enabled).all(|s|s.effect_id=="ink_editions" || (s.effect_id=="living_ink" && s.params.get("trail_strength").copied().unwrap_or(0.22)==0.0));
    let count=if independent {requested_workers.clamp(1,6)} else {1};
    std::thread::scope(|scope| {
        let mut workers=Vec::new();
        if count>1 {
            for _ in 0..count {
                let mut local=Pipeline::new();
                for slot in &config {
                    let id=local.add_effect(&slot.effect_id)?;
                    local.update_params(&id,&slot.params);
                    local.set_enabled(&id,slot.enabled);
                }
                let (send,receive)=mpsc::sync_channel::<Job>(1);
                let (result,output)=mpsc::channel();
                scope.spawn(move || {
                    while let Ok(mut job)=receive.recv() {
                        let start=Instant::now();
                        local.set_dimensions(job.frame.width,job.frame.height);
                        local.process_frame_timed(&mut job.frame,job.number,job.dt);
                        if result.send(Done {frame:job.frame,milliseconds:start.elapsed().as_secs_f64()*1000.0}).is_err() {break;}
                    }
                });
                workers.push(Worker {input:send,output});
            }
        }
        // Keep cleanup inside the scope: drop channels before joining even on decode/encode failure.
        let result=(|| -> Result<Rendered,Error> {
            #[cfg(test)]
            let mut painted_frames=Vec::new();
            let mut encoder=None;
            let mut times=Vec::new(); let mut dimensions=(0,0); let mut submitted=0usize;
            let mut encode_ms=0.0; let mut callback_ms=0.0;
            let decode_start=Instant::now();
            let info=codec::decode_visit_bounded(input,width,step,frame_limit,|mut frame,fps,_,first| {
                let start=Instant::now(); let out_fps=fps/step as f64;
                if encoder.is_none() {
                    dimensions=(frame.width,frame.height);
                    if workers.is_empty() {pipeline.set_dimensions(frame.width,frame.height);}
                    if let (Some(last),Some(now))=(pipeline.source_end,first) {
                        if (now-last).abs()>2.0/fps {pipeline.reset();}
                    }
                    let t=Instant::now();
                    encoder=Some(codec::TsEncoder::new(frame.width,frame.height,out_fps,pts)?);
                    encode_ms+=t.elapsed().as_secs_f64()*1000.0;
                }
                let number=(offset+submitted as u64) as u32;
                if workers.is_empty() {
                    let t=Instant::now(); pipeline.process_frame_timed(&mut frame,number,(1.0/out_fps) as f32);
                    times.push(t.elapsed().as_secs_f64()*1000.0);
                    #[cfg(test)]
                    painted_frames.push(frame.data.clone());
                    let t=Instant::now(); encoder.as_mut().unwrap().push(&frame)?;
                    encode_ms+=t.elapsed().as_secs_f64()*1000.0;
                } else {
                    let worker=&workers[submitted%count];
                    let done=if submitted>=count {Some(worker.output.recv()?)} else {None};
                    // Refill this worker before encoding its previous result. The
                    // ordered drain is unchanged, but painting can overlap encoding.
                    worker.input.send(Job {frame,number,dt:(1.0/out_fps) as f32})?;
                    if let Some(done)=done {
                        times.push(done.milliseconds);
                        #[cfg(test)]
                        painted_frames.push(done.frame.data.clone());
                        let t=Instant::now(); encoder.as_mut().unwrap().push(&done.frame)?;
                        encode_ms+=t.elapsed().as_secs_f64()*1000.0;
                    }
                }
                submitted+=1; callback_ms+=start.elapsed().as_secs_f64()*1000.0; Ok(())
            })?;
            let decode_ms=(decode_start.elapsed().as_secs_f64()*1000.0-callback_ms).max(0.0);
            for index in submitted.saturating_sub(count)..submitted {
                if workers.is_empty() {break;}
                let done=workers[index%count].output.recv()?; times.push(done.milliseconds);
                #[cfg(test)]
                        painted_frames.push(done.frame.data.clone());
                        let t=Instant::now(); encoder.as_mut().unwrap().push(&done.frame)?;
                encode_ms+=t.elapsed().as_secs_f64()*1000.0;
            }
            let t=Instant::now(); let bytes=encoder.ok_or("No decoded frames")?.finish()?;
            encode_ms+=t.elapsed().as_secs_f64()*1000.0;
            let fps=info.fps/step as f64;
            if !workers.is_empty() {pipeline.clear_enabled_history();}
            pipeline.source_end=info.first_seconds.map(|first|first+info.frames as f64/fps);
            Ok(Rendered {#[cfg(test)] painted_frames,bytes,fps,frames:info.frames,width:dimensions.0,height:dimensions.1,first_seconds:info.first_seconds,times,decode_ms,encode_ms})
        })();
        drop(workers);
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn configured(trails:f32)->Pipeline {
        let mut p=Pipeline::new(); let id=p.add_effect("living_ink").unwrap();
        p.update_params(&id,&crate::registry::ParamValues::from([
            ("trail_strength".into(),trails),("warp_amplitude".into(),0.012),("subpixel".into(),1.0)
        ])); p
    }
    #[test]
    fn edition_parallel_output_matches_serial_output() {
        codec::init();
        let mut frames=Vec::new();
        for index in 0..9 {
            let mut frame=RawFrame::filled(64,36,35,45,55);
            for y in 8..28 {for x in (8+index)..(25+index) {frame.data[(y*64+x)*3..(y*64+x)*3+3].fill(220);}}
            frames.push(frame);
        }
        let input=codec::encode_segment(&frames,30,0).unwrap();
        for edition in [15., 17., 18., 19., 20., 21., 22., 23., 24., 25., 26., 27., 28., 29., 30., 31., 32., 33., 34., 35., 36., 37., 38., 39., 40., 41., 42., 43., 44., 45., 46., 47., 48., 49., 50., 51., 52., 53., 54., 55., 56., 57., 58., 59., 60., 61., 62., 63., 64., 65.] {
            let configured=|| {let mut p=Pipeline::new();let id=p.add_effect("ink_editions").unwrap();p.update_params(&id,&crate::registry::ParamValues::from([("edition".into(),edition)]));p};
            let serial=render(&input,64,1,0,700,&mut configured(),1).unwrap();
            for workers in [3,5,6] {
                let parallel=render(&input,64,1,0,700,&mut configured(),workers).unwrap();
                assert_eq!(serial.frames,9);assert_eq!(serial.painted_frames,parallel.painted_frames);
            }
        }
    }

    #[test]
    fn parallel_frames_match_serial_order_and_warp_phase() {
        codec::init();
        let mut frames=Vec::new();
        for i in 0..7 {
            let mut f=RawFrame::filled(64,36,20,20,20);
            for y in 0..36 {for x in 15+i..35+i {f.data[(y*64+x)*3..][..3].fill(220);}}
            frames.push(f);
        }
        let input=codec::encode_segment(&frames,30,0).unwrap();
        for trails in [0.0,0.22] {
            let serial=render(&input,64,1,90000,71,&mut configured(trails),1).unwrap();
            let parallel=render(&input,64,1,90000,71,&mut configured(trails),3).unwrap();
            assert_eq!(serial.frames,7); assert_eq!(parallel.frames,7);
            assert_eq!(serial.painted_frames,parallel.painted_frames);
        }
    }
    #[test]
    fn switching_trails_off_in_parallel_discards_old_history() {
        codec::init();
        let inputs:Vec<_>=[15,45,45].into_iter().enumerate().map(|(segment,x)| {
            let mut frame=RawFrame::filled(64,36,20,20,20);
            for y in 0..36 {for xx in x..64 {frame.data[(y*64+xx)*3..][..3].fill(220);}}
            codec::encode_segment(&vec![frame;30],30,segment as i64*30).unwrap()
        }).collect();
        let mut serial=configured(0.22); let mut parallel=configured(0.22);
        for (segment,input) in inputs.iter().enumerate() {
            let trails=if segment==1 {0.0} else {0.22};
            for p in [&mut serial,&mut parallel] {
                let id=p.view()[0].slot_id.clone();
                p.update_params(&id,&crate::registry::ParamValues::from([("trail_strength".into(),trails)]));
            }
            let a=render(input,64,1,segment as i64*90000,segment as u64*30,&mut serial,1).unwrap();
            let b=render(input,64,1,segment as i64*90000,segment as u64*30,&mut parallel,3).unwrap();
            assert_eq!(a.painted_frames,b.painted_frames,"segment {segment}");
        }
    }
    #[test]
    fn empty_invalid_input_returns_without_worker_deadlock() {
        codec::init();
        assert!(render(&[],64,1,0,0,&mut configured(0.0),3).is_err());
    }
}
