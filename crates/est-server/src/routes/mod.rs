use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::state::AppState;

mod exercises;
mod mentor;
mod projects;
mod sessions;

pub fn api() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/me", get(me))
        .route("/projects", get(projects::list).post(projects::create))
        .route("/projects/{id}", get(projects::get))
        .route("/projects/{id}/files", get(projects::files))
        .route("/exercises", get(exercises::list).post(exercises::create))
        .route("/exercises/{id}", get(exercises::get))
        .route("/sessions", get(sessions::list).post(sessions::create))
        .route("/sessions/{id}", get(sessions::get))
        .route("/sessions/{id}/status", post(sessions::set_status))
        .route("/sessions/{id}/events", get(sessions::events))
        .route("/sessions/{id}/files", get(sessions::list_files))
        .route(
            "/sessions/{id}/files/{*path}",
            get(sessions::read_file).put(sessions::write_file),
        )
        .route("/sessions/{id}/git/status", get(sessions::git_status))
        .route("/sessions/{id}/git/diff", get(sessions::git_diff))
        .route("/sessions/{id}/git/log", get(sessions::git_log))
        .route("/sessions/{id}/git/branches", get(sessions::git_branches))
        .route("/sessions/{id}/git/add", post(sessions::git_add))
        .route("/sessions/{id}/git/commit", post(sessions::git_commit))
        .route("/sessions/{id}/mentor/route", post(mentor::route))
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

async fn me(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<est_storage::User> {
    Json((*state.user).clone())
}
