use est_core::{ProjectId, TaskId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::{DomainError, EngagementLevel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskType {
    BugFix,
    Feature,
    Refactoring,
    Documentation,
    Performance,
    Testing,
    Regression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Difficulty {
    Beginner,
    Intermediate,
    Advanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    Open,
    InProgress,
    Done,
    Archived,
}

/// Where a task came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TaskSource {
    Manual,
    Exercise { id: String, version: String },
    Issue { url: String },
}

/// Mentor limits for one task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MentorSettings {
    /// Highest engagement level reachable by escalation in this task.
    pub max_level: EngagementLevel,
    /// Globs the mentor must never read, e.g. hidden reference tests.
    pub denied_paths: Vec<String>,
}

impl Default for MentorSettings {
    fn default() -> Self {
        Self {
            max_level: EngagementLevel::Active,
            denied_paths: Vec::new(),
        }
    }
}

/// Primary source curated by the task author.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocReference {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub topics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub project_id: ProjectId,
    pub title: String,
    pub description: String,
    pub task_type: TaskType,
    pub difficulty: Difficulty,
    pub source: TaskSource,
    pub status: TaskStatus,
    pub learning_goals: Vec<String>,
    /// Discipline ids from the ESM catalog this task exercises.
    pub disciplines: Vec<String>,
    /// Commit the task is pinned to; `None` = project HEAD at start time.
    pub base_revision: Option<String>,
    /// Command to run the task's tests (executed by the sandbox).
    pub test_command: Option<String>,
    pub mentor: MentorSettings,
    pub docs: Vec<DocReference>,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NewTask {
    pub project_id: ProjectId,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub task_type: TaskType,
    pub difficulty: Difficulty,
    #[serde(default = "manual")]
    pub source: TaskSource,
    #[serde(default)]
    pub learning_goals: Vec<String>,
    #[serde(default)]
    pub disciplines: Vec<String>,
    #[serde(default)]
    pub base_revision: Option<String>,
    #[serde(default)]
    pub test_command: Option<String>,
    #[serde(default)]
    pub mentor: Option<MentorSettings>,
    #[serde(default)]
    pub docs: Vec<DocReference>,
}

fn manual() -> TaskSource {
    TaskSource::Manual
}

impl NewTask {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.title.trim().is_empty() {
            return Err(DomainError::Invalid("task title must not be empty".into()));
        }
        if let Some(rev) = &self.base_revision
            && (rev.len() != 40 || !rev.chars().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(DomainError::Invalid(
                "base_revision must be a full 40-character commit hash".into(),
            ));
        }
        Ok(())
    }
}

impl Task {
    /// Status transition when a session for this task starts.
    pub fn start(&mut self) -> Result<(), DomainError> {
        match self.status {
            TaskStatus::Archived => Err(DomainError::InvalidTransition(
                "an archived task cannot be started".into(),
            )),
            _ => {
                self.status = TaskStatus::InProgress;
                Ok(())
            }
        }
    }
}
