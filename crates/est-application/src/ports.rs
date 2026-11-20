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
    /// Initialises a repository with all files of `path` in one commit;
    /// returns the commit hash.
    fn init_with_commit(&self, path: &Path, message: &str) -> AppResult<String>;
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
    /// Directory for a practice project created by a learning program.
    fn practice_dir(&self, name: &str) -> PathBuf;
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

/// Receives the answer's text as it arrives.
pub type DeltaSink<'a> = &'a (dyn Fn(&str) + Send + Sync);

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn info(&self) -> ProviderInfo;
    async fn generate(&self, request: LlmRequest) -> AppResult<LlmResponse>;
    /// Like [`LlmProvider::generate`], passing text to `on_delta` as it
    /// arrives; the returned response is the whole answer. Providers that
    /// cannot stream deliver it in one piece.
    async fn generate_stream(
        &self,
        request: LlmRequest,
        on_delta: DeltaSink<'_>,
    ) -> AppResult<LlmResponse> {
        let response = self.generate(request).await?;
        on_delta(&response.content);
        Ok(response)
    }
}

/// The currently active AI Gateway provider. The learner can switch
/// providers at runtime (settings page), so the application reads the slot
/// on every request instead of holding a provider for its whole lifetime.
#[derive(Default)]
pub struct LlmSlot(std::sync::RwLock<Option<std::sync::Arc<dyn LlmProvider>>>);

impl LlmSlot {
    pub fn new(provider: Option<std::sync::Arc<dyn LlmProvider>>) -> Self {
        Self(std::sync::RwLock::new(provider))
    }

    /// The active provider, `None` when no model is connected.
    pub fn get(&self) -> Option<std::sync::Arc<dyn LlmProvider>> {
        self.0.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set(&self, provider: Option<std::sync::Arc<dyn LlmProvider>>) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = provider;
    }
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

// ---- learning --------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LessonStatus {
    NotStarted,
    Started,
    Completed,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct LessonProgress {
    pub program_id: String,
    pub lesson_id: String,
    pub status: LessonStatus,
    /// Best score, 0–100 (quizzes and artifacts).
    pub score: Option<u8>,
    pub attempts: u32,
    pub detail: serde_json::Value,
    pub updated_at: est_core::Timestamp,
}

#[async_trait]
pub trait LearningProgressRepository: Send + Sync {
    async fn progress(
        &self,
        user: &UserId,
        program: Option<&str>,
    ) -> AppResult<Vec<LessonProgress>>;
    async fn save_progress(&self, user: &UserId, progress: &LessonProgress) -> AppResult<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationStatus {
    Generating,
    Ready,
    Failed,
}

/// A learning program generated by a model, with its generation state.
#[derive(Debug, Clone)]
pub struct GeneratedProgramRecord {
    pub id: String,
    pub topic: String,
    pub lang: String,
    pub level: est_learning::generated::Difficulty,
    pub status: GenerationStatus,
    pub error: Option<String>,
    pub lessons_total: u32,
    pub lessons_done: u32,
    /// `provider/model` that wrote it.
    pub model: Option<String>,
    /// Present once the outline is accepted.
    pub content: Option<est_learning::generated::GeneratedProgram>,
    pub created_at: est_core::Timestamp,
    pub updated_at: est_core::Timestamp,
}

#[async_trait]
pub trait GeneratedProgramRepository: Send + Sync {
    /// Inserts or replaces the record.
    async fn save_generated(&self, user: &UserId, record: &GeneratedProgramRecord)
    -> AppResult<()>;
    async fn generated(&self, id: &str) -> AppResult<Option<GeneratedProgramRecord>>;
    /// Newest first.
    async fn list_generated(&self, user: &UserId) -> AppResult<Vec<GeneratedProgramRecord>>;
    /// Deletes the program and the learner's progress in it.
    async fn delete_generated(&self, user: &UserId, id: &str) -> AppResult<()>;
    /// Marks generations left running by a previous daemon as failed.
    async fn fail_interrupted_generations(&self) -> AppResult<u64>;
}

/// Learning recommendations produced by a model from the learner's activity.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct RecommendationSet {
    pub id: String,
    pub created_at: est_core::Timestamp,
    /// `provider/model` that wrote it.
    pub model: String,
    pub lang: String,
    pub summary: String,
    pub items: Vec<crate::recommendations::Recommendation>,
    /// Exactly what the model was given.
    pub activity: serde_json::Value,
}

#[async_trait]
pub trait RecommendationRepository: Send + Sync {
    async fn save_recommendations(&self, user: &UserId, set: &RecommendationSet) -> AppResult<()>;
    /// Newest first.
    async fn recommendations(
        &self,
        user: &UserId,
        limit: usize,
    ) -> AppResult<Vec<RecommendationSet>>;
}

/// Session Summaries, written in code when a session ends.
#[async_trait]
pub trait SessionSummaryRepository: Send + Sync {
    /// Inserts or replaces the summary of its session.
    async fn save_summary(
        &self,
        user: &UserId,
        summary: &est_domain::SessionSummary,
    ) -> AppResult<()>;
    async fn summary(&self, session: &SessionId) -> AppResult<Option<est_domain::SessionSummary>>;
    /// Newest first.
    async fn summaries(&self, user: &UserId) -> AppResult<Vec<est_domain::SessionSummary>>;
}

/// One observed result of a quiz attempt or an artifact, kept as Skill
/// Matrix evidence (lesson progress keeps only the best score).
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
pub struct LessonResult {
    pub program_id: String,
    pub lesson_id: String,
    pub source: est_discipline::EvidenceSource,
    pub score: u8,
    pub observed_at: est_core::Timestamp,
}

#[async_trait]
pub trait LessonResultRepository: Send + Sync {
    async fn record_result(&self, user: &UserId, result: &LessonResult) -> AppResult<()>;
    /// Oldest first.
    async fn lesson_results(&self, user: &UserId) -> AppResult<Vec<LessonResult>>;
}
