//! Ports: what the application needs from the outside world.
//!
//! Repository ports are async; Git, filesystem and editor ports are
//! synchronous and are called from blocking worker threads by the services.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use est_core::{DomainEvent, ProjectId, SessionId, TaskId, UserId};
use est_domain::{
    CommandSpec, LanguageDetection, LearningSession, NewProject, NewTask, Project, Task, TaskStatus,
};
use serde::Serialize;

use crate::AppResult;

// ---- persistence ---------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: UserId,
    pub display_name: String,
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    /// The implicit local user; created on first call.
    async fn local_user(&self) -> AppResult<User>;
}

#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn create(&self, project: NewProject) -> AppResult<Project>;
    async fn get(&self, id: &ProjectId) -> AppResult<Option<Project>>;
    async fn find_by_path(&self, path: &Path) -> AppResult<Option<Project>>;
    async fn find_by_url(&self, url: &str) -> AppResult<Option<Project>>;
    async fn list(&self) -> AppResult<Vec<Project>>;
}

#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn create(&self, task: NewTask) -> AppResult<Task>;
    async fn get(&self, id: &TaskId) -> AppResult<Option<Task>>;
    async fn list(&self, project: Option<&ProjectId>) -> AppResult<Vec<Task>>;
    async fn set_status(&self, id: &TaskId, status: TaskStatus) -> AppResult<()>;
    async fn set_acceptance_criteria(
        &self,
        id: &TaskId,
        criteria: &[est_domain::AcceptanceCriterion],
    ) -> AppResult<()>;
    /// Task imported from exercise `exercise_id` at `version` into `project`.
    async fn find_exercise(
        &self,
        project: &ProjectId,
        exercise_id: &str,
        version: &str,
    ) -> AppResult<Option<Task>>;
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create(&self, session: &LearningSession) -> AppResult<()>;
    async fn update(&self, session: &LearningSession) -> AppResult<()>;
    async fn get(&self, id: &SessionId) -> AppResult<Option<LearningSession>>;
    async fn list(&self, user: &UserId) -> AppResult<Vec<LearningSession>>;
    async fn active_for_task(&self, task: &TaskId) -> AppResult<Option<LearningSession>>;
    /// The session whose workspace contains `path` (used by the CLI).
    async fn find_by_workspace(&self, path: &Path) -> AppResult<Option<LearningSession>>;
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredEvent {
    pub id: i64,
    #[serde(flatten)]
    pub event: DomainEvent,
}

#[async_trait]
pub trait EventLog: Send + Sync {
    async fn append(&self, event: &DomainEvent) -> AppResult<i64>;
    async fn for_session(
        &self,
        session: &SessionId,
        kind: Option<&str>,
    ) -> AppResult<Vec<StoredEvent>>;
    async fn for_conversation(
        &self,
        conversation: &est_core::ConversationId,
        kind: Option<&str>,
    ) -> AppResult<Vec<StoredEvent>>;
}

/// Pushes events to live clients (WebSocket). Best effort.
pub trait EventPublisher: Send + Sync {
    fn publish(&self, event: &DomainEvent);
}

pub struct NoopPublisher;

impl EventPublisher for NoopPublisher {
    fn publish(&self, _: &DomainEvent) {}
}

// ---- git -----------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FileStatus {
    pub code: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitSummary {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SourceMatch {
    pub path: String,
    pub line: u32,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffTarget<'a> {
    /// Unstaged changes.
    Worktree,
    /// Staged changes.
    Staged,
    /// Everything since the given commit, including uncommitted changes.
    Since(&'a str),
}

pub trait GitPort: Send + Sync {
    fn is_repository(&self, path: &Path) -> bool;
    /// Clones a remote repository (subject to the URL policy).
    fn clone_remote(&self, url: &str, dest: &Path) -> AppResult<()>;
    /// Clones a local repository EST already knows into a workspace.
    fn clone_local(&self, source: &Path, dest: &Path) -> AppResult<()>;
    fn remote_url(&self, repo: &Path) -> AppResult<Option<String>>;
    fn resolve(&self, repo: &Path, rev: &str) -> AppResult<Option<String>>;
    fn fetch(&self, repo: &Path) -> AppResult<()>;
    fn create_branch(&self, repo: &Path, name: &str, start: &str) -> AppResult<()>;
    fn current_branch(&self, repo: &Path) -> AppResult<Option<String>>;
    fn status(&self, repo: &Path) -> AppResult<Vec<FileStatus>>;
    fn diff(&self, repo: &Path, target: DiffTarget<'_>) -> AppResult<String>;
    fn log(&self, repo: &Path, since: Option<&str>, limit: usize) -> AppResult<Vec<CommitSummary>>;
    fn tracked_files(&self, repo: &Path) -> AppResult<Vec<String>>;
    fn grep(&self, repo: &Path, needle: &str, max: usize) -> AppResult<Vec<SourceMatch>>;
}

// ---- projects, languages, workspaces, editor ------------------------------

#[derive(Debug, Clone, Default)]
pub struct ProjectInspection {
    pub languages: Vec<LanguageDetection>,
    pub license: Option<String>,
}

pub trait ProjectInspector: Send + Sync {
    fn inspect(&self, root: &Path, tracked_files: &[String]) -> ProjectInspection;
    /// Build / test / check commands the language adapters suggest.
    fn commands(&self, root: &Path, tracked_files: &[String]) -> Vec<CommandSpec>;
    /// Pass/fail counts from a test runner's output, if recognisable.
    fn summarize_tests(&self, output: &str) -> Option<est_domain::TestSummary>;
}

pub trait WorkspaceLayout: Send + Sync {
    /// Directory for a task's workspace, e.g. `~/.est/workspaces/<project>/<task>`.
    fn workspace_for(&self, project: &Project, task: &Task) -> PathBuf;
    /// Directory for a clone of a remote project.
    fn project_cache(&self, project_name: &str) -> PathBuf;
}

pub trait IdeLauncher: Send + Sync {
    /// Opens `path` in the learner's editor. Returns the command used.
    fn open(&self, path: &Path) -> AppResult<String>;
}

// ---- conversations and engagement ------------------------------------------

use est_core::{ConversationId, EscalationId, MessageId};
use est_domain::{Card, Conversation, EngagementState, Escalation, Message};

#[async_trait]
pub trait ConversationRepository: Send + Sync {
    async fn create(&self, conversation: &Conversation) -> AppResult<()>;
    async fn get(&self, id: &ConversationId) -> AppResult<Option<Conversation>>;
    async fn list(&self) -> AppResult<Vec<Conversation>>;
    async fn for_task(&self, task: &TaskId) -> AppResult<Option<Conversation>>;
    /// Stores a message and bumps the conversation's `updated_at`, atomically.
    async fn append(&self, message: &Message) -> AppResult<()>;
    async fn message(&self, id: &MessageId) -> AppResult<Option<Message>>;
    /// Messages in chronological order; `limit` takes the most recent ones.
    async fn messages(&self, id: &ConversationId, limit: Option<usize>) -> AppResult<Vec<Message>>;
    /// Updates the structured cards of a message (e.g. escalation status).
    async fn update_cards(&self, id: &MessageId, cards: &[Card]) -> AppResult<()>;
    /// Deletes the conversation with its messages, escalations, engagement
    /// state, events and the ADI of its task (v0.3 §19.3).
    async fn delete(&self, id: &ConversationId) -> AppResult<()>;
}

#[async_trait]
pub trait EngagementRepository: Send + Sync {
    async fn state(&self, conversation: &ConversationId) -> AppResult<Option<EngagementState>>;
    async fn save_state(&self, state: &EngagementState) -> AppResult<()>;
    async fn save_escalation(&self, escalation: &Escalation) -> AppResult<()>;
    async fn escalation(&self, id: &EscalationId) -> AppResult<Option<Escalation>>;
    async fn escalations(&self, conversation: &ConversationId) -> AppResult<Vec<Escalation>>;
}

// ---- analytics -------------------------------------------------------------

/// Final ADI of one completed Issue, stored with its scoring version.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct AdiAssessment {
    pub task_id: TaskId,
    pub score: u8,
    pub scoring_version: String,
    pub observed_features: serde_json::Value,
    pub factors: serde_json::Value,
    pub explanation: String,
    pub confidence: String,
    pub created_at: est_core::Timestamp,
    /// Completion order, used to break timestamp ties.
    pub sequence: i64,
}

#[async_trait]
pub trait AdiRepository: Send + Sync {
    /// Replaces any earlier assessment of the same task.
    async fn save(&self, assessment: &AdiAssessment) -> AppResult<AdiAssessment>;
    async fn list(&self) -> AppResult<Vec<AdiAssessment>>;
}

/// Everything stored about the user, for export (v0.3 §20).
#[async_trait]
pub trait DataExporter: Send + Sync {
    async fn export_all(&self) -> AppResult<serde_json::Value>;
}

// ---- AI Gateway ------------------------------------------------------------

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct ChatMessage {
    /// `user` or `assistant`.
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub system: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub name: String,
    pub model: String,
    pub endpoint: String,
    /// Data leaves this machine.
    pub remote: bool,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn info(&self) -> ProviderInfo;
    async fn generate(&self, request: LlmRequest) -> AppResult<LlmResponse>;
}

// ---- sandbox ---------------------------------------------------------------

/// One command to execute in isolation.
#[derive(Debug, Clone)]
pub struct SandboxRequest {
    pub run_id: String,
    /// Host path of the workspace; mounted read-only and copied inside.
    pub workspace: PathBuf,
    /// Working directory relative to the workspace root.
    pub workdir: String,
    /// Shell command line, interpreted inside the container.
    pub command: String,
    /// Selects the container image.
    pub language: Option<String>,
    /// Network access; only with the learner's explicit permission.
    pub network: bool,
    /// Build cache shared by runs of one session (removed when it ends).
    pub cache_key: String,
    /// Dependency cache shared by sessions of one project.
    pub deps_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SandboxOutcome {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub image: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SandboxInfo {
    /// `docker` or `podman`, when one is usable.
    pub runtime: Option<String>,
    pub available: bool,
    /// Why the sandbox is unavailable, if it is.
    pub reason: Option<String>,
    pub limits: serde_json::Value,
}

#[async_trait]
pub trait Sandbox: Send + Sync {
    fn info(&self) -> SandboxInfo;
    async fn run(&self, request: SandboxRequest) -> AppResult<SandboxOutcome>;
    /// Removes the per-session build cache.
    async fn discard(&self, cache_key: &str) -> AppResult<()>;
}

#[async_trait]
pub trait SandboxRunRepository: Send + Sync {
    async fn create(&self, run: &est_domain::SandboxRun) -> AppResult<()>;
    async fn update(&self, run: &est_domain::SandboxRun) -> AppResult<()>;
    async fn get(&self, id: &str) -> AppResult<Option<est_domain::SandboxRun>>;
    /// Newest first.
    async fn for_session(&self, session: &SessionId) -> AppResult<Vec<est_domain::SandboxRun>>;
    /// Marks runs left `running` by a previous daemon as failed; returns how many.
    async fn fail_interrupted(&self) -> AppResult<u64>;
}
