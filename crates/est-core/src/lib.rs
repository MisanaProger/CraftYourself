//! Shared primitives used by every EST crate: typed identifiers, timestamps
//! and domain events. No business rules live here.

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type Timestamp = DateTime<Utc>;

pub fn now() -> Timestamp {
    Utc::now()
}

macro_rules! id_type {
    ($($(#[$meta:meta])* $name:ident),* $(,)?) => {$(
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// A fresh random identifier.
            pub fn generate() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// First 8 characters, for human-facing labels.
            pub fn short(&self) -> &str {
                &self.0[..self.0.len().min(8)]
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_owned())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    )*};
}

id_type!(
    /// Learner account (the implicit local user in Local Edition).
    UserId,
    /// Connected Git repository.
    ProjectId,
    /// Learning task inside a project.
    TaskId,
    /// One attempt at a task in its own workspace.
    SessionId,
    /// Long-lived dialogue, usually bound to one Issue.
    ConversationId,
    MessageId,
    /// Proposal to raise the engagement level.
    EscalationId,
);

/// Something that happened, recorded in the session event log and pushed to
/// clients over WebSocket. Assessment is built from these observed actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainEvent {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ProjectId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<ConversationId>,
    pub payload: serde_json::Value,
    pub at: Timestamp,
}

impl DomainEvent {
    pub fn new(kind: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            kind: kind.into(),
            session_id: None,
            project_id: None,
            task_id: None,
            conversation_id: None,
            payload,
            at: now(),
        }
    }

    pub fn session(mut self, id: &SessionId) -> Self {
        self.session_id = Some(id.clone());
        self
    }

    pub fn project(mut self, id: &ProjectId) -> Self {
        self.project_id = Some(id.clone());
        self
    }

    pub fn task(mut self, id: &TaskId) -> Self {
        self.task_id = Some(id.clone());
        self
    }

    pub fn conversation(mut self, id: &ConversationId) -> Self {
        self.conversation_id = Some(id.clone());
        self
    }
}

/// Well-known event kinds.
pub mod events {
    pub const PROJECT_ADDED: &str = "project.added";
    pub const TASK_CREATED: &str = "task.created";
    pub const SESSION_STARTED: &str = "session.started";
    pub const SESSION_RESUMED: &str = "session.resumed";
    pub const SESSION_FINISHED: &str = "session.finished";
    pub const WORKSPACE_OPENED: &str = "workspace.opened";
    pub const TASK_COMPLETED: &str = "task.completed";
    pub const CONVERSATION_CREATED: &str = "conversation.created";
    pub const MESSAGE_CREATED: &str = "conversation.message.created";
    pub const CONVERSATION_DELETED: &str = "conversation.deleted";
    pub const DOCS_REDIRECT: &str = "documentation.redirect";
    pub const AI_ANSWER: &str = "ai.answer";
    /// The learner connected, switched or disconnected an AI provider.
    pub const AI_PROVIDER_CHANGED: &str = "ai.provider.changed";
    pub const ENGAGEMENT_INITIALIZED: &str = "engagement.level.initialized";
    pub const ESCALATION_PROPOSED: &str = "engagement.escalation.proposed";
    pub const ESCALATION_ACCEPTED: &str = "engagement.escalation.accepted";
    pub const ESCALATION_REJECTED: &str = "engagement.escalation.rejected";
    pub const LEVEL_CHANGED: &str = "engagement.level.changed";
    pub const ISSUE_RESET: &str = "engagement.issue.reset";
    pub const ADI_ASSESSED: &str = "adi.assessed";
}

/// Lowercase ASCII slug: letters, digits and single dashes.
pub fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Turns arbitrary text into a slug of at most `max` characters.
pub fn slugify(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
        if out.len() >= max {
            break;
        }
    }
    out.trim_end_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        let a = TaskId::generate();
        assert_ne!(a, TaskId::generate());
        assert_eq!(a.short().len(), 8);
        assert_eq!(serde_json::to_string(&TaskId::from("x")).unwrap(), "\"x\"");
    }

    #[test]
    fn slugs() {
        assert!(is_slug("fix-unicode-2"));
        assert!(!is_slug("-bad") && !is_slug("Bad") && !is_slug("a--b"));
        assert_eq!(
            slugify("Fix: off-by-one in Range!", 40),
            "fix-off-by-one-in-range"
        );
        assert_eq!(slugify("Исправить ошибку", 40), "");
        assert_eq!(slugify("abcdef ghi", 6), "abcdef");
    }
}
