//! Domain layer: models and business rules.
//!
//! This crate must not depend on storage, HTTP, Git or any AI provider
//! (architecture v0.2, §4).

mod help;
mod language;
mod project;
mod session;
mod skill;
mod task;

pub use help::HelpLevel;
pub use language::{CommandKind, CommandSpec, LanguageDetection};
pub use project::{NewProject, Project};
pub use session::{LearningSession, SessionOutcome, SessionStatus};
pub use skill::Skill;
pub use task::{
    Difficulty, DocReference, MentorSettings, NewTask, Task, TaskSource, TaskStatus, TaskType,
};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid value: {0}")]
    Invalid(String),
    #[error("{0}")]
    InvalidTransition(String),
}
