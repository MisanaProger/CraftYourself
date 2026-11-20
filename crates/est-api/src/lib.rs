//! EST daemon: HTTP API (`/api/v1`), WebSocket events and the composition
//! root that wires infrastructure adapters into the application.

pub mod config;
mod error;
mod guard;
pub mod llm;
mod routes;
pub mod runtime;

use std::path::Path;

use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

pub use config::Config;
pub use runtime::Runtime;

/// The full HTTP application: API, WebSocket and (optionally) the built
/// React frontend.
pub fn router(rt: Runtime, frontend_dir: Option<&Path>) -> Router {
    let allowed_hosts = rt.config.allowed_hosts();
    let mut app = Router::new().nest("/api/v1", routes::api()).with_state(rt);
    if let Some(dir) = frontend_dir.filter(|d| d.join("index.html").is_file()) {
        app = app
            .fallback_service(ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html"))));
    }
    app.layer(axum::middleware::from_fn_with_state(
        allowed_hosts,
        guard::local_only,
    ))
}

/// API and WebSocket plus a custom service for everything else — used by the
/// desktop app to serve the frontend embedded in its binary. The Host/Origin
/// guard covers both.
pub fn router_with_fallback(rt: Runtime, fallback: Router) -> Router {
    let allowed_hosts = rt.config.allowed_hosts();
    Router::new()
        .nest("/api/v1", routes::api())
        .with_state(rt)
        .fallback_service(fallback)
        .layer(axum::middleware::from_fn_with_state(
            allowed_hosts,
            guard::local_only,
        ))
}

/// Serves the daemon until Ctrl+C.
pub async fn serve(
    listener: tokio::net::TcpListener,
    rt: Runtime,
    frontend_dir: Option<&Path>,
) -> anyhow::Result<()> {
    axum::serve(listener, router(rt, frontend_dir))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
