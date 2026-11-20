use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::runtime::Runtime;

mod analytics;
mod conversations;
mod isolated;
mod knowledge;
mod learning;
mod llm;
mod problems;
mod projects;
mod recommendations;
mod remote;
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
        .route("/tasks/{id}/complete", post(tasks::complete))
        .route(
            "/tasks/{id}/acceptance-criteria",
            axum::routing::put(tasks::set_criteria),
        )
        .route("/tasks/{id}/conversation", get(tasks::conversation))
        .route("/exercises/import", post(tasks::import_exercise))
        .route("/sessions", get(sessions::list))
        .route("/sessions/{id}", get(sessions::get))
        .route("/sessions/{id}/finish", post(sessions::finish))
        .route("/sessions/{id}/diff", get(sessions::diff))
        .route("/sessions/{id}/events", get(sessions::events))
        .route("/sessions/{id}/summary", get(knowledge::session_summary))
        .route("/knowledge/profile", get(knowledge::profile))
        .route(
            "/sessions/{id}/workspace/open",
            post(sessions::open_workspace),
        )
        .route(
            "/conversations",
            get(conversations::list).post(conversations::create),
        )
        .route(
            "/conversations/{id}",
            get(conversations::get).delete(conversations::delete),
        )
        .route(
            "/conversations/{id}/messages",
            get(conversations::messages).post(conversations::post_message),
        )
        .route(
            "/conversations/{id}/messages/{message}/pages/{n}",
            get(conversations::page),
        )
        .route(
            "/conversations/{id}/engagement",
            get(conversations::engagement),
        )
        .route(
            "/conversations/{id}/engagement/raise",
            post(conversations::raise),
        )
        .route(
            "/engagement/escalations/{id}/accept",
            post(conversations::accept),
        )
        .route(
            "/engagement/escalations/{id}/reject",
            post(conversations::reject),
        )
        .route("/sandbox", get(sessions::sandbox_info))
        .route("/sandbox/runs/{id}", get(sessions::run))
        .route(
            "/sessions/{id}/sandbox/runs",
            get(sessions::runs).post(sessions::start_run),
        )
        .route("/learning/programs", get(learning::programs))
        .route(
            "/learning/generated",
            get(learning::generated).post(learning::generate),
        )
        .route(
            "/learning/generated/{id}",
            get(learning::generated_one).delete(learning::delete_generated),
        )
        .route(
            "/learning/programs/{id}/lessons/{lesson}/page",
            get(learning::page),
        )
        .route("/learning/programs/{id}", get(learning::program))
        .route(
            "/learning/programs/{id}/lessons/{lesson}",
            get(learning::lesson),
        )
        .route(
            "/learning/programs/{id}/lessons/{lesson}/complete",
            post(learning::complete),
        )
        .route(
            "/learning/programs/{id}/lessons/{lesson}/quiz",
            post(learning::quiz),
        )
        .route(
            "/learning/programs/{id}/lessons/{lesson}/progress",
            post(learning::artifact_progress),
        )
        .route(
            "/learning/programs/{id}/lessons/{lesson}/practice",
            post(learning::practice),
        )
        .route(
            "/learning/artifacts/{id}/{*path}",
            get(learning::artifact_file),
        )
        .route("/problems", get(problems::list).post(problems::import))
        .route("/problems/fetch", post(remote::fetch_problem))
        .route("/issues/preview", post(remote::preview_issue))
        .route("/issues/import", post(remote::import_issue))
        .route("/tasks/{id}/issue/comments", post(remote::draft_comment))
        .route(
            "/publish/drafts/{id}",
            axum::routing::delete(remote::discard),
        )
        .route("/publish/drafts/{id}/confirm", post(remote::confirm))
        .route(
            "/recommendations",
            get(recommendations::latest).post(recommendations::generate),
        )
        .route("/recommendations/activity", get(recommendations::activity))
        .route("/analytics/adi", get(analytics::adi))
        .route("/analytics/adi/history", get(analytics::adi_history))
        .route("/analytics/disciplines", get(analytics::disciplines))
        .route("/analytics/skills", get(analytics::skills))
        .route("/data/export", post(analytics::export))
        .route("/settings/privacy", get(analytics::privacy))
        .route("/llm/presets", get(llm::presets))
        .route("/llm/providers", get(llm::list).post(llm::add))
        .route(
            "/llm/providers/{id}",
            axum::routing::patch(llm::update).delete(llm::remove),
        )
        .route("/llm/active", post(llm::set_active))
        .route("/llm/models", post(llm::models))
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
