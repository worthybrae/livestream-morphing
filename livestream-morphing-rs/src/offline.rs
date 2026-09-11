//! Local replay uses the exact production Effect implementation, with no browser artwork.
use crate::pipeline::Pipeline;
use std::time::Instant;

pub fn render(args: &[String]) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    if args.len()<3 { return Err("usage: morphing render INPUT OUTPUT WIDTH [ink|legacy|psychedelic|edition] [PRESET.json]".into()); }
    let input=std::fs::read(&args[0])?;
    let width=args[2].parse::<u32>()?.clamp(64,1920);
    // Local comparison only; production cadence is unchanged.
    let step=std::env::var("RENDER_FRAME_STEP").ok().and_then(|v|v.parse::<usize>().ok()).unwrap_or(1).clamp(1,3);
    let start=Instant::now();
    let mut pipeline=Pipeline::new();
    if args.get(3).map(|s| s.as_str())==Some("legacy") {
        for effect in ["distortion","grayscale","quantize","edges","canvas_texture"] { pipeline.add_effect(effect)?; }
    } else {
        if args.get(3).map(|s|s.as_str())==Some("psychedelic") {
            let warp=pipeline.add_effect("distortion")?;
            let params=serde_json::from_str(r#"{"amplitude":0.012,"frequency":8,"cycle_length":240}"#)?;
            pipeline.update_params(&warp,&params);
        }
        let slot=pipeline.add_effect(if args.get(3).map(|s|s.as_str())==Some("edition") {"ink_editions"} else {"living_ink"})?;
        if let Some(path)=args.get(4) {
            let params=serde_json::from_slice(&std::fs::read(path)?)?;
            pipeline.update_params(&slot,&params);
        }
    }
    let workers=std::env::var("RENDER_WORKERS").ok().and_then(|s|s.parse().ok()).unwrap_or(3);
    let rendered=crate::renderer::render_bounded(&input,width,step,0,0,&mut pipeline,workers,18000)?;
    let mut timings=rendered.times;
    let effects_ms: f64=timings.iter().sum();
    timings.sort_by(f64::total_cmp);
    let decode_ms=rendered.decode_ms; let encode_ms=rendered.encode_ms;
    let output_fps=rendered.fps; let dimensions=(rendered.width,rendered.height);
    let encoded=rendered.bytes;
    let total_ms=start.elapsed().as_secs_f64()*1000.0;
    std::fs::write(&args[1],&encoded)?;
    let peak_rss_kib=std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|l|l.starts_with("VmHWM:")).and_then(|l|l.split_whitespace().nth(1)).and_then(|v|v.parse::<u64>().ok()));
    Ok(serde_json::json!({"width":dimensions.0,"height":dimensions.1,
        "source_fps":output_fps*step as f64,"paint_fps":rendered.paint_fps,"painted_targets":timings.len(),"output_fps":output_fps,"frames":rendered.frames,"decode_ms":decode_ms,"effects_ms":effects_ms,
        "frame_p50_ms":timings[timings.len()/2],"frame_p95_ms":timings[(timings.len()*95/100).min(timings.len()-1)],
        "encode_ms":encode_ms,"post_ms":rendered.post_ms,"total_ms":total_ms,"processing_fps":rendered.frames as f64*1000.0/total_ms,
        "duration_seconds":rendered.frames as f64/output_fps,"bytes":encoded.len(),"peak_rss_kib":peak_rss_kib}))
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("{}", render(args)?);
    Ok(())
}
