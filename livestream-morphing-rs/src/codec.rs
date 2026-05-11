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

/// Decode a .ts segment from raw bytes into RGB24 frames.
pub fn decode_segment(
    ts_bytes: &[u8],
) -> Result<Vec<RawFrame>, Box<dyn std::error::Error + Send + Sync>> {
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

    let decoder_ctx = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
    let mut decoder = decoder_ctx.decoder().video()?;

    let mut scaler = ScalingContext::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::RGB24,
        decoder.width(),
        decoder.height(),
        Flags::BILINEAR,
    )?;

    let width = decoder.width();
    let height = decoder.height();
    let mut frames = Vec::new();

    let mut receive_frames =
        |decoder: &mut ffmpeg::decoder::Video| -> Result<(), ffmpeg::Error> {
            let mut decoded = Video::empty();
            while decoder.receive_frame(&mut decoded).is_ok() {
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
    Ok(frames)
}

/// Encode RGB24 frames into an H.264 MPEG-TS segment.
/// `pts_offset` is the starting PTS for this segment (in frame units) to ensure
/// continuous timestamps across segments for seamless HLS playback.
pub fn encode_segment(
    frames: &[RawFrame],
    fps: u32,
    pts_offset: i64,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
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
    encoder.set_frame_rate(Some(ffmpeg::Rational(fps as i32, 1)));
    encoder.set_time_base(ffmpeg::Rational(1, fps as i32));

    if needs_global_header {
        encoder.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
    }

    let mut x264_opts = ffmpeg::Dictionary::new();
    x264_opts.set("preset", "ultrafast");
    x264_opts.set("crf", "25");

    let mut encoder = encoder.open_with(x264_opts)?;
    ost.set_parameters(&encoder);

    octx.write_header()?;

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
        yuv.set_pts(Some(pts_offset + i as i64));

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
