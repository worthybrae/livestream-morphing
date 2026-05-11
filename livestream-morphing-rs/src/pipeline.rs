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
                let frames = codec::decode_segment(&ts_bytes)?;

                if frames.is_empty() {
                    return Err("No frames decoded".into());
                }

                let num_frames = frames.len() as i64;
                let orig_w = frames[0].width;
                let orig_h = frames[0].height;

                // Downsample to half res for processing
                let mut half_frames: Vec<_> = frames.iter().map(|f| downsample_2x(f)).collect();
                let half_w = half_frames[0].width;
                let half_h = half_frames[0].height;

                // Process each frame
                let mut processor = FrameProcessor::new(half_w, half_h);

                // Adjust edge darkness based on London time
                let (edge_color, _bg) = crate::time_color::get_colors_now();
                processor.edge_darkness = if edge_color == (0, 0, 0) { 100 } else { 40 };

                for (i, frame) in half_frames.iter_mut().enumerate() {
                    processor.process_frame(frame, i as u32);
                }

                // Upsample back to original size
                let full_frames: Vec<_> = half_frames
                    .iter()
                    .map(|f| upsample_2x(f, orig_w, orig_h))
                    .collect();

                // Encode with continuous PTS
                let encoded = codec::encode_segment(&full_frames, 30, pts_offset)?;
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

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}
