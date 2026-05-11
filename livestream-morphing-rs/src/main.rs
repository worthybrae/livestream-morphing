mod codec;
mod effects;
mod hls;
mod pipeline;
mod registry;
mod server;
mod stream_source;
mod time_color;

use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "livestream_morphing_rs=info".into()),
        )
        .init();

    // Initialize FFmpeg
    codec::init();

    // Create shared state
    let (state, active_rx) = pipeline::AppState::new();

    // Start pipeline in background
    let pipeline_state = state.clone();
    let pipeline_handle = tokio::spawn(async move {
        pipeline::run(pipeline_state, active_rx).await;
    });

    // Start HTTP server
    let app = server::router(state);
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!(%addr, "Server starting");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| {
        tracing::error!(%addr, error = %e, "Failed to bind TCP listener");
        std::process::exit(1);
    });

    // Race server and pipeline — if either exits, shut down
    tokio::select! {
        res = axum::serve(listener, app) => {
            if let Err(e) = res {
                tracing::error!(error = %e, "Server error");
            }
        }
        _ = pipeline_handle => {
            tracing::error!("Pipeline exited unexpectedly — shutting down");
        }
    }
}
