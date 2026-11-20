//! Domain layer: models and business rules.
//!
//! This crate must not depend on storage, HTTP, Git, containers or any AI
//! provider (architecture v0.3, §4.1).

mod conversation;
mod engagement;
mod language;
mod project;
mod sandbox;
mod session;
mod task;

pub use conversation::{Card, Conversation, ConversationStatus, Message, MessageRole, SourceLink};
pub use engagement::{EngagementLevel, EngagementState, Escalation, EscalationStatus};
pub use language::{CommandKind, CommandSpec, LanguageDetection};
pub use project::{NewProject, Project};
pub use sandbox::{RunKind, RunStatus, SandboxRun, TestSummary};
pub use session::{LearningSession, SessionOutcome, SessionStatus};
pub use task::{
    AcceptanceCriterion, Difficulty, DocReference, MentorSettings, NewTask, Task, TaskSource,
    TaskStatus, TaskType,
};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid value: {0}")]
    Invalid(String),
    #[error("{0}")]
    InvalidTransition(String),
}
