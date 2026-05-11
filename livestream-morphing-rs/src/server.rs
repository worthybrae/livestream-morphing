use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::pipeline::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/stream", get(stream_playlist))
        .route("/api/segments/{segment_id}.ts", get(get_segment))
        .route("/health", get(health))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn stream_playlist(State(state): State<Arc<AppState>>) -> Response {
    // Record client activity and activate pipeline
    state.touch();

    let buf = state.hls_buffer.read().await;
    let playlist = buf.generate_playlist();

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/x-mpegURL"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        playlist,
    )
        .into_response()
}

async fn get_segment(
    State(state): State<Arc<AppState>>,
    Path(segment_id): Path<String>,
) -> Response {
    state.touch();

    // Strip .ts extension if present
    let id = segment_id.strip_suffix(".ts").unwrap_or(&segment_id);

    let buf = state.hls_buffer.read().await;
    match buf.get_segment(id) {
        Some(data) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "video/mp2t"),
                (header::CACHE_CONTROL, "max-age=3600"),
            ],
            data.to_vec(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn health() -> &'static str {
    "ok"
}
