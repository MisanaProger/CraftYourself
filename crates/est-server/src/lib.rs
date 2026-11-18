//! HTTP API of the Engineering Skills Trainer (Local Edition).
//!
//! The server is the composition root of the modular monolith: it wires the
//! storage, git, exercise and policy subsystems together and exposes them as
//! a REST API under `/api`.

pub mod config;
pub mod error;
pub mod inspect;
pub mod routes;
pub mod state;

use axum::Router;

pub use config::Config;
pub use state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new().nest("/api", routes::api()).with_state(state)
}
