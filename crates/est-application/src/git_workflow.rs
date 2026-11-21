//! Git workflow (spec v0.3, stage 9): commit review, pull request
//! preparation and publishing, Issue writing.
//!
//! Reviews are fixed rules (`est-review`) applied to what Git reports; the
//! conventions are learned from the repository's history before the
//! learner's work. The pull request draft is assembled from observed data.
//! Publishing a pull request or a new Issue goes through the same gate as a
//! comment: a draft with a digest, confirmed by the learner, used once. EST
//! never pushes: the pull request names a branch the learner pushed.

use est_ai_policy::gates::{PublishDraft, PublishKind};
use est_core::{ProjectId, SessionId};
use est_domain::{LearningSession, RunKind, RunStatus, TaskSource};
use est_issues::RepoRef;
use est_review::commits::Review;
use est_review::issue_text::{IssueKind, IssueTextReview};
use est_review::pull_request::{PrDraft, PrInput, TestRun};
use serde::{Deserialize, Serialize};

use crate::remote::{PendingAction, PublishPreview};
use crate::service::blocking;
use crate::{App, AppError, AppResult};

/// History the conventions are learned from.
const HISTORY: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct PullRequestView {
    #[serde(flatten)]
    pub draft: PrDraft,
    /// Where it would be opened, when known.
    pub repository: Option<RepoRef>,
    /// The workspace branch the learner pushes.
    pub branch: String,
    /// The project's default branch, as checked out in the project.
    pub base: Option<String>,
    /// A token for that forge is configured.
    pub can_publish: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DraftPullRequest {
    pub title: String,
    pub body: String,
    /// `branch` or `user:branch` — pushed by the learner.
    pub head: String,
    pub base: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IssueText {
    pub kind: IssueKind,
    pub title: String,
    pub body: String,
}

/// A branch name as forges accept it (`user:` prefix allowed in `head`).
fn valid_branch(name: &str, allow_owner: bool) -> bool {
    let (owner, branch) = match name.split_once(':') {
        Some((o, b)) if allow_owner => (Some(o), b),
        Some(_) => return false,
        None => (None, name),
    };
    let ok = |s: &str| {
        !s.is_empty()
            && s.len() <= 200
            && !s.starts_with(['-', '/', '.'])
            && !s.ends_with(['/', '.'])
            && !s.contains("..")
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
    };
    ok(branch) && owner.is_none_or(|o| ok(o) && !o.contains('/'))
}

impl App {
    /// The learner's commits (oldest first) and the project's history.
    async fn commit_review(&self, s: &LearningSession) -> AppResult<Review> {
        let (git, ws, base) = (
            self.ports.git.clone(),
            s.workspace_path.clone(),
            s.base_revision.clone(),
        );
        let (mut mine, history) = blocking(move || {
            Ok((
                git.commits(&ws, Some(&base), None, 500)?,
                git.commits(&ws, None, Some(&base), HISTORY)?,
            ))
        })
        .await?;
        mine.reverse();
        let conventions = est_review::commits::learn(&history);
        Ok(est_review::commits::review_all(&mine, &conventions))
    }

    pub async fn review_commits(&self, session: &SessionId) -> AppResult<Review> {
        let s = self.get_session(session).await?;
        self.commit_review(&s).await
    }

    /// Commit quality of a finished session, for its summary; `None` when
    /// there are no commits or the workspace is gone.
    pub(crate) async fn commit_quality(&self, s: &LearningSession) -> Option<u8> {
        self.commit_review(s).await.ok().and_then(|r| r.score)
    }

    async fn pr_repository(
        &self,
        s: &LearningSession,
    ) -> AppResult<(Option<RepoRef>, Option<u64>, Option<String>)> {
        let task = self.get_task(&s.task_id).await?;
        let project = self.get_project(&s.project_id).await?;
        let project_repo = project
            .repository_url
            .as_deref()
            .and_then(|u| est_issues::parse_repo_url(u, self.gitlab_hosts()));
        Ok(match &task.source {
            TaskSource::Issue { url } => {
                let issue = est_issues::parse_issue_url(url, self.gitlab_hosts()).ok();
                let repo = issue.as_ref().map(|i| i.repo()).or(project_repo);
                let closes = issue
                    .as_ref()
                    .filter(|i| repo.as_ref() == Some(&i.repo()))
                    .map(|i| i.number);
                (repo, closes, Some(url.clone()))
            }
            _ => (project_repo, None, None),
        })
    }

    /// A pull request draft from the session's observed data.
    pub async fn pull_request(&self, session: &SessionId) -> AppResult<PullRequestView> {
        let s = self.get_session(session).await?;
        let task = self.get_task(&s.task_id).await?;
        let project = self.get_project(&s.project_id).await?;
        let review = self.commit_review(&s).await?;
        let (repository, closes, issue_url) = self.pr_repository(&s).await?;
        let (git, ws, root) = (
            self.ports.git.clone(),
            s.workspace_path.clone(),
            project.local_path.clone(),
        );
        let (uncommitted, base) =
            blocking(move || Ok((git.status(&ws)?.len(), git.current_branch(&root)?))).await?;
        let last_test = self
            .ports
            .sandbox_runs
            .for_session(&s.id)
            .await?
            .into_iter()
            .find(|r| {
                r.kind == RunKind::Test && matches!(r.status, RunStatus::Passed | RunStatus::Failed)
            })
            .map(|r| TestRun {
                passed: r.status == RunStatus::Passed,
                summary: r
                    .summary
                    .map(|t| format!("{} passed, {} failed", t.passed, t.failed)),
                command: r.command,
            });
        let level = self
            .highest_level(&s.task_id)
            .await?
            .map(|l| l.as_u8())
            .unwrap_or(0);
        let draft = est_review::pull_request::draft(&PrInput {
            task_title: task.title.clone(),
            issue_url,
            closes,
            commits: review.commits.iter().map(|c| c.subject.clone()).collect(),
            criteria: task
                .acceptance_criteria
                .iter()
                .map(|c| (c.text.clone(), c.done))
                .collect(),
            last_test,
            uncommitted_files: uncommitted as u32,
            commit_score: review.score,
            has_fixups: review
                .commits
                .iter()
                .any(|c| c.findings.iter().any(|f| f.code == "fixup")),
            highest_level: level,
        });
        Ok(PullRequestView {
            can_publish: repository
                .as_ref()
                .is_some_and(|r| self.ports.remote.can_publish(r)),
            repository,
            branch: s.branch.clone(),
            base,
            draft,
        })
    }

    /// Prepares opening a pull request from a branch the learner pushed.
    /// Nothing is sent until the draft is confirmed.
    pub async fn draft_pull_request(
        &self,
        session: &SessionId,
        req: DraftPullRequest,
    ) -> AppResult<PublishPreview> {
        let s = self.get_session(session).await?;
        let (repo, _, _) = self.pr_repository(&s).await?;
        let repo = repo.ok_or_else(|| {
            AppError::Invalid(
                "the project has no GitHub or GitLab repository to open a pull request in".into(),
            )
        })?;
        let (head, base) = (req.head.trim(), req.base.trim());
        if !valid_branch(head, true) || !valid_branch(base, false) {
            return Err(AppError::Invalid(
                "head is `branch` or `user:branch`, base is a branch name".into(),
            ));
        }
        let title = req.title.trim().to_owned();
        if title.is_empty() || title.chars().count() > 200 {
            return Err(AppError::Invalid(
                "the title must be 1–200 characters".into(),
            ));
        }
        let body = req.body.trim().to_owned();
        let draft = PublishDraft {
            kind: PublishKind::PullRequest,
            host: repo.host.clone(),
            target: format!("{}/compare/{base}...{head}", repo.url()),
            body: format!("{title}\n\n{body}"),
        };
        self.prepare_publish(
            Some(s.task_id.clone()),
            Some(s.project_id.clone()),
            repo.clone(),
            PendingAction::PullRequest {
                repo,
                title,
                body,
                head: head.to_owned(),
                base: base.to_owned(),
            },
            draft,
        )
        .await
    }

    /// The checklist of a bug report or feature request.
    pub fn review_issue_text(&self, req: &IssueText) -> IssueTextReview {
        est_review::issue_text::review(req.kind, &req.title, &req.body)
    }

    /// Prepares opening a new Issue in the project's repository. A text
    /// without a title or a description is refused.
    pub async fn draft_new_issue(
        &self,
        project: &ProjectId,
        req: IssueText,
    ) -> AppResult<PublishPreview> {
        let project = self.get_project(project).await?;
        let review = self.review_issue_text(&req);
        if !review.blockers.is_empty() {
            let codes: Vec<_> = review.blockers.iter().map(|b| b.code.as_str()).collect();
            return Err(AppError::Invalid(format!(
                "the Issue is not ready: {}",
                codes.join(", ")
            )));
        }
        let repo = project
            .repository_url
            .as_deref()
            .and_then(|u| est_issues::parse_repo_url(u, self.gitlab_hosts()))
            .ok_or_else(|| {
                AppError::Invalid("the project has no GitHub or GitLab repository".into())
            })?;
        let title = req.title.trim().chars().take(200).collect::<String>();
        let body = req.body.trim().to_owned();
        let draft = PublishDraft {
            kind: PublishKind::Issue,
            host: repo.host.clone(),
            target: format!("{}/issues/new", repo.url()),
            body: format!("{title}\n\n{body}"),
        };
        self.prepare_publish(
            None,
            Some(project.id.clone()),
            repo.clone(),
            PendingAction::Issue { repo, title, body },
            draft,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::valid_branch;

    #[test]
    fn branch_names() {
        assert!(valid_branch("est/fix-range", false));
        assert!(valid_branch("me:est/fix-range", true));
        assert!(!valid_branch("me:est/fix", false));
        for bad in ["", "-x", "a..b", "a b", "a/", "me/x:y", "x;rm"] {
            assert!(!valid_branch(bad, true), "{bad}");
        }
    }
}
