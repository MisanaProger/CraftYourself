use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::runtime::Runtime;

mod projects;
mod sessions;
mod tasks;

pub fn api() -> Router<Runtime> {
    Router::new()
        .route("/health", get(health))
        .route("/me", get(me))
        .route("/events", get(events_ws))
        .route("/projects", get(projects::list).post(projects::create))
        .route("/projects/{id}", get(projects::get))
        .route("/projects/{id}/files", get(projects::files))
        .route("/projects/{id}/commands", get(projects::commands))
        .route("/tasks", get(tasks::list).post(tasks::create))
        .route("/tasks/{id}", get(tasks::get))
        .route("/tasks/{id}/start", post(tasks::start))
        .route("/exercises/import", post(tasks::import_exercise))
        .route("/sessions", get(sessions::list))
        .route("/sessions/{id}", get(sessions::get))
        .route("/sessions/{id}/finish", post(sessions::finish))
        .route("/sessions/{id}/diff", get(sessions::diff))
        .route("/sessions/{id}/events", get(sessions::events))
        .route(
            "/sessions/{id}/workspace/open",
            post(sessions::open_workspace),
        )
        .route("/sessions/{id}/mentor/ask", post(sessions::mentor_ask))
        .route(
            "/sessions/{id}/mentor/history",
            get(sessions::mentor_history),
        )
}

async fn health(State(rt): State<Runtime>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "api": "v1",
        "home": rt.home.root(),
    }))
}

async fn me(State(rt): State<Runtime>) -> Json<est_application::ports::User> {
    Json(rt.app.user().clone())
}

/// Pushes every domain event to the client as JSON.
async fn events_ws(State(rt): State<Runtime>, ws: WebSocketUpgrade) -> Response {
    let mut rx = rt.events.subscribe();
    ws.on_upgrade(move |mut socket: WebSocket| async move {
        loop {
            tokio::select! {
                event = rx.recv() => match event {
                    Ok(e) => {
                        let text = serde_json::to_string(&e).unwrap_or_default();
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                },
                incoming = socket.recv() => match incoming {
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => {}
                },
            }
        }
    })
}
