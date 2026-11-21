//! Use cases. Each public method on [`App`] is one scenario of the learning
//! loop: project → task → workspace → exploration → mentor → solution → Git.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use est_core::{DomainEvent, ProjectId, SessionId, TaskId, events, slugify};
use est_dependency::DecayParams;
use est_domain::{
    CommandSpec, LearningSession, NewProject, NewTask, Project, SessionOutcome, SessionStatus,
    Task, TaskStatus,
};
use est_exercises::ExerciseManifest;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ports::*;
use crate::{AppError, AppResult};

/// All infrastructure the application needs, injected by the composition root.
#[derive(Clone)]
pub struct Ports {
    pub users: Arc<dyn UserRepository>,
    pub projects: Arc<dyn ProjectRepository>,
    pub tasks: Arc<dyn TaskRepository>,
    pub sessions: Arc<dyn SessionRepository>,
    pub events: Arc<dyn EventLog>,
    pub publisher: Arc<dyn EventPublisher>,
    pub git: Arc<dyn GitPort>,
    pub inspector: Arc<dyn ProjectInspector>,
    pub workspaces: Arc<dyn WorkspaceLayout>,
    pub ide: Arc<dyn IdeLauncher>,
    pub conversations: Arc<dyn ConversationRepository>,
    pub engagement: Arc<dyn EngagementRepository>,
    pub adi: Arc<dyn AdiRepository>,
    pub export: Arc<dyn DataExporter>,
    /// AI Gateway provider; empty when no model is configured.
    pub llm: Arc<LlmSlot>,
    pub sandbox: Arc<dyn Sandbox>,
    pub sandbox_runs: Arc<dyn SandboxRunRepository>,
    pub learning: Arc<dyn LearningProgressRepository>,
    pub generated: Arc<dyn GeneratedProgramRepository>,
    pub recommendations: Arc<dyn RecommendationRepository>,
    pub summaries: Arc<dyn SessionSummaryRepository>,
    pub lesson_results: Arc<dyn LessonResultRepository>,
    /// Code forges and online judges.
    pub remote: Arc<dyn RemoteSources>,
    pub discoveries: Arc<dyn DiscoveryRepository>,
}

#[derive(Debug, Clone, Default)]
pub struct AppSettings {
    /// ADI double-decay parameters (preliminary MVP values by default).
    pub adi_decay: DecayParams,
    /// Self-hosted GitLab instances Issues may come from.
    pub gitlab_hosts: Vec<String>,
}

#[derive(Clone)]
pub struct App {
    pub(crate) ports: Ports,
    pub(crate) settings: AppSettings,
    pub(crate) user: User,
    /// Publications waiting for the learner's confirmation.
    pub(crate) drafts: crate::remote::PendingDrafts,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddProject {
    /// Path to a local Git repository, or a remote clone URL.
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StartedSession {
    #[serde(flatten)]
    pub session: LearningSession,
    /// True when an already active session was returned.
    pub resumed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionOverview {
    pub session: LearningSession,
    pub task: Task,
    pub project: Project,
    pub current_branch: Option<String>,
    pub changes: Vec<FileStatus>,
    /// Commits made since the base revision: the learner's own work.
    pub commits: Vec<CommitSummary>,
}

pub(crate) async fn blocking<T, F>(f: F) -> AppResult<T>
where
    F: FnOnce() -> AppResult<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Infrastructure(format!("worker failed: {e}")))?
}

/// `https://host/owner/repo.git` → `repo`; `/home/me/src/app` → `app`.
pub fn name_from_source(source: &str) -> String {
    let name = source
        .trim_end_matches('/')
        .rsplit(['/', ':', '\\'])
        .next()
        .unwrap_or(source)
        .trim_end_matches(".git");
    if name.is_empty() {
        "project".into()
    } else {
        name.into()
    }
}

impl App {
    pub async fn new(ports: Ports, settings: AppSettings) -> AppResult<Self> {
        let user = ports.users.local_user().await?;
        // Runs cannot survive a daemon restart; record them as failed.
        ports.sandbox_runs.fail_interrupted().await?;
        // So are program generations.
        ports.generated.fail_interrupted_generations().await?;
        Ok(Self {
            ports,
            settings,
            user,
            drafts: Default::default(),
        })
    }

    pub fn user(&self) -> &User {
        &self.user
    }

    /// Info about the active AI provider, if any.
    pub fn llm_info(&self) -> Option<ProviderInfo> {
        self.ports.llm.get().map(|l| l.info())
    }

    /// Switches the AI provider the mentor uses. Policy is unaffected: the
    /// level, limits and documentation boundary are decided before any
    /// provider is called. The event records where context now goes (never
    /// a key).
    pub async fn set_llm(&self, provider: Option<Arc<dyn LlmProvider>>) -> AppResult<()> {
        let info = provider.as_ref().map(|p| p.info());
        self.ports.llm.set(provider);
        self.emit(DomainEvent::new(
            events::AI_PROVIDER_CHANGED,
            json!({ "provider": info }),
        ))
        .await
    }

    pub(crate) async fn emit(&self, event: DomainEvent) -> AppResult<()> {
        self.ports.events.append(&event).await?;
        self.ports.publisher.publish(&event);
        Ok(())
    }

    // ---- projects --------------------------------------------------------

    pub async fn add_project(&self, req: AddProject) -> AppResult<Project> {
        let source = req.source.trim().to_owned();
        if source.is_empty() {
            return Err(AppError::Invalid("project source must not be empty".into()));
        }
        let git = self.ports.git.clone();
        let local = source
            .strip_prefix("file://")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&source));

        let (local_path, repository_url) = if local.is_dir() {
            // A repository on the learner's machine: EST reads it in place.
            let path = local
                .canonicalize()
                .map_err(|e| AppError::Invalid(format!("cannot open {source}: {e}")))?;
            if !git.is_repository(&path) {
                return Err(AppError::Invalid(format!(
                    "{} is not a Git repository",
                    path.display()
                )));
            }
            if let Some(p) = self.ports.projects.find_by_path(&path).await? {
                return Err(AppError::Conflict(format!(
                    "{} is already connected as project {}",
                    path.display(),
                    p.id
                )));
            }
            let url = {
                let (git, path) = (git.clone(), path.clone());
                blocking(move || git.remote_url(&path)).await?
            };
            (path, url)
        } else {
            if let Some(p) = self.ports.projects.find_by_url(&source).await? {
                return Err(AppError::Conflict(format!(
                    "{source} is already connected as project {}",
                    p.id
                )));
            }
            let cache_name = format!(
                "{}-{}",
                name_from_source(&source),
                ProjectId::generate().short()
            );
            let dest = self.ports.workspaces.project_cache(&cache_name);
            {
                let (git, url, dest) = (git.clone(), source.clone(), dest.clone());
                blocking(move || {
                    let r = git.clone_remote(&url, &dest);
                    if r.is_err() {
                        let _ = std::fs::remove_dir_all(&dest);
                    }
                    r
                })
                .await?;
            }
            (dest, Some(source.clone()))
        };

        let inspection = {
            let (git, inspector, path) = (
                git.clone(),
                self.ports.inspector.clone(),
                local_path.clone(),
            );
            blocking(move || {
                let files = git.tracked_files(&path)?;
                Ok(inspector.inspect(&path, &files))
            })
            .await?
        };
        let project = self
            .ports
            .projects
            .create(NewProject {
                name: name_from_source(&source),
                repository_url,
                local_path,
                languages: inspection.languages,
                license: inspection.license,
            })
            .await?;
        self.emit(
            DomainEvent::new(events::PROJECT_ADDED, json!({ "name": project.name }))
                .project(&project.id),
        )
        .await?;
        Ok(project)
    }

    pub async fn list_projects(&self) -> AppResult<Vec<Project>> {
        self.ports.projects.list().await
    }

    pub async fn get_project(&self, id: &ProjectId) -> AppResult<Project> {
        self.ports
            .projects
            .get(id)
            .await?
            .ok_or_else(|| AppError::NotFound("project".into()))
    }

    pub async fn project_files(&self, id: &ProjectId) -> AppResult<Vec<String>> {
        let p = self.get_project(id).await?;
        let git = self.ports.git.clone();
        blocking(move || git.tracked_files(&p.local_path)).await
    }

    /// Build/test/check commands suggested by the language adapters.
    pub async fn project_commands(&self, id: &ProjectId) -> AppResult<Vec<CommandSpec>> {
        let p = self.get_project(id).await?;
        let (git, inspector) = (self.ports.git.clone(), self.ports.inspector.clone());
        blocking(move || {
            let files = git.tracked_files(&p.local_path)?;
            Ok(inspector.commands(&p.local_path, &files))
        })
        .await
    }

    // ---- tasks -----------------------------------------------------------

    pub async fn create_task(&self, task: NewTask) -> AppResult<Task> {
        task.validate()?;
        if let Some(d) = task
            .disciplines
            .iter()
            .find(|d| !est_discipline::is_known_discipline(d))
        {
            return Err(AppError::Invalid(format!("unknown discipline `{d}`")));
        }
        let project = self.get_project(&task.project_id).await?;
        if let Some(rev) = task.base_revision.clone() {
            self.resolve_in_project(&project, &rev).await?;
        }
        let task = self.ports.tasks.create(task).await?;
        self.emit(
            DomainEvent::new(events::TASK_CREATED, json!({ "title": task.title }))
                .project(&project.id)
                .task(&task.id),
        )
        .await?;
        Ok(task)
    }

    pub async fn list_tasks(&self, project: Option<&ProjectId>) -> AppResult<Vec<Task>> {
        self.ports.tasks.list(project).await
    }

    pub async fn get_task(&self, id: &TaskId) -> AppResult<Task> {
        self.ports
            .tasks
            .get(id)
            .await?
            .ok_or_else(|| AppError::NotFound("task".into()))
    }

    /// Replaces the task's acceptance criteria (written by the learner, or
    /// saved from a mentor suggestion they accepted).
    pub async fn set_acceptance_criteria(
        &self,
        id: &TaskId,
        criteria: Vec<est_domain::AcceptanceCriterion>,
    ) -> AppResult<Task> {
        let task = self.get_task(id).await?;
        let criteria: Vec<_> = criteria
            .into_iter()
            .map(|c| est_domain::AcceptanceCriterion {
                text: c.text.trim().to_owned(),
                done: c.done,
            })
            .filter(|c| !c.text.is_empty())
            .collect();
        if criteria.len() > 20 || criteria.iter().any(|c| c.text.chars().count() > 500) {
            return Err(AppError::Invalid(
                "at most 20 criteria of up to 500 characters".into(),
            ));
        }
        self.ports
            .tasks
            .set_acceptance_criteria(&task.id, &criteria)
            .await?;
        self.emit(
            DomainEvent::new(
                "task.acceptance_criteria.updated",
                json!({ "count": criteria.len(), "done": criteria.iter().filter(|c| c.done).count() }),
            )
            .task(&task.id),
        )
        .await?;
        self.get_task(id).await
    }

    /// Imports an Exercise Manifest as a task, connecting its project if
    /// needed. Importing the same exercise version twice is idempotent.
    pub async fn import_exercise(&self, manifest_toml: &str) -> AppResult<Task> {
        let manifest = ExerciseManifest::from_toml(manifest_toml)?;
        let repo = &manifest.source.repository;
        let local = repo.strip_prefix("file://").map(PathBuf::from);
        let existing = match &local {
            Some(path) => match path.canonicalize() {
                Ok(p) => self.ports.projects.find_by_path(&p).await?,
                Err(_) => None,
            },
            None => self.ports.projects.find_by_url(repo).await?,
        };
        let project = match existing {
            Some(p) => p,
            None => {
                self.add_project(AddProject {
                    source: repo.clone(),
                })
                .await?
            }
        };
        if let Some(task) = self
            .ports
            .tasks
            .find_exercise(
                &project.id,
                &manifest.exercise.id,
                &manifest.exercise.version,
            )
            .await?
        {
            return Ok(task);
        }
        self.create_task(manifest.to_new_task(project.id)).await
    }

    async fn resolve_in_project(&self, project: &Project, rev: &str) -> AppResult<String> {
        let (git, path, rev) = (
            self.ports.git.clone(),
            project.local_path.clone(),
            rev.to_owned(),
        );
        let is_remote_cache = project.repository_url.is_some();
        blocking(move || {
            if let Some(hash) = git.resolve(&path, &rev)? {
                return Ok(hash);
            }
            if is_remote_cache
                && git.fetch(&path).is_ok()
                && let Some(hash) = git.resolve(&path, &rev)?
            {
                return Ok(hash);
            }
            Err(AppError::Invalid(format!(
                "revision {rev} does not exist in the project repository"
            )))
        })
        .await
    }

    // ---- sessions --------------------------------------------------------

    /// Starts (or resumes) work on a task in its own workspace.
    pub async fn start_task(&self, task_id: &TaskId) -> AppResult<StartedSession> {
        if let Some(session) = self.ports.sessions.active_for_task(task_id).await? {
            self.emit(DomainEvent::new(events::SESSION_RESUMED, json!({})).session(&session.id))
                .await?;
            return Ok(StartedSession {
                session,
                resumed: true,
            });
        }
        let mut task = self.get_task(task_id).await?;
        let project = self.get_project(&task.project_id).await?;
        task.start()?;

        let base = self
            .resolve_in_project(&project, task.base_revision.as_deref().unwrap_or("HEAD"))
            .await?;
        let workspace = self.ports.workspaces.workspace_for(&project, &task);
        let title_slug = slugify(&task.title, 40);
        let branch = if title_slug.is_empty() {
            format!("est/task-{}", task.id.short())
        } else {
            format!("est/{title_slug}-{}", task.id.short())
        };

        {
            let git = self.ports.git.clone();
            let (src, ws, branch, base) = (
                project.local_path.clone(),
                workspace.clone(),
                branch.clone(),
                base.clone(),
            );
            blocking(move || {
                if git.is_repository(&ws) {
                    // A previous session left this workspace: continue in it.
                    return Ok(());
                }
                if ws.exists()
                    && ws
                        .read_dir()
                        .map(|mut d| d.next().is_some())
                        .unwrap_or(true)
                {
                    return Err(AppError::Conflict(format!(
                        "{} exists and is not an EST workspace",
                        ws.display()
                    )));
                }
                let result = git
                    .clone_local(&src, &ws)
                    .and_then(|_| git.create_branch(&ws, &branch, &base));
                if result.is_err() {
                    let _ = std::fs::remove_dir_all(&ws);
                }
                result
            })
            .await?;
        }

        let conversation = self.conversation_for_task(&task).await?;
        let session = LearningSession {
            id: SessionId::generate(),
            user_id: self.user.id.clone(),
            project_id: project.id.clone(),
            task_id: task.id.clone(),
            conversation_id: Some(conversation.id),
            workspace_path: workspace,
            branch,
            base_revision: base,
            status: SessionStatus::Active,
            started_at: est_core::now(),
            finished_at: None,
        };
        self.ports.sessions.create(&session).await?;
        self.ports.tasks.set_status(&task.id, task.status).await?;
        self.emit(
            DomainEvent::new(
                events::SESSION_STARTED,
                json!({ "workspace": session.workspace_path, "branch": session.branch }),
            )
            .session(&session.id)
            .project(&project.id)
            .task(&task.id),
        )
        .await?;
        Ok(StartedSession {
            session,
            resumed: false,
        })
    }

    pub async fn get_session(&self, id: &SessionId) -> AppResult<LearningSession> {
        match self.ports.sessions.get(id).await? {
            Some(s) if s.user_id == self.user.id => Ok(s),
            _ => Err(AppError::NotFound("session".into())),
        }
    }

    pub async fn list_sessions(&self) -> AppResult<Vec<LearningSession>> {
        self.ports.sessions.list(&self.user.id).await
    }

    /// The session whose workspace contains `path` (e.g. the CLI's cwd).
    pub async fn session_for_path(&self, path: &Path) -> AppResult<LearningSession> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
        match self.ports.sessions.find_by_workspace(&path).await? {
            Some(s) if s.user_id == self.user.id => Ok(s),
            _ => Err(AppError::NotFound(format!(
                "session for {}",
                path.display()
            ))),
        }
    }

    pub async fn finish_session(
        &self,
        id: &SessionId,
        outcome: SessionOutcome,
    ) -> AppResult<LearningSession> {
        let mut session = self.get_session(id).await?;
        session.finish(outcome, est_core::now())?;
        self.ports.sessions.update(&session).await?;
        let task_status = match outcome {
            SessionOutcome::Completed => TaskStatus::Done,
            SessionOutcome::Abandoned => TaskStatus::Open,
        };
        self.ports
            .tasks
            .set_status(&session.task_id, task_status)
            .await?;
        let commits = self.commits_since_base(&session).await?;
        self.emit(
            DomainEvent::new(
                events::SESSION_FINISHED,
                json!({ "outcome": outcome, "commits": commits }),
            )
            .session(&session.id)
            .task(&session.task_id),
        )
        .await?;
        // Temporary build caches are removed with the session (v0.3 §17).
        self.ports.sandbox.discard(session.id.as_str()).await?;
        // The final ADI is set only for a completed Issue (v0.3 §10.2).
        let adi = match outcome {
            SessionOutcome::Completed => Some(self.assess_adi(&session).await?.score),
            SessionOutcome::Abandoned => None,
        };
        self.summarize_session(&session, adi).await?;
        Ok(session)
    }

    /// Completes the task's active session (v0.3 `POST /tasks/{id}/complete`).
    pub async fn complete_task(&self, task_id: &TaskId) -> AppResult<LearningSession> {
        let session = self
            .ports
            .sessions
            .active_for_task(task_id)
            .await?
            .ok_or_else(|| AppError::Conflict("the task has no active session".into()))?;
        self.finish_session(&session.id, SessionOutcome::Completed)
            .await
    }

    pub(crate) async fn commits_since_base(&self, s: &LearningSession) -> AppResult<u32> {
        let (git, ws, base) = (
            self.ports.git.clone(),
            s.workspace_path.clone(),
            s.base_revision.clone(),
        );
        let n = blocking(move || git.log(&ws, Some(&base), 10_000).map(|l| l.len())).await?;
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    }

    pub async fn session_overview(&self, id: &SessionId) -> AppResult<SessionOverview> {
        let session = self.get_session(id).await?;
        let task = self.get_task(&session.task_id).await?;
        let project = self.get_project(&session.project_id).await?;
        let (git, ws, base) = (
            self.ports.git.clone(),
            session.workspace_path.clone(),
            session.base_revision.clone(),
        );
        let (current_branch, changes, commits) = blocking(move || {
            Ok((
                git.current_branch(&ws)?,
                git.status(&ws)?,
                git.log(&ws, Some(&base), 200)?,
            ))
        })
        .await?;
        Ok(SessionOverview {
            session,
            task,
            project,
            current_branch,
            changes,
            commits,
        })
    }

    /// `target`: `worktree`, `staged` or `base` (all work since the base revision).
    pub async fn session_diff(&self, id: &SessionId, target: &str) -> AppResult<String> {
        let s = self.get_session(id).await?;
        let target = target.to_owned();
        let git = self.ports.git.clone();
        blocking(move || {
            let t = match target.as_str() {
                "worktree" => DiffTarget::Worktree,
                "staged" => DiffTarget::Staged,
                "base" => DiffTarget::Since(&s.base_revision),
                other => return Err(AppError::Invalid(format!("unknown diff target `{other}`"))),
            };
            git.diff(&s.workspace_path, t)
        })
        .await
    }

    pub async fn open_workspace(&self, id: &SessionId) -> AppResult<String> {
        let s = self.get_session(id).await?;
        let ide = self.ports.ide.clone();
        let ws = s.workspace_path.clone();
        let command = blocking(move || ide.open(&ws)).await?;
        self.emit(
            DomainEvent::new(events::WORKSPACE_OPENED, json!({ "command": command }))
                .session(&s.id),
        )
        .await?;
        Ok(command)
    }

    pub async fn session_events(&self, id: &SessionId) -> AppResult<Vec<StoredEvent>> {
        self.get_session(id).await?;
        self.ports.events.for_session(id, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(
            name_from_source("https://github.com/BurntSushi/ripgrep.git"),
            "ripgrep"
        );
        assert_eq!(name_from_source("git@github.com:o/r.git"), "r");
        assert_eq!(name_from_source("/home/me/src/app/"), "app");
    }
}
