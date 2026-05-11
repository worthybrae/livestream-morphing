use std::sync::Arc;
use tokio::sync::{watch, RwLock};

use crate::codec;
use crate::effects::{downsample_2x, upsample_2x, FrameProcessor};
use crate::hls::HlsBuffer;
use crate::stream_source::StreamSource;

/// Shared state between the pipeline and HTTP server.
pub struct AppState {
    pub hls_buffer: RwLock<HlsBuffer>,
    pub pipeline_active: watch::Sender<bool>,
    pub last_client_request: std::sync::atomic::AtomicU64,
}

impl AppState {
    pub fn new() -> (Arc<Self>, watch::Receiver<bool>) {
        let (tx, rx) = watch::channel(false);
        let state = Arc::new(Self {
            hls_buffer: RwLock::new(HlsBuffer::new(10)),
            pipeline_active: tx,
            last_client_request: std::sync::atomic::AtomicU64::new(0),
        });
        (state, rx)
    }

    /// Record a client request and activate the pipeline if needed.
    pub fn touch(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_client_request
            .store(now, std::sync::atomic::Ordering::Relaxed);
        let _ = self.pipeline_active.send(true);
    }

    /// Seconds since last client request.
    pub fn idle_seconds(&self) -> u64 {
        let last = self
            .last_client_request
            .load(std::sync::atomic::Ordering::Relaxed);
        if last == 0 {
            return u64::MAX;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now.saturating_sub(last)
    }
}

/// Main pipeline loop. Runs until the shutdown receiver signals.
pub async fn run(state: Arc<AppState>, mut active: watch::Receiver<bool>) {
    tracing::info!("Pipeline waiting for first client...");

    // Wait for first client to activate
    loop {
        if *active.borrow_and_update() {
            break;
        }
        if active.changed().await.is_err() {
            return;
        }
    }

    tracing::info!("Pipeline activated!");
    let mut source = StreamSource::new();
    let mut frame_counter: i64 = 0;
    let idle_timeout = std::time::Duration::from_secs(300); // 5 minutes

    loop {
        // Check idle timeout
        if state.idle_seconds() > idle_timeout.as_secs() {
            tracing::info!("No clients for 5 minutes, pipeline going idle");
            let _ = state.pipeline_active.send(false);
            state.hls_buffer.write().await.clear();

            // Wait for reactivation
            loop {
                if *active.borrow_and_update() {
                    break;
                }
                if active.changed().await.is_err() {
                    return;
                }
            }
            tracing::info!("Pipeline reactivated!");
            source = StreamSource::new();
            frame_counter = 0;
        }

        // Fetch latest segment
        let segment_id = match source.fetch_latest_segment_id().await {
            Some(id) => id,
            None => {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        // Download
        let ts_bytes = match source.download_segment(&segment_id).await {
            Some(bytes) => bytes,
            None => {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        // Process in blocking thread (CPU-bound)
        let seg_id = segment_id.clone();
        let pts_offset = frame_counter;
        let processed = tokio::task::spawn_blocking(
            move || -> Result<(Vec<u8>, i64), Box<dyn std::error::Error + Send + Sync>> {
                let t0 = std::time::Instant::now();

                let frames = codec::decode_segment(&ts_bytes)?;
                let t_decode = t0.elapsed();

                if frames.is_empty() {
                    return Err("No frames decoded".into());
                }

                let num_frames = frames.len() as i64;

                // Downsample to half res for processing + output
                let mut half_frames: Vec<_> = frames.iter().map(|f| downsample_2x(f)).collect();
                let half_w = half_frames[0].width;
                let half_h = half_frames[0].height;
                let t_downsample = t0.elapsed();

                // Process every 3rd frame, duplicate for the skipped ones
                let mut processor = FrameProcessor::new(half_w, half_h);
                let (edge_color, _bg) = crate::time_color::get_colors_now();
                processor.edge_darkness = if edge_color == (0, 0, 0) { 100 } else { 40 };

                let frame_skip = 3;
                for i in (0..half_frames.len()).step_by(frame_skip) {
                    processor.process_frame(&mut half_frames[i], i as u32);
                    // Copy processed frame to skipped neighbors
                    for j in 1..frame_skip {
                        if i + j < half_frames.len() {
                            half_frames[i + j].data = half_frames[i].data.clone();
                        }
                    }
                }
                let t_effects = t0.elapsed();

                // Encode at half resolution (960x540) — no upsample needed
                let encoded = codec::encode_segment(&half_frames, 30, pts_offset)?;
                let t_encode = t0.elapsed();

                tracing::info!(
                    decode_ms = t_decode.as_millis() as u64,
                    downsample_ms = (t_downsample - t_decode).as_millis() as u64,
                    effects_ms = (t_effects - t_downsample).as_millis() as u64,
                    encode_ms = (t_encode - t_effects).as_millis() as u64,
                    total_ms = t_encode.as_millis() as u64,
                    frames = num_frames,
                    "Pipeline timing"
                );

                Ok((encoded, num_frames))
            },
        )
        .await;

        match processed {
            Ok(Ok((encoded, num_frames))) => {
                tracing::info!(
                    segment_id,
                    size_kb = encoded.len() / 1024,
                    "Segment processed"
                );
                frame_counter += num_frames;
                state
                    .hls_buffer
                    .write()
                    .await
                    .push_segment(segment_id, encoded);
            }
            Ok(Err(e)) => {
                tracing::error!(segment_id = seg_id, error = %e, "Processing failed");
            }
            Err(e) => {
                tracing::error!(segment_id = seg_id, error = %e, "Task panicked");
            }
        }
        // No sleep on success — fetch next segment immediately
    }
}
