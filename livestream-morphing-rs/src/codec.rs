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
/// The pipeline halves these frames again, producing 640x360 for a 16:9 source.
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

    let mut frames = Vec::new();
    let mut first_seconds = None;

    let mut receive_frames =
        |decoder: &mut ffmpeg::decoder::Video| -> Result<(), ffmpeg::Error> {
            let mut decoded = Video::empty();
            while decoder.receive_frame(&mut decoded).is_ok() {
                if frames.len() >= 720 { return Err(ffmpeg::Error::InvalidData); }
                if first_seconds.is_none() { first_seconds = decoded.timestamp().map(|pts|pts as f64*time_base); }
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

                frames.push(RawFrame { data, width, height });
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

    tracing::info!(frame_count = frames.len(), width, height, "Decoded segment");
    Ok(Decoded { frames, fps, first_seconds })
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

pub fn encode_timed(frames: &[RawFrame], fps: f64, pts_offset: i64) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    if frames.is_empty() {
        return Err("No frames to encode".into());
    }

    let width = frames[0].width;
    let height = frames[0].height;

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

    if needs_global_header {
        encoder.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
    }

    let mut x264_opts = ffmpeg::Dictionary::new();
    x264_opts.set("preset", "veryfast");
    x264_opts.set("crf", "30");

    let mut encoder = encoder.open_with(x264_opts)?;
    ost.set_parameters(&encoder);

    let mut muxer_opts = ffmpeg::Dictionary::new();
    // Preserve the shared timeline instead of letting each independent muxer
    // normalize negative DTS or add its own MPEG-TS timestamp offset.
    muxer_opts.set("avoid_negative_ts", "disabled");
    muxer_opts.set("mpegts_copyts", "1");
    octx.write_header_with(muxer_opts)?;

    // Scaler: RGB24 → YUV420P
    let mut scaler = ScalingContext::get(
        Pixel::RGB24,
        width,
        height,
        Pixel::YUV420P,
        width,
        height,
        Flags::BILINEAR,
    )?;

    let row_bytes = (width * 3) as usize;

    for (i, frame) in frames.iter().enumerate() {
        let mut rgb = Video::new(Pixel::RGB24, width, height);
        // Copy data accounting for stride
        let stride = rgb.stride(0);
        for y in 0..height as usize {
            let src_offset = y * row_bytes;
            let dst_offset = y * stride;
            rgb.data_mut(0)[dst_offset..dst_offset + row_bytes]
                .copy_from_slice(&frame.data[src_offset..src_offset + row_bytes]);
        }

        let mut yuv = Video::empty();
        scaler.run(&rgb, &mut yuv)?;
        // PTS in 90kHz ticks: each frame = 90000/fps ticks apart
        let frame_ticks = (i as f64 * 90000.0 / fps).round() as i64;
        yuv.set_pts(Some(90000 + pts_offset + frame_ticks));

        encoder.send_frame(&yuv)?;

        let mut encoded_packet = ffmpeg::Packet::empty();
        while encoder.receive_packet(&mut encoded_packet).is_ok() {
            encoded_packet.set_stream(0);
            encoded_packet.write_interleaved(&mut octx)?;
        }
    }

    // Flush encoder
    encoder.send_eof()?;
    let mut encoded_packet = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut encoded_packet).is_ok() {
        encoded_packet.set_stream(0);
        encoded_packet.write_interleaved(&mut octx)?;
    }

    octx.write_trailer()?;
    drop(octx); // Close output before reading

    let output_bytes = std::fs::read(&tmp_path)?;
    tracing::info!(
        frame_count = frames.len(),
        size_kb = output_bytes.len() / 1024,
        "Encoded segment"
    );
    Ok(output_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

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
