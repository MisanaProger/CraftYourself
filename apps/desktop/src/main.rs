//! Craft Yourself desktop app.
//!
//! One process: the same local daemon as `est serve` (REST, WebSocket,
//! sandbox, database in `~/.est`) listens on a random loopback port, and a
//! Tauri window shows the interface served by it. Keeping one origin means
//! the Host/Origin guard, the WebSocket and the sandboxed learning-artifact
//! iframes work exactly as in the browser build. The window gets no Tauri
//! IPC: everything goes through the daemon's API.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::SocketAddr;

use axum::Router;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use est_api::{Config, Runtime};
use est_workspace::EstHome;
use include_dir::{Dir, include_dir};
use tauri::webview::DownloadEvent;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use url::Url;

static FRONTEND: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../frontend/dist");

fn mime(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("json") => "application/json",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    }
}

/// Serves the embedded single-page app; unknown paths get `index.html`.
async fn frontend(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let file = FRONTEND
        .get_file(path)
        .filter(|_| !path.is_empty())
        .map(|f| (f, path))
        .or_else(|| FRONTEND.get_file("index.html").map(|f| (f, "index.html")));
    match file {
        Some((f, p)) => {
            let cache = if p.starts_with("assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            (
                [
                    (header::CONTENT_TYPE, mime(p)),
                    (header::CACHE_CONTROL, cache),
                ],
                f.contents(),
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Binds the port used by the previous launch when it is free, so the
/// window keeps the same origin (language and theme live in its storage);
/// otherwise a new free loopback port, remembered for next time.
async fn bind_stable(home: &EstHome) -> anyhow::Result<tokio::net::TcpListener> {
    let file = home.config_dir().join("desktop-port");
    let saved = std::fs::read_to_string(&file)
        .ok()
        .and_then(|s| s.trim().parse::<u16>().ok());
    if let Some(port) = saved.filter(|p| *p >= 1024)
        && let Ok(l) = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))).await
    {
        return Ok(l);
    }
    let l = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
    let _ = std::fs::write(&file, l.local_addr()?.port().to_string());
    Ok(l)
}

/// Starts the daemon on a loopback port and returns its address.
async fn start_daemon() -> anyhow::Result<SocketAddr> {
    let home = EstHome::from_env();
    home.init()?;
    let mut config = Config::load(&home.config_file())?;
    let listener = bind_stable(&home).await?;
    let addr = listener.local_addr()?;
    // The guard accepts only this exact Host.
    config.bind = addr;
    let rt = Runtime::build(home, config).await?;
    let app = est_api::router_with_fallback(rt, Router::new().fallback(frontend));
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("daemon stopped: {e}");
        }
    });
    Ok(addr)
}

/// External links open in the system browser, never inside the app window.
const INIT_SCRIPT: &str = r#"
window.__CRAFT_DESKTOP__ = true;
document.addEventListener("click", (e) => {
  const a = e.target instanceof Element ? e.target.closest("a[href]") : null;
  if (!a) return;
  const url = new URL(a.href, location.href);
  if (url.origin !== location.origin && (url.protocol === "https:" || url.protocol === "http:")) {
    e.preventDefault();
    location.assign(url.href); // intercepted by on_navigation → system browser
  }
}, true);
"#;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let tokio = tokio::runtime::Runtime::new()?;
    let addr = tokio.block_on(start_daemon())?;
    let origin = Url::parse(&format!("http://{addr}/"))?;
    tracing::info!("Craft Yourself daemon on {origin}");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let own = origin.clone();
            let downloads = app.path().download_dir().ok();
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(origin.clone()))
                .title("Craft Yourself")
                .inner_size(1440.0, 900.0)
                .min_inner_size(1024.0, 680.0)
                .initialization_script(INIT_SCRIPT)
                .on_navigation(move |url| {
                    if url.origin() == own.origin()
                        || url.scheme() == "about"
                        || url.scheme() == "blob"
                    {
                        return true;
                    }
                    if matches!(url.scheme(), "http" | "https") {
                        let _ = tauri_plugin_opener::open_url(url.as_str(), None::<&str>);
                    }
                    false
                })
                .on_download(move |_, event| {
                    // Exports are saved to the user's Downloads folder.
                    if let DownloadEvent::Requested { destination, .. } = event
                        && let Some(dir) = &downloads
                    {
                        let name = destination
                            .file_name()
                            .map(|n| n.to_owned())
                            .unwrap_or_else(|| "craft-yourself-export.json".into());
                        *destination = dir.join(name);
                    }
                    true
                })
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())?;
    drop(tokio);
    Ok(())
}
