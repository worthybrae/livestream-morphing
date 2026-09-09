use axum::{
    extract::{Path, State},
    http::{header, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

use crate::api;
use crate::pipeline::AppState;

pub struct ServerConfig {
    pub public_mode: bool,
    pub allowed_origin: HeaderValue,
}

impl ServerConfig {
    fn from_env() -> Self {
        let public_mode = std::env::var("PUBLIC_MODE").as_deref() == Ok("true");
        let default_origin = if public_mode {
            "https://worthyrae.com"
        } else {
            "http://localhost:5176"
        };
        let allowed_origin = std::env::var("ALLOWED_ORIGIN")
            .unwrap_or_else(|_| default_origin.into())
            .parse::<HeaderValue>()
            .expect("ALLOWED_ORIGIN must be a valid HTTP header value");

        Self {
            public_mode,
            allowed_origin,
        }
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    router_with_config(state, ServerConfig::from_env())
}

pub fn router_with_config(state: Arc<AppState>, config: ServerConfig) -> Router {
    let allowed_methods = if config.public_mode {
        vec![Method::GET]
    } else {
        vec![
            Method::GET,
            Method::PUT,
            Method::PATCH,
            Method::POST,
            Method::DELETE,
        ]
    };
    let cors = CorsLayer::new()
        .allow_origin([config.allowed_origin])
        .allow_methods(allowed_methods)
        .allow_headers([header::CONTENT_TYPE]);

    let public = Router::new()
        .route("/api/live/status", get(live_status))
        .route("/api/stream", get(stream_playlist))
        .route("/api/segments/{segment_id}", get(get_segment))
        .route("/health", get(health));

    let app = if config.public_mode {
        public
    } else {
        public
            .merge(api::api_router())
            .fallback_service(
                ServeDir::new("../studio/dist")
                    .not_found_service(ServeFile::new("../studio/dist/index.html")),
            )
    };

    app.layer(cors)
        .with_state(state)
}

async fn live_status(State(state): State<Arc<AppState>>) -> Response {
    state.touch();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time must be after the Unix epoch")
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    let status = state.live_status(now_ms).await;

    ([(header::CACHE_CONTROL, "no-store")], Json(status)).into_response()
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
            (header::CACHE_CONTROL, "no-store"),
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
                (
                    header::CACHE_CONTROL,
                    "public, max-age=3600, immutable",
                ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{HeaderValue, Request},
    };
    use tower::ServiceExt;

    fn test_config(public_mode: bool) -> ServerConfig {
        ServerConfig {
            public_mode,
            allowed_origin: HeaderValue::from_static("https://worthyrae.com"),
        }
    }

    #[tokio::test]
    async fn public_status_wakes_pipeline_and_is_not_cached() {
        let (state, rx) = AppState::new();
        let app = router_with_config(state, test_config(true));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/live/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(*rx.borrow());
    }

    #[tokio::test]
    async fn public_status_returns_live_status_json() {
        let (state, _rx) = AppState::new();
        let app = router_with_config(state, test_config(true));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/live/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "state": "starting",
                "buffer_count": 0,
                "ready": false,
                "last_segment_at": 0,
            })
        );
    }

    #[tokio::test]
    async fn public_router_does_not_mount_mutation_routes() {
        let (state, _rx) = AppState::new();
        let app = router_with_config(state, test_config(true));

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/source")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"url":"https://example.com"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(matches!(
            response.status(),
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
        ));
    }

    #[tokio::test]
    async fn successful_segments_are_public_and_immutable() {
        let (state, _rx) = AppState::new();
        state
            .hls_buffer
            .write()
            .await
            .push_segment("1000-44".into(), vec![1, 2, 3]);
        let app = router_with_config(state, test_config(true));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/segments/1000-44.ts")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=3600, immutable"
        );
    }

    #[tokio::test]
    async fn playlist_is_not_cached() {
        let (state, _rx) = AppState::new();
        let app = router_with_config(state, test_config(true));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/stream")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn cors_allows_only_the_configured_origin() {
        let (state, _rx) = AppState::new();
        let app = router_with_config(state, test_config(true));

        let allowed = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header(header::ORIGIN, "https://worthyrae.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let rejected = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header(header::ORIGIN, "https://attacker.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            allowed.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            "https://worthyrae.com"
        );
        assert!(!rejected
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            "unexpected origin header: {:?}",
            rejected
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        );
    }
}
