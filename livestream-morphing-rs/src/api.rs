use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, patch, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::pipeline::AppState;
use crate::registry::{self, ParamDef, ParamValues};

// ---------------------------------------------------------------------------
// Effect library
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct EffectInfo {
    id: String,
    name: String,
    params: Vec<ParamDef>,
}

async fn list_effects() -> impl IntoResponse {
    let effects: Vec<EffectInfo> = registry::all_effects()
        .iter()
        .map(|e| EffectInfo {
            id: e.id().to_string(),
            name: e.name().to_string(),
            params: e.params(),
        })
        .collect();
    Json(effects)
}

// ---------------------------------------------------------------------------
// Pipeline endpoints
// ---------------------------------------------------------------------------

async fn get_pipeline(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let view = state.pipeline.lock().unwrap().view();
    Json(view)
}

#[derive(Deserialize)]
struct PipelineEntry {
    effect_id: String,
    params: ParamValues,
    enabled: bool,
}

async fn put_pipeline(
    State(state): State<Arc<AppState>>,
    Json(entries): Json<Vec<PipelineEntry>>,
) -> impl IntoResponse {
    let tuples: Vec<(String, ParamValues, bool)> = entries
        .into_iter()
        .map(|e| (e.effect_id, e.params, e.enabled))
        .collect();

    let mut pipeline = state.pipeline.lock().unwrap();
    match pipeline.replace(tuples) {
        Ok(()) => Json(pipeline.view()).into_response(),
        Err(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
    }
}

#[derive(Deserialize)]
struct PatchSlot {
    params: Option<ParamValues>,
    enabled: Option<bool>,
}

async fn patch_slot(
    State(state): State<Arc<AppState>>,
    Path(slot_id): Path<String>,
    Json(body): Json<PatchSlot>,
) -> impl IntoResponse {
    let mut pipeline = state.pipeline.lock().unwrap();
    if let Some(params) = &body.params {
        pipeline.update_params(&slot_id, params);
    }
    if let Some(enabled) = body.enabled {
        pipeline.set_enabled(&slot_id, enabled);
    }
    Json(pipeline.view())
}

async fn add_effect(
    State(state): State<Arc<AppState>>,
    Path(effect_id): Path<String>,
) -> impl IntoResponse {
    let mut pipeline = state.pipeline.lock().unwrap();
    match pipeline.add_effect(&effect_id) {
        Ok(_slot_id) => Json(pipeline.view()).into_response(),
        Err(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
    }
}

async fn delete_slot(
    State(state): State<Arc<AppState>>,
    Path(slot_id): Path<String>,
) -> impl IntoResponse {
    let mut pipeline = state.pipeline.lock().unwrap();
    pipeline.remove_slot(&slot_id);
    Json(pipeline.view())
}

// ---------------------------------------------------------------------------
// Preset storage
// ---------------------------------------------------------------------------

/// Derive a filesystem-safe ID from a preset name.
fn name_to_id(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c == ' ' { '_' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect()
}

fn presets_dir() -> std::path::PathBuf {
    // Resolve relative to the binary's working directory (i.e., the project root).
    let mut dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    dir.push("presets");
    dir
}

/// One effect entry as stored in a TOML preset.
#[derive(Serialize, Deserialize, Clone)]
struct PresetEffect {
    effect_id: String,
    enabled: bool,
    params: HashMap<String, f32>,
}

/// Full preset file contents.
#[derive(Serialize, Deserialize)]
struct PresetFile {
    name: String,
    effects: Vec<PresetEffect>,
}

/// Summary returned by `GET /api/presets`.
#[derive(Serialize)]
struct PresetSummary {
    id: String,
    name: String,
}

async fn list_presets() -> impl IntoResponse {
    let dir = presets_dir();
    let mut summaries: Vec<PresetSummary> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(preset) = toml::from_str::<PresetFile>(&content) {
                    let id = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    summaries.push(PresetSummary { id, name: preset.name });
                }
            }
        }
    }

    Json(summaries)
}

#[derive(Deserialize)]
struct SavePresetBody {
    name: String,
}

async fn save_preset(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SavePresetBody>,
) -> impl IntoResponse {
    let id = name_to_id(&body.name);
    if id.is_empty() {
        return (StatusCode::BAD_REQUEST, "Invalid preset name").into_response();
    }

    let slots = state.pipeline.lock().unwrap().view();
    let effects: Vec<PresetEffect> = slots
        .into_iter()
        .map(|s| PresetEffect {
            effect_id: s.effect_id,
            enabled: s.enabled,
            params: s.params,
        })
        .collect();

    let preset = PresetFile { name: body.name, effects };

    let toml_str = match toml::to_string_pretty(&preset) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let dir = presets_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    let path = dir.join(format!("{}.toml", id));
    if let Err(e) = std::fs::write(&path, toml_str) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    let body = serde_json::json!({ "id": id });
    (StatusCode::CREATED, Json(body)).into_response()
}

async fn apply_preset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Basic path-traversal guard.
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return (StatusCode::BAD_REQUEST, "Invalid preset id").into_response();
    }

    let path = presets_dir().join(format!("{}.toml", id));
    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    let preset: PresetFile = match toml::from_str(&content) {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let entries: Vec<(String, ParamValues, bool)> = preset
        .effects
        .into_iter()
        .map(|e| (e.effect_id, e.params, e.enabled))
        .collect();

    let mut pipeline = state.pipeline.lock().unwrap();
    match pipeline.replace(entries) {
        Ok(()) => Json(pipeline.view()).into_response(),
        Err(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
    }
}

async fn delete_preset(Path(id): Path<String>) -> impl IntoResponse {
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return (StatusCode::BAD_REQUEST, "Invalid preset id").into_response();
    }

    let path = presets_dir().join(format!("{}.toml", id));
    match std::fs::remove_file(&path) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Stream source
// ---------------------------------------------------------------------------

async fn get_source(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let url = state.stream_url.read().await.clone();
    Json(serde_json::json!({ "url": url }))
}

#[derive(Deserialize)]
struct SetSourceBody {
    url: String,
}

async fn put_source(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetSourceBody>,
) -> impl IntoResponse {
    let mut stream_url = state.stream_url.write().await;
    *stream_url = body.url.clone();
    Json(serde_json::json!({ "url": body.url }))
}

// ---------------------------------------------------------------------------
// Processing status
// ---------------------------------------------------------------------------

async fn get_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let stats = state.stats.lock().unwrap().clone();
    let buf = state.hls_buffer.read().await;
    let segments: Vec<serde_json::Value> = buf
        .segment_info()
        .into_iter()
        .map(|(id, size)| serde_json::json!({ "id": id, "size_kb": size / 1024 }))
        .collect();
    let total_segments = buf.segment_count();
    let max_segments = 10; // matches HlsBuffer::new(10)

    Json(serde_json::json!({
        "effects_ms": stats.effects_ms,
        "total_ms": stats.total_ms,
        "frames": stats.frames,
        "segment_completed_at": stats.segment_completed_at,
        "segments": segments,
        "buffer_count": total_segments,
        "buffer_max": max_segments,
    }))
}

// ---------------------------------------------------------------------------
// Router factory
// ---------------------------------------------------------------------------

pub fn api_router() -> Router<Arc<AppState>> {
    Router::new()
        // Effects library
        .route("/api/effects", get(list_effects))
        // Pipeline
        .route("/api/pipeline", get(get_pipeline))
        .route("/api/pipeline", put(put_pipeline))
        .route("/api/pipeline/{slot_id}", patch(patch_slot))
        .route("/api/pipeline/add/{effect_id}", post(add_effect))
        .route("/api/pipeline/{slot_id}", delete(delete_slot))
        // Stream source
        .route("/api/source", get(get_source))
        .route("/api/source", put(put_source))
        // Status
        .route("/api/status", get(get_status))
        // Presets
        .route("/api/presets", get(list_presets))
        .route("/api/presets", post(save_preset))
        .route("/api/presets/{id}/apply", put(apply_preset))
        .route("/api/presets/{id}", delete(delete_preset))
}
