//! Conversations and messages (architecture v0.3, §5.3, §6).
//!
//! Messages store content and structured cards separately from how the UI
//! renders them.

use est_core::{ConversationId, MessageId, ProjectId, TaskId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::EngagementLevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationStatus {
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    pub id: ConversationId,
    pub project_id: Option<ProjectId>,
    pub task_id: Option<TaskId>,
    pub title: String,
    pub status: ConversationStatus,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    /// Notices produced by EST itself (policy, escalation, events).
    System,
}

/// A link to a primary source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLink {
    /// `source`, `curated` or `reference_search`.
    pub kind: String,
    pub title: String,
    /// URL or `path:line`.
    pub target: String,
}

/// Structured blocks attached to a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Card {
    /// Documentation Boundary: read these sources.
    Sources {
        symbols: Vec<String>,
        links: Vec<SourceLink>,
    },
    /// A proposal to raise the engagement level by one step.
    Escalation {
        escalation_id: String,
        from: EngagementLevel,
        to: EngagementLevel,
        unlocks: String,
        status: String,
    },
    /// Something the policy removed or limited in the answer.
    PolicyNotice {
        text: String,
        /// Stable code for localized rendering (`code_removed`, `capped`).
        #[serde(default)]
        code: Option<String>,
    },
    /// Result of a sandbox run, shown in the timeline.
    SandboxRun {
        run_id: String,
        kind: crate::RunKind,
        command: String,
        status: crate::RunStatus,
        summary: Option<crate::TestSummary>,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub conversation_id: ConversationId,
    pub role: MessageRole,
    /// Markdown text.
    pub content: String,
    pub cards: Vec<Card>,
    /// Engagement level the message was produced under.
    pub level: EngagementLevel,
    /// Structured metadata (policy decision, provider, …).
    pub meta: serde_json::Value,
    pub created_at: Timestamp,
}

impl Message {
    pub fn new(
        conversation_id: &ConversationId,
        role: MessageRole,
        content: impl Into<String>,
        level: EngagementLevel,
    ) -> Self {
        Self {
            id: MessageId::generate(),
            conversation_id: conversation_id.clone(),
            role,
            content: content.into(),
            cards: Vec::new(),
            level,
            meta: serde_json::Value::Null,
            created_at: est_core::now(),
        }
    }

    pub fn with_card(mut self, card: Card) -> Self {
        self.cards.push(card);
        self
    }

    pub fn with_meta(mut self, meta: serde_json::Value) -> Self {
        self.meta = meta;
        self
    }
}
