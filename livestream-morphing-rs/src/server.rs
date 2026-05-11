use axum::{
    extract::{Path, State},
    http::{header, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::pipeline::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods([Method::GET]);

    Router::new()
        .route("/api/stream", get(stream_playlist))
        .route("/api/segments/{segment_id}.ts", get(get_segment))
        .route("/health", get(health))
        .layer(cors)
        .with_state(state)
}

async fn stream_playlist(State(state): State<Arc<AppState>>) -> Response {
    state.touch();

    let playlist = {
        let buf = state.hls_buffer.read().await;
        buf.generate_playlist()
    };

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

    if segment_id.len() > 64
        || !segment_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let data = {
        let buf = state.hls_buffer.read().await;
        buf.get_segment(&segment_id).map(|d| d.to_vec())
    };

    match data {
        Some(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "video/mp2t"),
                (header::CACHE_CONTROL, "max-age=3600"),
            ],
            bytes,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn health() -> &'static str {
    "ok"
}
