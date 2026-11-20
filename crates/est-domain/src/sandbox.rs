//! Sandbox runs (architecture v0.3, §17): build, test and check commands
//! executed in an isolated container, never with the daemon's privileges.

use est_core::{SessionId, TaskId, Timestamp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    #[default]
    Test,
    Build,
    Check,
    /// Fetching dependencies; needs network and therefore explicit permission.
    Setup,
    /// A command typed by the learner.
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Passed,
    Failed,
    TimedOut,
    /// The sandbox could not run the command (no runtime, image pull failed…).
    Error,
}

/// Counts parsed from a test runner's output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TestSummary {
    pub passed: u32,
    pub failed: u32,
    pub ignored: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxRun {
    pub id: String,
    pub session_id: SessionId,
    pub task_id: TaskId,
    pub kind: RunKind,
    pub command: String,
    /// Relative to the workspace root.
    pub workdir: String,
    pub image: Option<String>,
    pub network: bool,
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    pub summary: Option<TestSummary>,
    pub duration_ms: Option<u64>,
    /// Captured output (head and tail when long).
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
    pub started_at: Timestamp,
    pub finished_at: Option<Timestamp>,
}
