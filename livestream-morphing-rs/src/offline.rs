//! Local replay uses the exact production Effect implementation, with no browser artwork.
use crate::{codec, pipeline::Pipeline};
use std::time::Instant;

pub fn render(args: &[String]) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    if args.len()<3 { return Err("usage: morphing render INPUT OUTPUT WIDTH [legacy] [PRESET.json]".into()); }
    let input=std::fs::read(&args[0])?;
    let width=args[2].parse::<u32>()?.clamp(64,1920);
    // Local comparison only; production cadence is unchanged.
    let step=std::env::var("RENDER_FRAME_STEP").ok().and_then(|v|v.parse::<usize>().ok()).unwrap_or(1).clamp(1,3);
    let start=Instant::now();
    let decoded=codec::decode_sampled(&input,width,step)?;
    let decode_ms=start.elapsed().as_secs_f64()*1000.0;
    let output_fps=decoded.fps/step as f64;
    let mut frames=decoded.frames;
    if frames.is_empty() { return Err("No decoded frames".into()); }
    let mut pipeline=Pipeline::new();
    if args.get(3).map(|s| s.as_str())==Some("legacy") {
        for effect in ["distortion","grayscale","quantize","edges","canvas_texture"] { pipeline.add_effect(effect)?; }
    } else {
        let slot=pipeline.add_effect("living_ink")?;
        if let Some(path)=args.get(4) {
            let params=serde_json::from_slice(&std::fs::read(path)?)?;
            pipeline.update_params(&slot,&params);
        }
    }
    pipeline.set_dimensions(frames[0].width,frames[0].height);
    let mut timings=Vec::new();
    for (i,frame) in frames.iter_mut().enumerate() {
        let t=Instant::now();
        pipeline.process_frame_timed(frame,i as u32,(1.0/output_fps) as f32);
        timings.push(t.elapsed().as_secs_f64()*1000.0);
    }
    let effects_ms: f64=timings.iter().sum();
    timings.sort_by(f64::total_cmp);
    let t=Instant::now();
    let encoded=codec::encode_timed(&frames,output_fps,0)?;
    let encode_ms=t.elapsed().as_secs_f64()*1000.0;
    let total_ms=start.elapsed().as_secs_f64()*1000.0;
    std::fs::write(&args[1],&encoded)?;
    let peak_rss_kib=std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|l|l.starts_with("VmHWM:")).and_then(|l|l.split_whitespace().nth(1)).and_then(|v|v.parse::<u64>().ok()));
    Ok(serde_json::json!({"width":frames[0].width,"height":frames[0].height,
        "source_fps":decoded.fps,"output_fps":output_fps,"frames":frames.len(),"decode_ms":decode_ms,"effects_ms":effects_ms,
        "frame_p50_ms":timings[timings.len()/2],"frame_p95_ms":timings[(timings.len()*95/100).min(timings.len()-1)],
        "encode_ms":encode_ms,"total_ms":total_ms,"processing_fps":frames.len() as f64*1000.0/total_ms,
        "duration_seconds":frames.len() as f64/output_fps,"bytes":encoded.len(),"peak_rss_kib":peak_rss_kib}))
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("{}", render(args)?);
    Ok(())
}
