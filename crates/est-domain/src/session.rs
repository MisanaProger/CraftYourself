use std::path::PathBuf;

use est_core::{ProjectId, SessionId, TaskId, Timestamp, UserId};
use serde::{Deserialize, Serialize};

use crate::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionStatus {
    Active,
    Completed,
    Abandoned,
}

impl SessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionStatus::Active => "active",
            SessionStatus::Completed => "completed",
            SessionStatus::Abandoned => "abandoned",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "active" => SessionStatus::Active,
            "completed" => SessionStatus::Completed,
            "abandoned" => SessionStatus::Abandoned,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionOutcome {
    Completed,
    Abandoned,
}

/// Links a learner, a project, a task, mentor interactions and check results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearningSession {
    pub id: SessionId,
    pub user_id: UserId,
    pub project_id: ProjectId,
    pub task_id: TaskId,
    /// The task's own working directory, opened in the learner's IDE.
    pub workspace_path: PathBuf,
    pub branch: String,
    /// Commit the work started from; `base..HEAD` is the learner's work.
    pub base_revision: String,
    pub status: SessionStatus,
    pub started_at: Timestamp,
    pub finished_at: Option<Timestamp>,
}

impl LearningSession {
    pub fn is_active(&self) -> bool {
        self.status == SessionStatus::Active
    }

    pub fn finish(&mut self, outcome: SessionOutcome, at: Timestamp) -> Result<(), DomainError> {
        if !self.is_active() {
            return Err(DomainError::InvalidTransition(format!(
                "session is already {}",
                self.status.as_str()
            )));
        }
        self.status = match outcome {
            SessionOutcome::Completed => SessionStatus::Completed,
            SessionOutcome::Abandoned => SessionStatus::Abandoned,
        };
        self.finished_at = Some(at);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> LearningSession {
        LearningSession {
            id: SessionId::generate(),
            user_id: UserId::generate(),
            project_id: ProjectId::generate(),
            task_id: TaskId::generate(),
            workspace_path: "/tmp/ws".into(),
            branch: "est/x".into(),
            base_revision: "a".repeat(40),
            status: SessionStatus::Active,
            started_at: est_core::now(),
            finished_at: None,
        }
    }

    #[test]
    fn finishing_is_one_way() {
        let mut s = session();
        s.finish(SessionOutcome::Completed, est_core::now())
            .unwrap();
        assert_eq!(s.status, SessionStatus::Completed);
        assert!(s.finished_at.is_some());
        assert!(
            s.finish(SessionOutcome::Abandoned, est_core::now())
                .is_err()
        );
    }
}
