use std::sync::{Arc, Mutex};
use tokio::sync::{watch, RwLock};
use serde::{Deserialize, Serialize};

use crate::codec;
use crate::effects::{downsample_2x, RawFrame};
use crate::hls::HlsBuffer;
use crate::registry::{self, default_params, Effect, FrameCtx, ParamValues};
use crate::stream_source::StreamSource;

/// A single slot in the pipeline — one effect instance with its params.
pub struct PipelineSlot {
    pub slot_id: String,
    pub effect_id: String,
    pub effect: Box<dyn Effect>,
    pub params: ParamValues,
    pub enabled: bool,
}

/// Serializable view of a slot for API responses.
#[derive(Serialize, Deserialize, Clone)]
pub struct PipelineSlotView {
    pub slot_id: String,
    pub effect_id: String,
    pub params: ParamValues,
    pub enabled: bool,
}

/// The dynamic effect pipeline.
pub struct Pipeline {
    slots: Vec<PipelineSlot>,
    dimensions: Option<(u32, u32)>,
}

impl Pipeline {
    /// Create an empty pipeline.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            dimensions: None,
        }
    }

    /// Store dimensions and call init() on all existing effects.
    pub fn set_dimensions(&mut self, width: u32, height: u32) {
        self.dimensions = Some((width, height));
        for slot in self.slots.iter_mut() {
            slot.effect.init(width, height);
        }
    }

    /// Create a fresh effect instance from the registry, assign a UUID slot_id,
    /// call init if dimensions are known, and return the slot_id.
    pub fn add_effect(&mut self, effect_id: &str) -> Result<String, String> {
        let mut all = registry::all_effects();
        let pos = all.iter().position(|e| e.id() == effect_id);
        let effect = match pos {
            Some(i) => all.remove(i),
            None => return Err(format!("Unknown effect: {}", effect_id)),
        };
        let params = default_params(&effect.params());
        let slot_id = uuid::Uuid::new_v4().to_string();
        let mut slot = PipelineSlot {
            slot_id: slot_id.clone(),
            effect_id: effect_id.to_string(),
            effect,
            params,
            enabled: true,
        };
        if let Some((w, h)) = self.dimensions {
            slot.effect.init(w, h);
        }
        self.slots.push(slot);
        Ok(slot_id)
    }

    /// Remove the slot with the given slot_id.
    pub fn remove_slot(&mut self, slot_id: &str) {
        self.slots.retain(|s| s.slot_id != slot_id);
    }

    /// Enable or disable a slot by slot_id.
    pub fn set_enabled(&mut self, slot_id: &str, enabled: bool) {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.slot_id == slot_id) {
            slot.enabled = enabled;
        }
    }

    /// Merge new_params into the slot's existing params.
    pub fn update_params(&mut self, slot_id: &str, new_params: &ParamValues) {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.slot_id == slot_id) {
            for (k, v) in new_params {
                slot.params.insert(k.clone(), *v);
            }
        }
    }

    /// Replace the entire pipeline with a new set of (effect_id, params, enabled) entries.
    /// Generates new slot_ids for all entries.
    pub fn replace(&mut self, entries: Vec<(String, ParamValues, bool)>) -> Result<(), String> {
        let mut new_slots = Vec::new();
        for (effect_id, params, enabled) in entries {
            let mut all = registry::all_effects();
            let pos = all.iter().position(|e| e.id() == effect_id.as_str());
            let mut effect = match pos {
                Some(i) => all.remove(i),
                None => return Err(format!("Unknown effect: {}", effect_id)),
            };
            // Merge provided params over defaults
            let mut merged = default_params(&effect.params());
            for (k, v) in &params {
                merged.insert(k.clone(), *v);
            }
            let slot_id = uuid::Uuid::new_v4().to_string();
            if let Some((w, h)) = self.dimensions {
                effect.init(w, h);
            }
            new_slots.push(PipelineSlot {
                slot_id,
                effect_id,
                effect,
                params: merged,
                enabled,
            });
        }
        self.slots = new_slots;
        Ok(())
    }

    /// Return a serializable snapshot of the pipeline.
    pub fn view(&self) -> Vec<PipelineSlotView> {
        self.slots
            .iter()
            .map(|s| PipelineSlotView {
                slot_id: s.slot_id.clone(),
                effect_id: s.effect_id.clone(),
                params: s.params.clone(),
                enabled: s.enabled,
            })
            .collect()
    }

    /// Number of slots in the pipeline.
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Apply all enabled slots to a frame in order.
    pub fn process_frame(&mut self, frame: &mut RawFrame, frame_number: u32) {
        let (width, height) = match self.dimensions {
            Some(d) => d,
            None => (frame.width, frame.height),
        };
        let ctx = FrameCtx { frame_number, width, height };
        for slot in self.slots.iter_mut() {
            if slot.enabled {
                slot.effect.apply(frame, &slot.params, &ctx);
            }
        }
    }
}

/// Shared state between the pipeline and HTTP server.
pub struct AppState {
    pub hls_buffer: RwLock<HlsBuffer>,
    pub pipeline_active: watch::Sender<bool>,
    pub last_client_request: std::sync::atomic::AtomicU64,
    pub pipeline: Mutex<Pipeline>,
}

impl AppState {
    pub fn new() -> (Arc<Self>, watch::Receiver<bool>) {
        let (tx, rx) = watch::channel(false);

        let mut p = Pipeline::new();
        p.add_effect("distortion").expect("distortion registered");
        p.add_effect("quantize").expect("quantize registered");
        p.add_effect("edges").expect("edges registered");
        p.add_effect("canvas_texture").expect("canvas_texture registered");

        let state = Arc::new(Self {
            hls_buffer: RwLock::new(HlsBuffer::new(10)),
            pipeline_active: tx,
            last_client_request: std::sync::atomic::AtomicU64::new(0),
            pipeline: Mutex::new(p),
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

        // Fetch and download first segment
        let segment_id = match source.fetch_latest_segment_id().await {
            Some(id) => id,
            None => {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };
        let ts_bytes = match source.download_segment(&segment_id).await {
            Some(bytes) => bytes,
            None => {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        // Spawn CPU-bound processing on blocking thread
        let pts_offset = frame_counter;
        let state_clone = state.clone();
        let process_handle = tokio::task::spawn_blocking(
            move || -> Result<(Vec<u8>, i64), Box<dyn std::error::Error + Send + Sync>> {
                let t0 = std::time::Instant::now();

                let all_frames = codec::decode_segment(&ts_bytes)?;
                let t_decode = t0.elapsed();

                if all_frames.is_empty() {
                    return Err("No frames decoded".into());
                }

                let mut half_frames: Vec<_> = all_frames
                    .iter()
                    .map(|f| downsample_2x(f))
                    .collect();
                let t_downsample = t0.elapsed();

                let half_w = half_frames[0].width;
                let half_h = half_frames[0].height;
                let out_frames = half_frames.len() as i64;

                {
                    let mut pipeline = state_clone.pipeline.lock().unwrap();
                    pipeline.set_dimensions(half_w, half_h);

                    // Time-of-day edge darkness
                    let (edge_color, _bg) = crate::time_color::get_colors_now();
                    let edge_darkness = if edge_color == (0, 0, 0) { 100.0 } else { 40.0 };
                    for slot in pipeline.view().iter() {
                        if slot.effect_id == "edges" {
                            let mut params = std::collections::HashMap::new();
                            params.insert("darkness".to_string(), edge_darkness);
                            pipeline.update_params(&slot.slot_id, &params);
                        }
                    }

                    for (i, frame) in half_frames.iter_mut().enumerate() {
                        pipeline.process_frame(frame, i as u32);
                    }
                }

                let t_effects = t0.elapsed();

                let encoded = codec::encode_segment(&half_frames, 30, pts_offset)?;
                let t_encode = t0.elapsed();

                tracing::info!(
                    decode_ms = t_decode.as_millis() as u64,
                    downsample_ms = (t_downsample - t_decode).as_millis() as u64,
                    effects_ms = (t_effects - t_downsample).as_millis() as u64,
                    encode_ms = (t_encode - t_effects).as_millis() as u64,
                    total_ms = t_encode.as_millis() as u64,
                    frames = out_frames,
                    "Pipeline timing"
                );

                Ok((encoded, out_frames))
            },
        );

        // While processing runs on blocking thread, prefetch next segment
        let next_id = source.fetch_latest_segment_id().await;
        let next_bytes = if let Some(ref id) = next_id {
            source.download_segment(id).await
        } else {
            None
        };

        // Now await processing result
        let processed = process_handle.await;
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
                tracing::error!(error = %e, "Processing failed");
            }
            Err(e) => {
                tracing::error!(error = %e, "Task panicked");
            }
        }

        // If we prefetched the next segment, process it immediately
        if let (Some(next_seg_id), Some(next_ts)) = (next_id, next_bytes) {
            let pts_offset = frame_counter;
            let state_clone = state.clone();
            let processed = tokio::task::spawn_blocking(
                move || -> Result<(Vec<u8>, i64), Box<dyn std::error::Error + Send + Sync>> {
                    let all_frames = codec::decode_segment(&next_ts)?;
                    if all_frames.is_empty() {
                        return Err("No frames decoded".into());
                    }
                    let mut half_frames: Vec<_> = all_frames
                        .iter()
                        .map(|f| downsample_2x(f))
                        .collect();
                    let half_w = half_frames[0].width;
                    let half_h = half_frames[0].height;
                    let out_frames = half_frames.len() as i64;

                    {
                        let mut pipeline = state_clone.pipeline.lock().unwrap();
                        pipeline.set_dimensions(half_w, half_h);

                        // Time-of-day edge darkness
                        let (edge_color, _bg) = crate::time_color::get_colors_now();
                        let edge_darkness = if edge_color == (0, 0, 0) { 100.0 } else { 40.0 };
                        for slot in pipeline.view().iter() {
                            if slot.effect_id == "edges" {
                                let mut params = std::collections::HashMap::new();
                                params.insert("darkness".to_string(), edge_darkness);
                                pipeline.update_params(&slot.slot_id, &params);
                            }
                        }

                        for (i, frame) in half_frames.iter_mut().enumerate() {
                            pipeline.process_frame(frame, i as u32);
                        }
                    }

                    let encoded = codec::encode_segment(&half_frames, 30, pts_offset)?;
                    Ok((encoded, out_frames))
                },
            )
            .await;

            match processed {
                Ok(Ok((encoded, num_frames))) => {
                    tracing::info!(
                        segment_id = next_seg_id,
                        size_kb = encoded.len() / 1024,
                        "Segment processed (prefetched)"
                    );
                    frame_counter += num_frames;
                    state
                        .hls_buffer
                        .write()
                        .await
                        .push_segment(next_seg_id, encoded);
                }
                Ok(Err(e)) => {
                    tracing::error!(error = %e, "Processing failed (prefetched)");
                }
                Err(e) => {
                    tracing::error!(error = %e, "Task panicked (prefetched)");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::RawFrame;

    #[test]
    fn pipeline_processes_frame() {
        let mut pipeline = Pipeline::new();
        pipeline.add_effect("quantize").unwrap();
        pipeline.set_dimensions(8, 8);

        let mut frame = RawFrame::new(8, 8);
        for i in 0..frame.data.len() {
            frame.data[i] = (i % 256) as u8;
        }
        let original = frame.data.clone();
        pipeline.process_frame(&mut frame, 0);
        assert_ne!(frame.data, original, "Pipeline should modify frame");
    }

    #[test]
    fn disabled_effect_skipped() {
        let mut pipeline = Pipeline::new();
        let slot_id = pipeline.add_effect("quantize").unwrap();
        pipeline.set_dimensions(4, 4);
        pipeline.set_enabled(&slot_id, false);

        let mut frame = RawFrame::filled(4, 4, 100, 100, 100);
        let original = frame.data.clone();
        pipeline.process_frame(&mut frame, 0);
        assert_eq!(frame.data, original, "Disabled effect should not modify frame");
    }

    #[test]
    fn add_unknown_effect_fails() {
        let mut pipeline = Pipeline::new();
        assert!(pipeline.add_effect("nonexistent").is_err());
    }

    #[test]
    fn remove_effect() {
        let mut pipeline = Pipeline::new();
        let slot_id = pipeline.add_effect("quantize").unwrap();
        assert_eq!(pipeline.slot_count(), 1);
        pipeline.remove_slot(&slot_id);
        assert_eq!(pipeline.slot_count(), 0);
    }
}
