//! Forges and judges: importing open-source Issues, loading problem
//! statements, publishing on the learner's behalf (stages 3 and 5).
//!
//! Every action goes through `est_ai_policy::gates` before the
//! [`RemoteSources`](crate::ports::RemoteSources) port is called:
//!
//! * reading needs the learner's explicit network permission for this
//!   request (`allow_network`) and a known host; without it the request
//!   fails with [`AppError::PermissionRequired`] and nothing is sent;
//! * publishing is two-step: a draft shows exactly what goes where and gets
//!   a digest; only `confirm` with that digest, within the time limit,
//!   publishes it, once. Drafts live in memory: a restart discards them.
//!
//! These are learner actions; the mentor cannot start any of them, and none
//! changes the engagement level or ADI.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use est_ai_policy::gates::{
    self, Confirmation, GateDecision, Initiator, PublishContext, PublishDraft, PublishKind, Warning,
};
use est_core::{DomainEvent, ProjectId, TaskId, Timestamp};
use est_domain::{
    Difficulty, DocReference, EngagementLevel, EscalationStatus, MentorSettings, NewTask, Task,
    TaskSource, TaskType,
};
use est_issues::{IssueRef, RemoteIssue, Suitability, TaskDraft};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ports::FetchedProblem;
use crate::sandbox::events::NETWORK_GRANTED;
use crate::{AddProject, App, AppError, AppResult};

pub mod events {
    pub const ISSUE_IMPORTED: &str = "issue.imported";
    pub const PUBLISH_DRAFTED: &str = "publish.drafted";
    pub const PUBLISH_COMPLETED: &str = "publish.completed";
    pub const PUBLISH_DISCARDED: &str = "publish.discarded";
}

/// Drafts waiting for the learner's confirmation, by id.
#[derive(Default, Clone)]
pub(crate) struct PendingDrafts(Arc<Mutex<HashMap<String, Pending>>>);

#[derive(Clone)]
struct Pending {
    task_id: Option<TaskId>,
    project_id: Option<ProjectId>,
    action: PendingAction,
    draft: PublishDraft,
    created_at: Timestamp,
}

/// What a confirmed draft does.
#[derive(Clone)]
pub(crate) enum PendingAction {
    Comment(IssueRef),
    PullRequest {
        repo: est_issues::RepoRef,
        title: String,
        body: String,
        head: String,
        base: String,
    },
    Issue {
        repo: est_issues::RepoRef,
        title: String,
        body: String,
    },
}

impl PendingDrafts {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Pending>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct IssueRequest {
    pub url: String,
    /// The learner allows EST to read from the forge for this request.
    #[serde(default)]
    pub allow_network: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct IssuePreview {
    pub issue: IssueRef,
    pub title: String,
    pub labels: Vec<String>,
    pub open: bool,
    pub comments: u32,
    pub repository: est_issues::RemoteRepository,
    pub suitability: Suitability,
    /// The task the Issue would become.
    pub task: TaskDraft,
    /// A connected project for this repository, if any.
    pub project_id: Option<ProjectId>,
    /// The Issue is already imported as this task.
    pub task_id: Option<TaskId>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportIssue {
    #[serde(flatten)]
    pub request: IssueRequest,
    /// Work in this connected project (e.g. the learner's fork) instead of
    /// cloning the Issue's repository.
    #[serde(default)]
    pub project_id: Option<ProjectId>,
    #[serde(default)]
    pub task_type: Option<TaskType>,
    #[serde(default)]
    pub difficulty: Option<Difficulty>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FetchProblem {
    pub url: String,
    #[serde(default)]
    pub allow_network: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DraftComment {
    pub body: String,
}

/// What would be published; nothing is sent until it is confirmed.
#[derive(Debug, Clone, Serialize)]
pub struct PublishPreview {
    pub id: String,
    pub kind: PublishKind,
    pub host: String,
    pub target: String,
    pub body: String,
    pub digest: String,
    pub warnings: Vec<Warning>,
    pub expires_at: Timestamp,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfirmPublish {
    pub digest: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Published {
    pub url: String,
}

pub(crate) fn gate(decision: GateDecision) -> AppResult<()> {
    match decision {
        GateDecision::Allow => Ok(()),
        GateDecision::NeedsPermission { reason } => Err(AppError::PermissionRequired(reason)),
        GateDecision::Deny { reason } => Err(AppError::Invalid(reason)),
        GateDecision::NeedsConfirmation { .. } => Err(AppError::Invalid(
            "publishing needs a confirmed draft".into(),
        )),
    }
}

impl App {
    pub(crate) fn gitlab_hosts(&self) -> &[String] {
        &self.settings.gitlab_hosts
    }

    async fn fetch_issue(&self, req: &IssueRequest) -> AppResult<(IssueRef, RemoteIssue)> {
        let issue = est_issues::parse_issue_url(&req.url, self.gitlab_hosts())
            .map_err(|e| AppError::Invalid(e.0))?;
        gate(gates::check_fetch(
            Initiator::Learner,
            &issue.host,
            req.allow_network,
            self.gitlab_hosts(),
        ))?;
        self.emit(DomainEvent::new(
            NETWORK_GRANTED,
            json!({ "host": issue.host, "purpose": "issue", "url": issue.url }),
        ))
        .await?;
        let remote = self.ports.remote.issue(&issue).await?;
        Ok((issue, remote))
    }

    async fn issue_task(&self, url: &str) -> AppResult<Option<Task>> {
        Ok(self
            .list_tasks(None)
            .await?
            .into_iter()
            .find(|t| matches!(&t.source, TaskSource::Issue { url: u } if u == url)))
    }

    async fn project_for(&self, clone_url: &str, issue: &IssueRef) -> AppResult<Option<ProjectId>> {
        for url in [clone_url.to_owned(), issue.clone_url()] {
            if let Some(p) = self.ports.projects.find_by_url(&url).await? {
                return Ok(Some(p.id));
            }
        }
        Ok(None)
    }

    /// Reads an Issue and checks whether it suits a learner. Nothing is
    /// stored.
    pub async fn preview_issue(&self, req: IssueRequest) -> AppResult<IssuePreview> {
        let (issue, remote) = self.fetch_issue(&req).await?;
        let suitability = est_issues::assess(&remote, est_core::now());
        let task = est_issues::draft_task(&issue, &remote);
        Ok(IssuePreview {
            project_id: self
                .project_for(&remote.repository.clone_url, &issue)
                .await?,
            task_id: self.issue_task(&issue.url).await?.map(|t| t.id),
            title: remote.title,
            labels: remote.labels,
            open: remote.open,
            comments: remote.comments,
            repository: remote.repository,
            suitability,
            task,
            issue,
        })
    }

    /// Imports an Issue as a task (once): reads it again, refuses an
    /// unsuitable one, connects its repository unless a project is given.
    /// The task starts like any other, at L0.
    pub async fn import_issue(&self, req: ImportIssue) -> AppResult<Task> {
        let (issue, remote) = self.fetch_issue(&req.request).await?;
        if let Some(task) = self.issue_task(&issue.url).await? {
            return Ok(task);
        }
        let suitability = est_issues::assess(&remote, est_core::now());
        if !suitability.importable() {
            let blockers: Vec<_> = suitability
                .findings
                .iter()
                .filter(|f| f.severity == est_issues::Severity::Blocker)
                .map(|f| f.code.as_str())
                .collect();
            return Err(AppError::Invalid(format!(
                "this Issue does not suit a learning task: {}",
                blockers.join(", ")
            )));
        }
        let project_id = match req.project_id {
            Some(id) => self.get_project(&id).await?.id,
            None => match self
                .project_for(&remote.repository.clone_url, &issue)
                .await?
            {
                Some(id) => id,
                None => {
                    self.add_project(AddProject {
                        source: remote.repository.clone_url.clone(),
                    })
                    .await?
                    .id
                }
            },
        };
        let draft = est_issues::draft_task(&issue, &remote);
        let task = self
            .create_task(NewTask {
                project_id: project_id.clone(),
                title: draft.title,
                description: draft.description,
                task_type: req.task_type.unwrap_or(draft.task_type),
                difficulty: req.difficulty.unwrap_or(draft.difficulty),
                source: TaskSource::Issue {
                    url: issue.url.clone(),
                },
                learning_goals: Vec::new(),
                disciplines: draft.disciplines,
                skills: Vec::new(),
                base_revision: None,
                test_command: None,
                acceptance_criteria: draft.acceptance_criteria,
                mentor: Some(MentorSettings::default()),
                docs: vec![DocReference {
                    title: format!("{}#{}", issue.project, issue.number),
                    url: issue.url.clone(),
                    topics: remote.labels.clone(),
                }],
            })
            .await?;
        self.emit(
            DomainEvent::new(
                events::ISSUE_IMPORTED,
                json!({
                    "forge": issue.forge.as_str(), "host": issue.host,
                    "repository": issue.project, "number": issue.number,
                    "verdict": suitability.verdict, "labels": remote.labels,
                }),
            )
            .project(&project_id)
            .task(&task.id),
        )
        .await?;
        Ok(task)
    }

    /// Reads a problem's statement and samples from the judge for the
    /// learner to review; importing is the usual `import_problem`.
    pub async fn fetch_problem(&self, req: FetchProblem) -> AppResult<FetchedProblem> {
        let problem = est_problems::parse_url(&req.url)?;
        let host = url::Url::parse(&problem.url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .unwrap_or_default();
        gate(gates::check_fetch(
            Initiator::Learner,
            &host,
            req.allow_network,
            &[],
        ))?;
        self.emit(DomainEvent::new(
            NETWORK_GRANTED,
            json!({ "host": host, "purpose": "problem", "url": problem.url }),
        ))
        .await?;
        self.ports.remote.problem(&problem).await
    }

    /// Highest level the mentor worked at in the task's conversation.
    pub(crate) async fn highest_level(&self, task: &TaskId) -> AppResult<Option<EngagementLevel>> {
        let Some(conv) = self.ports.conversations.for_task(task).await? else {
            return Ok(None);
        };
        let state = self.ports.engagement.state(&conv.id).await?;
        let accepted = self
            .ports
            .engagement
            .escalations(&conv.id)
            .await?
            .into_iter()
            .filter(|e| e.status == EscalationStatus::Accepted)
            .map(|e| e.to);
        Ok(state.map(|s| s.level).into_iter().chain(accepted).max())
    }

    /// Prepares a comment on the task's Issue. Nothing is sent.
    pub async fn draft_issue_comment(
        &self,
        task_id: &TaskId,
        req: DraftComment,
    ) -> AppResult<PublishPreview> {
        let task = self.get_task(task_id).await?;
        let TaskSource::Issue { url } = &task.source else {
            return Err(AppError::Invalid(
                "only tasks imported from an Issue can be commented on".into(),
            ));
        };
        let issue = est_issues::parse_issue_url(url, self.gitlab_hosts())
            .map_err(|e| AppError::Invalid(e.0))?;
        let draft = PublishDraft {
            kind: PublishKind::IssueComment,
            host: issue.host.clone(),
            target: issue.url.clone(),
            body: req.body.trim().to_owned(),
        };
        let repo = issue.repo();
        self.prepare_publish(
            Some(task.id.clone()),
            None,
            repo,
            PendingAction::Comment(issue),
            draft,
        )
        .await
    }

    /// Checks a draft with the gate, keeps it for confirmation and shows
    /// it. Nothing is sent.
    pub(crate) async fn prepare_publish(
        &self,
        task_id: Option<TaskId>,
        project_id: Option<ProjectId>,
        repo: est_issues::RepoRef,
        action: PendingAction,
        draft: PublishDraft,
    ) -> AppResult<PublishPreview> {
        let context = PublishContext {
            highest_level: match &task_id {
                Some(t) => self.highest_level(t).await?,
                None => None,
            },
        };
        let (digest, warnings) = match gates::check_publish(
            Initiator::Learner,
            &draft,
            &context,
            None,
            self.gitlab_hosts(),
        ) {
            GateDecision::NeedsConfirmation { digest, warnings } => (digest, warnings),
            other => {
                return Err(gate(other).err().unwrap_or_else(|| {
                    AppError::Invalid("publishing needs a confirmed draft".into())
                }));
            }
        };
        if !self.ports.remote.can_publish(&repo) {
            return Err(AppError::Invalid(format!(
                "no token for publishing to {} is configured (`[remote]` in config.toml)",
                repo.host
            )));
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        let created_at = est_core::now();
        self.drafts.lock().insert(
            id.clone(),
            Pending {
                task_id: task_id.clone(),
                project_id: project_id.clone(),
                action,
                draft: draft.clone(),
                created_at,
            },
        );
        let mut event = DomainEvent::new(
            events::PUBLISH_DRAFTED,
            json!({
                "draft_id": id, "kind": draft.kind, "host": draft.host,
                "target": draft.target, "digest": digest,
                "chars": draft.body.chars().count(), "warnings": warnings,
            }),
        );
        if let Some(t) = &task_id {
            event = event.task(t);
        }
        if let Some(p) = &project_id {
            event = event.project(p);
        }
        self.emit(event).await?;
        Ok(PublishPreview {
            id,
            kind: draft.kind,
            host: draft.host,
            target: draft.target,
            body: draft.body,
            digest,
            warnings,
            expires_at: created_at + chrono_secs(gates::CONFIRMATION_TTL_SECS),
        })
    }

    /// Publishes a draft the learner confirmed. A draft is used once.
    pub async fn confirm_publish(&self, id: &str, req: ConfirmPublish) -> AppResult<Published> {
        let pending = self
            .drafts
            .lock()
            .remove(id)
            .ok_or_else(|| AppError::NotFound("draft".into()))?;
        let context = PublishContext {
            highest_level: match &pending.task_id {
                Some(t) => self.highest_level(t).await?,
                None => None,
            },
        };
        gate(gates::check_publish(
            Initiator::Learner,
            &pending.draft,
            &context,
            Some(&Confirmation {
                digest: req.digest,
                drafted_at: pending.created_at,
                now: est_core::now(),
            }),
            self.gitlab_hosts(),
        ))?;
        let remote = &self.ports.remote;
        let url = match &pending.action {
            PendingAction::Comment(issue) => remote.comment(issue, &pending.draft.body).await?,
            PendingAction::PullRequest {
                repo,
                title,
                body,
                head,
                base,
            } => {
                remote
                    .create_pull_request(repo, title, body, head, base)
                    .await?
            }
            PendingAction::Issue { repo, title, body } => {
                remote.create_issue(repo, title, body).await?
            }
        };
        let mut event = DomainEvent::new(
            events::PUBLISH_COMPLETED,
            json!({
                "draft_id": id, "kind": pending.draft.kind, "host": pending.draft.host,
                "target": pending.draft.target, "url": url,
            }),
        );
        if let Some(t) = &pending.task_id {
            event = event.task(t);
        }
        if let Some(p) = &pending.project_id {
            event = event.project(p);
        }
        self.emit(event).await?;
        Ok(Published { url })
    }

    /// Discards a draft without publishing it.
    pub async fn discard_draft(&self, id: &str) -> AppResult<()> {
        let pending = self
            .drafts
            .lock()
            .remove(id)
            .ok_or_else(|| AppError::NotFound("draft".into()))?;
        let mut event = DomainEvent::new(events::PUBLISH_DISCARDED, json!({ "draft_id": id }));
        if let Some(t) = &pending.task_id {
            event = event.task(t);
        }
        self.emit(event).await
    }
}

fn chrono_secs(secs: i64) -> chrono::Duration {
    chrono::Duration::seconds(secs)
}
