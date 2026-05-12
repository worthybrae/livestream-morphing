use axum::{
    extract::{Path, State},
    http::{header, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::api;
use crate::pipeline::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods([Method::GET, Method::PUT, Method::PATCH, Method::POST, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE]);

    Router::new()
        .route("/api/stream", get(stream_playlist))
        .route("/api/segments/{segment_id}", get(get_segment))
        .route("/health", get(health))
        .merge(api::api_router())
        .layer(cors)
        .fallback_service(
            ServeDir::new("../studio/dist")
                .not_found_service(ServeFile::new("../studio/dist/index.html")),
        )
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
            (header::CONTENT_TYPE, "application/vnd.apple.mpegurl"),
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
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
    {
        return StatusCode::BAD_REQUEST.into_response();
    }

    // Strip .ts extension — the playlist generates URLs like /api/segments/12345.ts
    let id = segment_id.strip_suffix(".ts").unwrap_or(&segment_id);

    let data = {
        let buf = state.hls_buffer.read().await;
        buf.get_segment(id).map(|d| d.to_vec())
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
