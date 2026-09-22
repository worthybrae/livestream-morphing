// ffmpeg-next decode/encode

extern crate ffmpeg_next as ffmpeg;

use ffmpeg::format::{input, Pixel};
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context as ScalingContext, flag::Flags};
use ffmpeg::util::frame::video::Video;
use std::io::Write;
use tempfile::NamedTempFile;

use crate::effects::RawFrame;

/// Initialize FFmpeg. Call once at program start.
pub fn init() {
    ffmpeg::init().expect("Failed to initialize FFmpeg");
}

/// Decode a .ts segment into RGB24 frames bounded to 1280x720.
/// Production uses decode_sized to choose its output dimensions directly.
pub fn decode_segment(
    ts_bytes: &[u8],
) -> Result<Vec<RawFrame>, Box<dyn std::error::Error + Send + Sync>> {
    Ok(decode_sized(ts_bytes, 1280)?.frames)
}

pub struct Decoded {
    pub frames: Vec<RawFrame>,
    pub fps: f64,
    pub first_seconds: Option<f64>,
}

pub fn decode_sized(ts_bytes: &[u8], max_width: u32) -> Result<Decoded, Box<dyn std::error::Error + Send + Sync>> {
    decode_sampled(ts_bytes, max_width, 1)
}

/// Keep every `step`th source frame before RGB conversion/allocation.
/// FPS remains the source cadence; callers must account for selection in timing.
pub fn decode_sampled(ts_bytes: &[u8], max_width: u32, step: usize) -> Result<Decoded, Box<dyn std::error::Error + Send + Sync>> {
    let mut frames=Vec::new();
    let info=decode_visit(ts_bytes,max_width,step,|frame,_,_,_| { frames.push(frame); Ok(()) })?;
    Ok(Decoded {frames,fps:info.fps,first_seconds:info.first_seconds})
}

pub struct DecodeInfo { pub fps: f64, pub first_seconds: Option<f64>, pub frames: usize }
pub fn decode_visit<F>(ts_bytes: &[u8], max_width: u32, step: usize, visit: F) -> Result<DecodeInfo, Box<dyn std::error::Error + Send + Sync>>
where F: FnMut(RawFrame,f64,usize,Option<f64>)->Result<(),Box<dyn std::error::Error + Send + Sync>> {
    decode_visit_bounded(ts_bytes,max_width,step,720,visit)
}

pub fn decode_visit_bounded<F>(ts_bytes: &[u8], max_width: u32, step: usize, frame_limit: usize, mut visit: F) -> Result<DecodeInfo, Box<dyn std::error::Error + Send + Sync>>
where F: FnMut(RawFrame,f64,usize,Option<f64>)->Result<(),Box<dyn std::error::Error + Send + Sync>> {
    decode_visit_lazy(ts_bytes,max_width,step,frame_limit,|convert,_,fps,index,first|visit(convert()?,fps,index,first))
}

/// Visit the full timeline, converting to RGB only when the caller needs paint.
/// Held-canvas output still gets every source timestamp and callback.
pub fn decode_visit_lazy<F>(ts_bytes: &[u8], max_width: u32, step: usize, frame_limit: usize, mut visit: F) -> Result<DecodeInfo, Box<dyn std::error::Error + Send + Sync>>
where F: FnMut(&mut dyn FnMut()->Result<RawFrame,Box<dyn std::error::Error + Send + Sync>>,(u32,u32),f64,usize,Option<f64>)->Result<(),Box<dyn std::error::Error + Send + Sync>> {
    let step = step.max(1);
    // Write to temp file (ffmpeg-next requires file path for input)
    let mut tmp = NamedTempFile::new()?;
    tmp.write_all(ts_bytes)?;
    tmp.flush()?;

    let mut ictx = input(tmp.path())?;
    let stream = ictx
        .streams()
        .best(Type::Video)
        .ok_or("No video stream found")?;
    let stream_index = stream.index();
    let time_base = f64::from(stream.time_base());
    let rate = stream.avg_frame_rate();
    let rate = if rate.numerator() > 0 && rate.denominator() > 0 { rate } else { stream.rate() };
    let fps = f64::from(rate);
    if !fps.is_finite() || fps <= 0.0 || fps > 120.0 { return Err("Invalid source frame rate".into()); }

    let decoder_ctx = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
    let mut decoder = decoder_ctx.decoder().video()?;

    let (source_width, source_height) = (decoder.width(), decoder.height());
    let max_width=max_width.max(4);
    let max_height=(max_width*9/16).max(4);
    let scale=(max_width as f64/source_width as f64).min(max_height as f64/source_height as f64).min(1.0);
    let width=((source_width as f64*scale) as u32/2*2).max(2);
    let height=((source_height as f64*scale) as u32/2*2).max(2);

    let mut scaler = ScalingContext::get(
        decoder.format(),
        source_width,
        source_height,
        Pixel::RGB24,
        width,
        height,
        Flags::BILINEAR,
    )?;

    let mut selected_frames=0;
    let mut first_seconds = None;
    let mut source_frames = 0usize;

    let mut receive_frames =
        |decoder: &mut ffmpeg::decoder::Video| -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let mut decoded = Video::empty();
            while decoder.receive_frame(&mut decoded).is_ok() {
                if source_frames >= frame_limit { return Err(ffmpeg::Error::InvalidData.into()); }
                if first_seconds.is_none() { first_seconds = decoded.timestamp().map(|pts|pts as f64*time_base); }
                let selected = source_frames % step == 0;
                source_frames += 1;
                if !selected { continue; }
                let mut convert=||->Result<RawFrame,Box<dyn std::error::Error + Send + Sync>> {
                    let mut rgb = Video::empty();
                    scaler.run(&decoded, &mut rgb)?;

                // Copy RGB data, accounting for stride alignment
                let stride = rgb.stride(0);
                let row_bytes = (width * 3) as usize;
                let mut data = Vec::with_capacity((width * height * 3) as usize);
                for y in 0..height as usize {
                    let offset = y * stride;
                    data.extend_from_slice(&rgb.data(0)[offset..offset + row_bytes]);
                }

                    Ok(RawFrame { data, width, height })
                };
                visit(&mut convert,(width,height),fps,source_frames-1,first_seconds)?;
                selected_frames+=1;
            }
            Ok(())
        };

    for (stream, packet) in ictx.packets() {
        if stream.index() == stream_index {
            decoder.send_packet(&packet)?;
            receive_frames(&mut decoder)?;
        }
    }
    decoder.send_eof()?;
    receive_frames(&mut decoder)?;

    tracing::info!(frame_count = selected_frames, width, height, "Decoded segment");
    Ok(DecodeInfo { frames:selected_frames, fps, first_seconds })
}

/// Encode RGB24 frames into an H.264 MPEG-TS segment.
/// `pts_offset` is the starting PTS for this segment (in frame units) to ensure
/// continuous timestamps across segments for seamless HLS playback.
/// Every segment shares a one-second timestamp origin to accommodate B-frame
/// decode delay without independently shifting the first segment's timestamps.
pub fn encode_segment(
    frames: &[RawFrame],
    fps: u32,
    pts_offset: i64,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    encode_timed(frames, fps as f64, pts_offset * 90000 / fps as i64)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EncoderOptions {
    gop_frames: Option<u32>,
}

impl EncoderOptions {
    pub fn from_value(value: Option<&str>, fps: f64) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self { gop_frames: None });
        };
        let seconds = value
            .parse::<f64>()
            .map_err(|_| "ENCODE_GOP_SECONDS must be numeric".to_string())?;
        if !seconds.is_finite() || seconds <= 0.0 || seconds > 10.0 {
            return Err("ENCODE_GOP_SECONDS must be in (0, 10]".into());
        }
        Ok(Self {
            gop_frames: Some((seconds * fps).round().max(1.0) as u32),
        })
    }
}

pub fn encode_timed(frames: &[RawFrame], fps: f64, pts_offset: i64) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    if frames.is_empty() {
        return Err("No frames to encode".into());
    }

    let mut encoder = TsEncoder::new(frames[0].width, frames[0].height, fps, pts_offset)?;
    for frame in frames { encoder.push(frame)?; }
    encoder.finish()
}

fn encode_timed_with_options(
    frames: &[RawFrame],
    fps: f64,
    pts_offset: i64,
    options: EncoderOptions,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    if frames.is_empty() {
        return Err("No frames to encode".into());
    }

    let mut encoder = TsEncoder::new_with_options(
        frames[0].width,
        frames[0].height,
        fps,
        pts_offset,
        options,
    )?;
    for frame in frames { encoder.push(frame)?; }
    encoder.finish()
}

pub struct TsEncoder {
    tmp: NamedTempFile,
    octx: ffmpeg::format::context::Output,
    encoder: ffmpeg::encoder::video::Encoder,
    scaler: ScalingContext,
    rgb: Video,
    width:u32, height:u32, fps:f64, pts_offset:i64, index:usize,
}
impl TsEncoder {
    pub fn new(width:u32,height:u32,fps:f64,pts_offset:i64)->Result<Self,Box<dyn std::error::Error + Send + Sync>> {
    let options = EncoderOptions::from_value(std::env::var("ENCODE_GOP_SECONDS").ok().as_deref(), fps)?;
    Self::new_with_options(width, height, fps, pts_offset, options)
    }

    pub fn new_with_options(width:u32,height:u32,fps:f64,pts_offset:i64,options: EncoderOptions)->Result<Self,Box<dyn std::error::Error + Send + Sync>> {
    let tmp = NamedTempFile::new()?;
    let tmp_path = tmp.path().to_owned();

    // Create output context
    let mut octx = ffmpeg::format::output_as(&tmp_path, "mpegts")?;

    // Find H.264 encoder
    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::H264)
        .ok_or("H264 encoder not found — install libx264")?;

    // Check format flags before add_stream borrows octx mutably
    let needs_global_header = octx
        .format()
        .flags()
        .contains(ffmpeg::format::Flags::GLOBAL_HEADER);

    let mut ost = octx.add_stream(codec)?;

    // Configure encoder
    let mut encoder = ffmpeg::codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()?;

    encoder.set_width(width);
    encoder.set_height(height);
    encoder.set_format(Pixel::YUV420P);
    encoder.set_frame_rate(Some(ffmpeg::Rational((fps * 1000.0).round() as i32, 1000)));
    // Use 90kHz time_base to match MPEG-TS native clock
    encoder.set_time_base(ffmpeg::Rational(1, 90000));
    if let Some(frames) = options.gop_frames {
        encoder.set_gop(frames);
    }

    if needs_global_header {
        encoder.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
    }

    let mut x264_opts = ffmpeg::Dictionary::new();
    x264_opts.set("preset", "veryfast");
    let crf=std::env::var("ENCODE_CRF").ok().and_then(|value|value.parse::<u8>().ok()).unwrap_or(30).clamp(18,40);
    x264_opts.set("crf", &crf.to_string());
    let threads=std::env::var("ENCODE_THREADS").ok().and_then(|v|v.parse::<usize>().ok()).unwrap_or(2).clamp(1,4);
    x264_opts.set("threads",&threads.to_string());
    if let Some(frames) = options.gop_frames {
        let frames = frames.to_string();
        x264_opts.set("keyint", &frames);
        x264_opts.set("min-keyint", &frames);
        x264_opts.set("scenecut", "0");
    }

    let encoder = encoder.open_with(x264_opts)?;
    ost.set_parameters(&encoder);

    let mut muxer_opts = ffmpeg::Dictionary::new();
    // Preserve the shared timeline instead of letting each independent muxer
    // normalize negative DTS or add its own MPEG-TS timestamp offset.
    muxer_opts.set("avoid_negative_ts", "disabled");
    muxer_opts.set("mpegts_copyts", "1");
    octx.write_header_with(muxer_opts)?;

    // Scaler: RGB24 → YUV420P
    let scaler = ScalingContext::get(
        Pixel::RGB24,
        width,
        height,
        Pixel::YUV420P,
        width,
        height,
        Flags::BILINEAR,
    )?;

    Ok(Self {tmp,octx,encoder,scaler,rgb:Video::new(Pixel::RGB24,width,height),width,height,fps,pts_offset,index:0})
    }

    pub fn push(&mut self, frame:&RawFrame)->Result<(),Box<dyn std::error::Error + Send + Sync>> {
        let width=self.width; let height=self.height;
        if (frame.width,frame.height)!=(width,height) { return Err("Encoder frame dimensions changed".into()); }
        let row_bytes=(width*3) as usize;
        let rgb=&mut self.rgb;
        // Copy data accounting for stride
        let stride = rgb.stride(0);
        for y in 0..height as usize {
            let src_offset = y * row_bytes;
            let dst_offset = y * stride;
            rgb.data_mut(0)[dst_offset..dst_offset + row_bytes]
                .copy_from_slice(&frame.data[src_offset..src_offset + row_bytes]);
        }

        let mut yuv = Video::empty();
        self.scaler.run(rgb, &mut yuv)?;
        // PTS in 90kHz ticks: each frame = 90000/fps ticks apart
        let frame_ticks = (self.index as f64 * 90000.0 / self.fps).round() as i64;
        yuv.set_pts(Some(90000 + self.pts_offset + frame_ticks));

        self.encoder.send_frame(&yuv)?;

        let mut encoded_packet = ffmpeg::Packet::empty();
        while self.encoder.receive_packet(&mut encoded_packet).is_ok() {
            encoded_packet.set_stream(0);
            encoded_packet.write_interleaved(&mut self.octx)?;
        }
        self.index+=1;
        Ok(())
    }
    pub fn finish(mut self)->Result<Vec<u8>,Box<dyn std::error::Error + Send + Sync>> {
    // Flush encoder
    self.encoder.send_eof()?;
    let mut encoded_packet = ffmpeg::Packet::empty();
    while self.encoder.receive_packet(&mut encoded_packet).is_ok() {
        encoded_packet.set_stream(0);
        encoded_packet.write_interleaved(&mut self.octx)?;
    }

    self.octx.write_trailer()?;
    drop(self.octx); // Close output before reading

    let output_bytes = std::fs::read(self.tmp.path())?;
    tracing::info!(
        frame_count = self.index,
        size_kb = output_bytes.len() / 1024,
        "Encoded segment"
    );
    Ok(output_bytes)
}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twitch_gop_is_opt_in_and_uses_source_fps() {
        assert_eq!(
            EncoderOptions::from_value(None, 30000.0 / 1001.0)
                .unwrap()
                .gop_frames,
            None
        );
        assert_eq!(
            EncoderOptions::from_value(Some("2"), 30000.0 / 1001.0)
                .unwrap()
                .gop_frames,
            Some(60)
        );
        assert!(EncoderOptions::from_value(Some("0"), 30.0).is_err());
        assert!(EncoderOptions::from_value(Some("not-a-number"), 30.0).is_err());
    }

    fn video_keyframe_indices(bytes: &[u8]) -> Vec<usize> {
        let mut tmp = NamedTempFile::new().expect("temporary segment");
        tmp.write_all(bytes).expect("write segment");
        tmp.flush().expect("flush segment");

        let mut demuxer = input(tmp.path()).expect("demux segment");
        let video_index = demuxer
            .streams()
            .best(Type::Video)
            .expect("video stream")
            .index();
        let mut packets: Vec<_> = demuxer
            .packets()
            .filter(|(stream, _)| stream.index() == video_index)
            .map(|(_, packet)| {
                (
                    packet.pts().expect("packet PTS"),
                    packet.flags().contains(ffmpeg::packet::Flags::KEY),
                )
            })
            .collect();
        packets.sort_unstable_by_key(|(pts, _)| *pts);
        packets
            .iter()
            .enumerate()
            .filter_map(|(index, (_, is_key))| is_key.then_some(index))
            .collect()
    }

    #[test]
    fn configured_two_second_gop_marks_each_interval_as_key() {
        init();
        let frames: Vec<_> = (0..180)
            .map(|i| RawFrame::filled(64, 48, i as u8, 80, 160))
            .collect();
        let bytes = encode_timed_with_options(
            &frames,
            30000.0 / 1001.0,
            0,
            EncoderOptions {
                gop_frames: Some(60),
            },
        )
        .unwrap();
        let keys = video_keyframe_indices(&bytes);
        assert!(keys.contains(&0));
        assert!(
            keys.windows(2).all(|pair| pair[1] - pair[0] <= 60),
            "keyframes: {keys:?}"
        );
        assert!(
            keys.last().is_some_and(|last| 179 - *last <= 60),
            "keyframes: {keys:?}"
        );
    }

    #[test]
    fn long_local_exports_do_not_relax_the_live_segment_limit() {
        init();
        let frames=vec![RawFrame::filled(32,18,80,90,100);725];
        let bytes=encode_segment(&frames,30,0).unwrap();
        assert!(decode_visit(&bytes,32,1,|_,_,_,_|Ok(())).is_err());
        let local=decode_visit_bounded(&bytes,32,1,18000,|_,_,_,_|Ok(())).unwrap();
        assert_eq!(local.frames,725);
    }

    #[test]
    fn independently_muxed_segments_keep_continuous_timestamps() {
        init();
        let frames: Vec<_> = (0..180)
            .map(|i| RawFrame::filled(64, 48, (i % 256) as u8, 80, 160))
            .collect();
        let segments: Vec<Vec<(i64, i64)>> = (0..3)
            .map(|segment| {
                let bytes = encode_segment(&frames, 30, segment * 180).expect("encode segment");
                let mut tmp = NamedTempFile::new().expect("temporary segment");
                tmp.write_all(&bytes).expect("write segment");
                tmp.flush().expect("flush segment");
                let mut demuxer = input(tmp.path()).expect("demux segment");
                let stream = demuxer.streams().best(Type::Video).expect("video stream");
                assert_eq!(stream.time_base(), ffmpeg::Rational(1, 90000));
                let video_index = stream.index();
                demuxer
                    .packets()
                    .filter(|(stream, _)| stream.index() == video_index)
                    .map(|(_, packet)| {
                        (
                            packet.pts().expect("packet PTS"),
                            packet.dts().expect("packet DTS"),
                        )
                    })
                    .collect()
            })
            .collect();

        let mut previous_pts_end = None;
        let mut previous_dts_end = None;
        for packets in segments {
            assert_eq!(packets.len(), 180, "every frame must survive muxing");
            let mut presentation_times: Vec<_> = packets.iter().map(|p| p.0).collect();
            presentation_times.sort_unstable(); // B-frames reorder packet PTS, not presentation.
            assert!(presentation_times.windows(2).all(|p| p[1] - p[0] == 3000));
            assert!(packets.windows(2).all(|p| p[1].1 - p[0].1 == 3000));
            let pts_start = presentation_times[0];
            let dts_start = packets[0].1;
            assert!(
                dts_start >= 0,
                "the shared timestamp origin must accommodate B-frame delay"
            );
            if let Some(end) = previous_pts_end {
                assert_eq!(pts_start - end, 3000, "PTS boundary must not overlap or gap");
            }
            if let Some(end) = previous_dts_end {
                assert_eq!(dts_start - end, 3000, "DTS boundary must not overlap or gap");
            }
            previous_pts_end = presentation_times.last().copied();
            previous_dts_end = packets.last().map(|p| p.1);
        }
    }

    #[test]
    fn decode_caps_rgb_memory_without_stretching_frames() {
        init();

        for (width, height, expected_width, expected_height) in [
            (1920, 1080, 1280, 720),
            (1600, 1200, 960, 720),
            (1080, 1920, 404, 720),
        ] {
            let frames = vec![RawFrame::filled(width, height, 90, 130, 170); 3];
            let encoded = encode_segment(&frames, 30, 0).expect("encode fixture");
            let decoded = decode_segment(&encoded).expect("decode fixture");

            assert_eq!(decoded.len(), 3, "resizing must preserve frame cadence");
            for frame in decoded {
                assert_eq!(
                    (frame.width, frame.height),
                    (expected_width, expected_height)
                );
                assert_eq!(
                    frame.data.len(),
                    (expected_width * expected_height * 3) as usize
                );
                // The pipeline halves RGB dimensions before YUV420/H.264 encoding.
                assert_eq!(frame.width % 4, 0);
                assert_eq!(frame.height % 4, 0);
            }
        }
    }

    #[test]
    fn incremental_decode_propagates_consumer_failure() {
        init();
        let frames=vec![RawFrame::filled(64,36,90,90,90);5];
        let bytes=encode_segment(&frames,30,0).unwrap();
        let mut seen=0;
        let error=decode_visit(&bytes,64,1,|_,fps,index,first| {
            assert_eq!(fps,30.0); assert_eq!(index,seen); assert!(first.is_some());
            seen+=1;
            if seen==2 { return Err("stop consumer".into()); }
            Ok(())
        }).err().unwrap();
        assert_eq!(seen,2);
        assert_eq!(error.to_string(),"stop consumer");
    }
    #[test]
    fn sampled_decode_preserves_selected_pixels_and_source_timing() {
        init();
        let frames: Vec<_> = (0..7).map(|i| RawFrame::filled(64,36,i*30,80,160)).collect();
        let bytes = encode_segment(&frames,30,120).unwrap();
        let full = decode_sized(&bytes,64).unwrap();
        let selected = decode_sampled(&bytes,64,2).unwrap();
        assert_eq!(selected.frames.len(),4);
        assert_eq!(selected.fps,full.fps);
        assert_eq!(selected.first_seconds,full.first_seconds);
        for (actual,expected) in selected.frames.iter().zip(full.frames.iter().step_by(2)) {
            assert_eq!(actual.data,expected.data);
        }
    }

    #[test]
    fn roundtrip_encode_decode() {
        init();

        // Create 5 simple test frames (solid colors)
        let frames: Vec<RawFrame> = (0..5)
            .map(|i| {
                let val = (i * 50) as u8;
                RawFrame::filled(64, 48, val, val, val)
            })
            .collect();

        // Encode
        let ts_bytes = encode_segment(&frames, 30, 0).expect("encode failed");
        assert!(!ts_bytes.is_empty(), "Encoded bytes should not be empty");

        // Decode
        let decoded = decode_segment(&ts_bytes).expect("decode failed");
        assert_eq!(decoded.len(), 5, "Should get back 5 frames");
        assert_eq!(decoded[0].width, 64);
        assert_eq!(decoded[0].height, 48);
    }
}
