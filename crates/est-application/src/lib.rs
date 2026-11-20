//! Application layer (architecture v0.2, §2.1, §4).
//!
//! Defines the use cases of EST and the *ports* (traits) they need. The
//! infrastructure crates (`est-storage`, `est-git`, `est-projects`,
//! `est-workspace`, …) implement the ports; `est-api` and `est-cli` call
//! the use cases. Nothing here knows about SQLite, Axum or a concrete model.

mod error;
pub mod ports;
mod service;

pub use error::{AppError, AppResult};
pub use service::{
    AddProject, App, AppSettings, MentorAnswer, MentorAsk, Ports, SessionOverview, StartedSession,
    name_from_source,
};
