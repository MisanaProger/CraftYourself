//! Git workflow review (spec v0.3, stage 9). Pure: no Git, no network, no
//! model. Fixed rules give findings with stable codes (translated by the
//! interface) and a score; nothing here writes the learner's text for them.
//!
//! * [`commits`] — commit messages and sizes, checked against general rules
//!   and the conventions learned from the repository's own history;
//! * [`pull_request`] — a pull request draft assembled from observed data
//!   (commits, acceptance criteria, the last test run) and its readiness;
//! * [`issue_text`] — a checklist for a bug report or a feature request.

pub mod commits;
pub mod issue_text;
pub mod pull_request;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Problem,
}

/// One observation; `code` is translated by the interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Finding {
    pub fn new(code: &str, severity: Severity, detail: Option<String>) -> Self {
        Self {
            code: code.into(),
            severity,
            detail,
        }
    }
}
