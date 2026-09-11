use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{watch, RwLock};
use serde::{Deserialize, Serialize};

use crate::effects::RawFrame;
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
    pub(crate) source_end: Option<f64>,
    pub(crate) repaint: crate::repaint::Repaint,
}

impl Pipeline {
    /// Create an empty pipeline.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            dimensions: None,
            source_end: None,
            repaint: Default::default(),
        }
    }

    /// Store dimensions and call init() on all existing effects.
    pub fn set_dimensions(&mut self, width: u32, height: u32) {
        if self.dimensions == Some((width, height)) { return; }
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
        self.process_frame_timed(frame, frame_number, 1.0/30.0);
    }

    pub fn clear_enabled_history(&mut self) {
        for slot in self.slots.iter_mut().filter(|s|s.enabled) { slot.effect.clear_history(); }
    }

    pub fn reset(&mut self) {
        self.source_end = None;
        self.repaint = Default::default();
        if let Some((w,h)) = self.dimensions {
            for slot in &mut self.slots { slot.effect.init(w,h); }
        }
    }

    pub fn process_frame_timed(&mut self, frame: &mut RawFrame, frame_number: u32, delta_seconds: f32) {
        let (width, height) = match self.dimensions {
            Some(d) => d,
            None => (frame.width, frame.height),
        };
        let ctx = FrameCtx { delta_seconds, frame_number, width, height };
        for slot in self.slots.iter_mut() {
            if slot.enabled {
                slot.effect.apply(frame, &slot.params, &ctx);
            }
        }
    }
}

/// Processing stats from the last completed segment.
#[derive(Clone, Default, serde::Serialize)]
pub struct ProcessingStats {
    pub effects_ms: u64,
    pub total_ms: u64,
    pub frames: u64,
    pub segment_completed_at: u64, // unix timestamp in millis
    pub width: u32,
    pub height: u32,
    pub source_fps: f64,
    pub paint_fps: f64,
    pub post_ms: u64,
    pub repaint_ms: u64,
    pub warp_ms: u64,
    pub frame_p50_ms: f64,
    pub frame_p95_ms: f64,
    pub duration_seconds: f64,
    pub rendered_frames: u64,
    pub encoded_bytes: usize,
}

const READY_SEGMENTS: usize = 3;
const FRESH_SEGMENT_MS: u64 = 18_000;
const UNAVAILABLE_AFTER_MS: u64 = 90_000;

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LiveState {
    Starting,
    Live,
    Unavailable,
}

#[derive(Serialize)]
pub struct LiveStatus {
    pub state: LiveState,
    pub buffer_count: usize,
    pub ready: bool,
    pub last_segment_at: u64,
    pub engine: ProcessingStats,
}

/// Shared state between the pipeline and HTTP server.
pub struct AppState {
    pub hls_buffer: RwLock<HlsBuffer>,
    pub pipeline_active: watch::Sender<bool>,
    pub last_client_request: AtomicU64,
    pub pipeline: Mutex<Pipeline>,
    pub stream_url: RwLock<String>,
    pub stats: Mutex<ProcessingStats>,
    pipeline_started_at: AtomicU64,
    session_id: Mutex<uuid::Uuid>,
    #[cfg(test)]
    live_status_snapshot_hook:
        Mutex<Option<(Arc<tokio::sync::Barrier>, Arc<tokio::sync::Barrier>)>>,
}

// A deployment setting keeps the selected artwork across restarts, while an
// unset value retains the original line drawing for other environments.
fn live_paint_edition(value: Option<&str>) -> f32 {
    value.and_then(|v|v.parse::<u32>().ok()).filter(|v|(65..=80).contains(v)).unwrap_or(65) as f32
}
fn startup_pipeline(style: &str) -> Pipeline {
    let mut p=Pipeline::new();
    if style=="fine_knife" || style=="fine_knife_nyc" || style=="fine_knife_london" {
        let slot=p.add_effect("ink_editions").expect("editions registered");
        p.update_params(&slot,&ParamValues::from([
            ("edition".into(),if style=="fine_knife_nyc" {live_paint_edition(std::env::var("LIVE_PAINT_EDITION").ok().as_deref())} else if style=="fine_knife_london" {53.} else {46.}),("detail".into(),1.),("pigment".into(),1.)
        ]));
        if style=="fine_knife_london" && std::env::var("LIVE_REPAINT").as_deref()==Ok("true") {
            p.update_params(&slot,&ParamValues::from([
                ("repaint".into(),1.),
                ("calm".into(),if std::env::var("LIVE_CALM").as_deref()==Ok("true") {1.} else {0.}),
                ("paint_stride".into(),std::env::var("LIVE_PAINT_STRIDE").ok().and_then(|v|v.parse::<u8>().ok()).unwrap_or(1).clamp(1,3) as f32)
            ]));
        }
    } else {
        let slot=p.add_effect("living_ink").expect("living ink registered");
        let preset:ParamValues=serde_json::from_str(include_str!("living_ink_preset.json")).expect("valid built-in ink preset");
        p.update_params(&slot,&preset);
    }
    p
}

impl AppState {
    pub fn new() -> (Arc<Self>, watch::Receiver<bool>) {
        let (tx, rx) = watch::channel(false);

        let p = startup_pipeline(&std::env::var("LIVE_STYLE").unwrap_or_default());

        let state = Arc::new(Self {
            hls_buffer: RwLock::new(HlsBuffer::new(10)),
            pipeline_active: tx,
            last_client_request: AtomicU64::new(0),
            pipeline: Mutex::new(p),
            stream_url: RwLock::new(StreamSource::default_url()),
            stats: Mutex::new(ProcessingStats::default()),
            pipeline_started_at: AtomicU64::new(0),
            session_id: Mutex::new(uuid::Uuid::nil()),
            #[cfg(test)]
            live_status_snapshot_hook: Mutex::new(None),
        });
        (state, rx)
    }

    pub async fn live_status(&self, now_ms: u64) -> LiveStatus {
        let buffer = self.hls_buffer.read().await;
        let buffer_count = buffer.segment_count();
        #[cfg(test)]
        {
            let hook = self.live_status_snapshot_hook.lock().unwrap().clone();
            if let Some((captured, resume)) = hook {
                captured.wait().await;
                resume.wait().await;
            }
        }
        let last_segment_at = self.stats.lock().unwrap().segment_completed_at;
        drop(buffer);
        let started_at = self.pipeline_started_at.load(Ordering::Relaxed);
        let fresh =
            last_segment_at > 0 && now_ms.saturating_sub(last_segment_at) <= FRESH_SEGMENT_MS;
        let ready = buffer_count >= READY_SEGMENTS && fresh;
        let state = if ready {
            LiveState::Live
        } else if started_at > 0 && now_ms.saturating_sub(started_at) > UNAVAILABLE_AFTER_MS {
            LiveState::Unavailable
        } else {
            LiveState::Starting
        };
        LiveStatus {
            state,
            buffer_count,
            ready,
            last_segment_at,
            engine: self.stats.lock().unwrap().clone(),
        }
    }

    pub fn mark_pipeline_started(&self, now_ms: u64) {
        *self.stats.lock().unwrap() = ProcessingStats::default();
        self.pipeline.lock().unwrap().reset();
        self.pipeline_started_at.store(now_ms, Ordering::Relaxed);
        *self.session_id.lock().unwrap() = uuid::Uuid::new_v4();
    }

    pub fn output_segment_id(&self, source_id: &str) -> String {
        let session_id = self.session_id.lock().unwrap();
        format!("{session_id}-{source_id}")
    }

    async fn publish_segment(&self, source_id: &str, encoded: Vec<u8>, stats: ProcessingStats) {
        self.publish_paired_segment(source_id, encoded, stats, None).await;
    }

    async fn publish_paired_segment(&self, source_id: &str, encoded: Vec<u8>, stats: ProcessingStats, raw: Option<Vec<u8>>) {
        let mut buffer = self.hls_buffer.write().await;
        let mut current_stats = self.stats.lock().unwrap();
        let previous_completed_at = current_stats.segment_completed_at;
        if previous_completed_at > 0
            && stats
                .segment_completed_at
                .saturating_sub(previous_completed_at)
                > FRESH_SEGMENT_MS
        {
            buffer.clear();
        }
        *current_stats = stats;
        buffer.push_paired_segment(self.output_segment_id(source_id), encoded, current_stats.duration_seconds.max(0.001) as f32, raw);
    }

    /// Record a client request and activate the pipeline if needed.
    pub fn touch(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_client_request
            .store(now, Ordering::Relaxed);
        let _ = self.pipeline_active.send(true);
    }

    /// Seconds since last client request.
    pub fn idle_seconds(&self) -> u64 {
        let last = self
            .last_client_request
            .load(Ordering::Relaxed);
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


/// Shared segment processing for both ingestion paths. No duplicate RGB batch.
fn render_segment(bytes: &[u8], state: &AppState, pts: i64) -> Result<(Vec<u8>, i64, ProcessingStats), Box<dyn std::error::Error + Send + Sync>> {
    let start=std::time::Instant::now();
    let width=std::env::var("OUTPUT_WIDTH").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(1920).clamp(320,1920);
    let workers=std::env::var("RENDER_WORKERS").ok().and_then(|v|v.parse::<usize>().ok()).unwrap_or(3).clamp(1,6);
    let previous_stats=state.stats.lock().unwrap().clone();
    let rendered={
        let mut pipeline=state.pipeline.lock().unwrap();
        // Snapshot city time once per segment; worker pipelines share this grade.
        let local = chrono::Utc::now();
        use chrono::Timelike;
        for slot in pipeline.slots.iter_mut().filter(|s| s.effect_id == "ink_editions" && s.params.get("edition").map(|v|*v == 53.0 || (65.0..=80.0).contains(v)).unwrap_or(false)) {
            let timezone = if slot.params.get("edition") == Some(&53.) { chrono_tz::Europe::London } else { chrono_tz::America::New_York };
            let local = local.with_timezone(&timezone);
            slot.params.insert("hour".into(), local.hour() as f32 + local.minute() as f32/60. + local.second() as f32/3600.);
        }
        let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis() as u64;
        if previous_stats.segment_completed_at>0 && now.saturating_sub(previous_stats.segment_completed_at)>FRESH_SEGMENT_MS {pipeline.reset();}
        match crate::renderer::render(bytes,width,1,pts,previous_stats.rendered_frames,&mut pipeline,workers) {
            Ok(rendered) if rendered.frames as f64/rendered.fps<=13.0 => rendered,
            Ok(_) => {pipeline.reset();return Err("Source segment exceeds the 13-second HLS target".into());}
            Err(error) => {pipeline.reset();return Err(error);}
        }
    };
    let duration=rendered.frames as f64/rendered.fps;
    let mut times=rendered.times;
    let effects_ms=times.iter().sum::<f64>().round() as u64;
    times.sort_by(f64::total_cmp);
    let encoded=rendered.bytes;
    let stats=ProcessingStats {
        effects_ms,total_ms:start.elapsed().as_millis() as u64,frames:rendered.frames as u64,
        width:rendered.width,height:rendered.height,source_fps:rendered.fps,paint_fps:rendered.paint_fps,post_ms:rendered.post_ms.round() as u64,repaint_ms:rendered.repaint_ms.round() as u64,warp_ms:rendered.warp_ms.round() as u64,
        frame_p50_ms:times[times.len()/2],frame_p95_ms:times[(times.len()*95/100).min(times.len()-1)],
        duration_seconds:duration,rendered_frames:previous_stats.rendered_frames+rendered.frames as u64,
        encoded_bytes:encoded.len(),segment_completed_at:0,
    };
    Ok((encoded,(duration*90000.0).round() as i64,stats))
}

pub fn release_idle_memory() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        extern "C" { fn malloc_trim(pad: usize) -> i32; }
        // glibc's thread-safe trim releases only unused allocator pages.
        unsafe { malloc_trim(0); }
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

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    state.mark_pipeline_started(now_ms);
    tracing::info!("Pipeline activated!");
    let url = state.stream_url.read().await.clone();
    let mut source = StreamSource::new(url);
    let mut frame_counter: i64 = 0;
    let idle_timeout = std::time::Duration::from_secs(60); // Stop work promptly when the last viewer leaves.

    loop {
        // Check if stream URL changed
        {
            let current_url = state.stream_url.read().await;
            if source.url() != current_url.as_str() {
                tracing::info!(new_url = %current_url, "Stream URL changed, reconnecting");
                source = StreamSource::new(current_url.clone());
                state.hls_buffer.write().await.clear();
                frame_counter = 0;
                state.mark_pipeline_started(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64);
            }
        }

        // Check idle timeout
        if state.idle_seconds() > idle_timeout.as_secs() {
            tracing::info!("No clients for 1 minute, pipeline going idle");
            let _ = state.pipeline_active.send(false);
            state.hls_buffer.write().await.clear();

            // Return freed decoder/encoder arenas to Linux instead of paying for idle RSS.
            release_idle_memory();

            // Wait for reactivation
            loop {
                if *active.borrow_and_update() {
                    break;
                }
                if active.changed().await.is_err() {
                    return;
                }
            }
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            state.mark_pipeline_started(now_ms);
            tracing::info!("Pipeline reactivated!");
            let url = state.stream_url.read().await.clone();
            source = StreamSource::new(url);
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
        let process_handle = tokio::task::spawn_blocking(move || {
            (render_segment(&ts_bytes, &state_clone, pts_offset), ts_bytes)
        });

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
            Ok((Ok((encoded, ticks, mut stats)), raw)) => {
                tracing::info!(
                    segment_id,
                    size_kb = encoded.len() / 1024,
                    "Segment processed"
                );
                frame_counter += ticks;

                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                state
                    .publish_paired_segment(
                        &segment_id,
                        encoded,
                        { stats.segment_completed_at = now_ms; stats },
                        Some(raw),
                    )
                    .await;
            }
            Ok((Err(e), _)) => {
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
            let processed = tokio::task::spawn_blocking(move || {
                (render_segment(&next_ts, &state_clone, pts_offset), next_ts)
            }).await;

            match processed {
                Ok((Ok((encoded, ticks, mut stats)), raw)) => {
                    tracing::info!(
                        segment_id = next_seg_id,
                        size_kb = encoded.len() / 1024,
                        "Segment processed (prefetched)"
                    );
                    frame_counter += ticks;

                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    state
                        .publish_paired_segment(
                            &next_seg_id,
                            encoded,
                            { stats.segment_completed_at = now_ms; stats },
                        Some(raw),
                        )
                        .await;
                }
                Ok((Err(e), _)) => {
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
    #[test]
    fn rejected_long_segment_clears_continuity() {
        crate::codec::init();
        let (state,_)=AppState::new();
        let frames=vec![crate::effects::RawFrame::filled(64,36,180,180,180);420];
        let bytes=crate::codec::encode_segment(&frames,30,0).unwrap();
        let error=render_segment(&bytes,&state,0).err().unwrap();
        assert!(error.to_string().contains("13-second"));
        assert!(state.pipeline.lock().unwrap().source_end.is_none());
    }
    #[test]
    fn repeated_dimensions_preserve_temporal_state_and_reset_clears_it() {
        let mut p = super::Pipeline::new();
        p.add_effect("living_ink").unwrap();
        p.set_dimensions(64,32);
        let mut source = crate::effects::RawFrame::filled(64,32,20,20,20);
        for y in 0..32 { for x in 24..64 { source.data[(y*64+x)*3..(y*64+x)*3+3].fill(220); } }
        p.process_frame(&mut source,0);
        p.set_dimensions(64,32);
        let mut blank=crate::effects::RawFrame::filled(64,32,20,20,20);
        p.process_frame(&mut blank,1);
        assert!(blank.data.iter().any(|v|*v>24));
        p.reset();
        let mut blank=crate::effects::RawFrame::filled(64,32,20,20,20);
        p.process_frame(&mut blank,2);
        assert!(blank.data.iter().all(|v|*v==24));
    }

    #[test]
    fn live_paint_selection_is_bounded_and_defaults_to_known_release() {
        assert_eq!(live_paint_edition(None),65.);
        for value in ["", "0", "64", "81", "NaN", "66.5"] {assert_eq!(live_paint_edition(Some(value)),65.);}
        for value in 65..=80 {assert_eq!(live_paint_edition(Some(&value.to_string())),value as f32);}
    }
    #[test]
    fn new_york_startup_selects_refined_paint() {
        let slots=startup_pipeline("fine_knife_nyc").view();
        assert_eq!(slots[0].params["edition"],live_paint_edition(std::env::var("LIVE_PAINT_EDITION").ok().as_deref()));
    }

    #[test]
    fn fine_knife_startup_selects_the_approved_edition() {
        let dynamic = startup_pipeline("fine_knife_london").view();
        assert_eq!(dynamic[0].params["edition"],53.);
        assert!(dynamic[0].params.get("day_cycle").copied().unwrap_or(0.) < 0.5);
        let slots=startup_pipeline("fine_knife").view();
        assert_eq!(slots.len(),1);
        assert_eq!(slots[0].effect_id,"ink_editions");
        assert_eq!(slots[0].params["edition"],46.);
        assert_eq!(startup_pipeline("").view()[0].effect_id,"living_ink");
    }

    #[test]
    fn default_pipeline_is_monochrome_before_quantization_and_edges() {
        let (state, _rx) = AppState::new();
        let effect_ids: Vec<String> = state
            .pipeline
            .lock()
            .unwrap()
            .view()
            .into_iter()
            .map(|slot| slot.effect_id)
            .collect();

        assert_eq!(
            effect_ids,
            ["living_ink"]
        );
    }

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

    #[tokio::test]
    async fn public_status_requires_three_fresh_segments() {
        let (state, _rx) = AppState::new();
        state.mark_pipeline_started(1_000);
        for id in ["1", "2"] {
            state.hls_buffer.write().await.push_segment(id.into(), vec![1]);
        }
        state.stats.lock().unwrap().segment_completed_at = 10_000;
        let status = state.live_status(20_000).await;
        assert_eq!(status.state, LiveState::Starting);
        assert!(!status.ready);

        state.hls_buffer.write().await.push_segment("3".into(), vec![1]);
        let status = state.live_status(20_000).await;
        assert_eq!(status.state, LiveState::Live);
        assert!(status.ready);
    }

    #[tokio::test]
    async fn stale_stream_is_unavailable() {
        let (state, _rx) = AppState::new();
        state.mark_pipeline_started(1_000);
        for id in ["1", "2", "3"] {
            state.hls_buffer.write().await.push_segment(id.into(), vec![1]);
        }
        state.stats.lock().unwrap().segment_completed_at = 10_000;
        assert_eq!(state.live_status(101_001).await.state, LiveState::Unavailable);
    }

    #[tokio::test]
    async fn post_stall_segment_starts_new_readiness_window() {
        let (state, _rx) = AppState::new();
        state.mark_pipeline_started(1_000);
        for (id, completed_at) in [("1", 10_000), ("2", 16_000)] {
            state
                .publish_segment(
                    id,
                    vec![1],
                    ProcessingStats {
                        segment_completed_at: completed_at,
                        ..ProcessingStats::default()
                    },
                )
                .await;
        }

        state
            .publish_segment(
                "3",
                vec![1],
                ProcessingStats {
                    segment_completed_at: 40_001,
                    ..ProcessingStats::default()
                },
            )
            .await;

        let status = state.live_status(40_001).await;
        assert_eq!(status.buffer_count, 1);
        assert_eq!(status.state, LiveState::Starting);
        assert!(!status.ready);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn live_status_samples_one_readiness_window() {
        let (state, _rx) = AppState::new();
        state.mark_pipeline_started(1_000);
        for (id, completed_at) in [("1", 10_000), ("2", 16_000), ("3", 22_000)] {
            state
                .publish_segment(
                    id,
                    vec![1],
                    ProcessingStats {
                        segment_completed_at: completed_at,
                        ..ProcessingStats::default()
                    },
                )
                .await;
        }

        let captured = Arc::new(tokio::sync::Barrier::new(2));
        let resume = Arc::new(tokio::sync::Barrier::new(2));
        *state.live_status_snapshot_hook.lock().unwrap() =
            Some((captured.clone(), resume.clone()));

        let status_state = state.clone();
        let status_task = tokio::spawn(async move { status_state.live_status(40_001).await });
        captured.wait().await;

        let publish_state = state.clone();
        let mut publish_task = tokio::spawn(async move {
            publish_state
                .publish_segment(
                    "4",
                    vec![1],
                    ProcessingStats {
                        segment_completed_at: 40_001,
                        ..ProcessingStats::default()
                    },
                )
                .await;
        });
        let published_while_status_paused = match tokio::time::timeout(
            std::time::Duration::from_secs(1),
            &mut publish_task,
        )
        .await
        {
            Ok(result) => {
                result.unwrap();
                true
            }
            Err(_) => false,
        };

        resume.wait().await;
        let status = status_task.await.unwrap();
        if !published_while_status_paused {
            publish_task.await.unwrap();
        }

        assert_eq!(status.buffer_count, 3);
        assert_eq!(status.state, LiveState::Starting);
        assert!(!status.ready);
    }

    #[test]
    fn output_ids_change_between_sessions_with_same_timestamp() {
        let (state, _rx) = AppState::new();
        state.mark_pipeline_started(1_000);
        let first = state.output_segment_id("44");
        state.mark_pipeline_started(1_000);
        let second = state.output_segment_id("44");
        assert_ne!(first, second);
        assert!(first.ends_with("-44"));
        assert!(second.ends_with("-44"));
    }
}
