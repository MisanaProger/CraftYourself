//! Application layer (architecture v0.2, §2.1, §4).
//!
//! Defines the use cases of EST and the *ports* (traits) they need. The
//! infrastructure crates (`est-storage`, `est-git`, `est-projects`,
//! `est-workspace`, …) implement the ports; `est-api` and `est-cli` call
//! the use cases. Nothing here knows about SQLite, Axum or a concrete model.

mod analytics;
mod conversation;
mod error;
pub mod generation;
pub mod knowledge;
pub mod learning;
pub mod ports;
pub mod problems;
pub mod recommendations;
pub mod sandbox;
mod service;
pub mod skills;

pub use analytics::{AdiHistoryItem, AdiOverview, PrivacySettings, SkillMatrix};
pub use conversation::{ConversationView, NewConversation, PostMessage};
pub use error::{AppError, AppResult};
pub use generation::{GenerateProgram, GeneratedProgramSummary};
pub use knowledge::KnowledgeProfileView;
pub use problems::{ImportProblem, ProblemSummary};
pub use recommendations::RecommendRequest;
pub use sandbox::RunRequest;
pub use service::{
    AddProject, App, AppSettings, Ports, SessionOverview, StartedSession, name_from_source,
};
