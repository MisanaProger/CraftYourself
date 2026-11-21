//! Guided analysis of online-judge problems (LeetCode, Codeforces, …).
//!
//! The learner pastes a problem; it becomes an ordinary Issue in its own
//! practice project: a solution template in the chosen language, the
//! samples, and a test command that checks the solution in the sandbox.
//! Everything else is the usual learning loop — the Issue starts at L0, the
//! mentor follows the engagement levels (the system prompt adds the stages
//! of a problem analysis), escalation needs consent, ADI is computed on
//! completion. Nothing is fetched from the judge.

use std::path::PathBuf;

use est_core::{DomainEvent, TaskId};
use est_domain::{DocReference, MentorSettings, NewTask, TaskSource, TaskStatus, TaskType};
use est_problems::{Lang, Platform, ProblemError, ProblemInput};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::learning::normalize_lang;
use crate::service::{StartedSession, blocking};
use crate::{AddProject, App, AppError, AppResult};

pub mod events {
    pub const PROBLEM_IMPORTED: &str = "problem.imported";
}

impl From<ProblemError> for AppError {
    fn from(e: ProblemError) -> Self {
        match e {
            ProblemError::Invalid(w) => AppError::Invalid(w),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportProblem {
    #[serde(flatten)]
    pub problem: ProblemInput,
    /// `python`, `cpp` or `rust`.
    pub language: String,
    /// Language of the criteria and goals: `ru`, `en` or `uz`.
    #[serde(default)]
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProblemSummary {
    pub task_id: TaskId,
    pub title: String,
    pub platform: String,
    pub problem_id: String,
    pub url: Option<String>,
    pub difficulty: est_domain::Difficulty,
    pub status: TaskStatus,
    pub session_id: Option<String>,
    pub conversation_id: Option<String>,
    pub created_at: est_core::Timestamp,
}

impl App {
    /// Creates (once) the problem's practice project and Issue and starts it.
    /// Importing the same problem in the same language again resumes it.
    pub async fn import_problem(&self, req: ImportProblem) -> AppResult<StartedSession> {
        let problem = est_problems::normalize(&req.problem)?;
        let lang = Lang::parse(&req.language)?;
        let ui = normalize_lang(req.lang.as_deref());
        let dir: PathBuf = self
            .ports
            .workspaces
            .practice_dir(&problem.workspace_name(lang));
        let files = est_problems::workspace_files(&problem, lang, ui);
        let git = self.ports.git.clone();
        let revision = {
            let dir = dir.clone();
            blocking(move || {
                if git.is_repository(&dir) {
                    return git.resolve(&dir, "HEAD")?.ok_or_else(|| {
                        AppError::Infrastructure("problem repository has no commits".into())
                    });
                }
                for (rel, text) in &files {
                    let path = dir.join(rel);
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| AppError::Infrastructure(e.to_string()))?;
                    }
                    std::fs::write(&path, text)
                        .map_err(|e| AppError::Infrastructure(e.to_string()))?;
                }
                git.init_with_commit(&dir, "Problem statement and solution template")
            })
            .await?
        };
        let canonical = dir
            .canonicalize()
            .map_err(|e| AppError::Infrastructure(e.to_string()))?;
        let project = match self.ports.projects.find_by_path(&canonical).await? {
            Some(p) => p,
            None => {
                self.add_project(AddProject {
                    source: canonical.to_string_lossy().into_owned(),
                })
                .await?
            }
        };
        let platform = problem.platform.as_str().to_owned();
        let existing = self
            .list_tasks(Some(&project.id))
            .await?
            .into_iter()
            .find(|t| {
                matches!(&t.source, TaskSource::Problem { platform: p, id, .. }
                    if *p == platform && *id == problem.id)
            });
        let task = match existing {
            Some(t) => t,
            None => {
                let task = self
                    .create_task(NewTask {
                        project_id: project.id.clone(),
                        title: problem.task_title(),
                        description: problem.statement.clone(),
                        task_type: TaskType::Algorithm,
                        difficulty: problem.difficulty_level(),
                        source: TaskSource::Problem {
                            platform: platform.clone(),
                            id: problem.id.clone(),
                            url: problem.url.clone(),
                        },
                        learning_goals: est_problems::learning_goals(ui),
                        disciplines: vec!["algorithms".into(), "data-structures".into()],
                        skills: Vec::new(),
                        base_revision: Some(revision),
                        test_command: Some(est_problems::test_command(&problem, lang)),
                        acceptance_criteria: est_problems::acceptance_criteria(&problem, ui),
                        mentor: Some(MentorSettings::default()),
                        docs: problem
                            .url
                            .iter()
                            .map(|u| DocReference {
                                title: problem.task_title(),
                                url: u.clone(),
                                topics: problem.tags.clone(),
                            })
                            .collect(),
                    })
                    .await?;
                self.emit(
                    DomainEvent::new(
                        events::PROBLEM_IMPORTED,
                        json!({
                            "platform": platform, "problem_id": problem.id,
                            "language": lang.as_str(), "samples": problem.samples.len(),
                            "tags": problem.tags, "rating": problem.rating,
                            "difficulty": problem.difficulty,
                        }),
                    )
                    .project(&project.id)
                    .task(&task.id),
                )
                .await?;
                task
            }
        };
        self.start_task(&task.id).await
    }

    /// Imported problems, newest first.
    pub async fn problems(&self) -> AppResult<Vec<ProblemSummary>> {
        let sessions = self.list_sessions().await?;
        Ok(self
            .list_tasks(None)
            .await?
            .into_iter()
            .filter_map(|t| {
                let TaskSource::Problem { platform, id, url } = &t.source else {
                    return None;
                };
                // Sessions are newest first: the latest one of the task.
                let session = sessions.iter().find(|s| s.task_id == t.id);
                Some(ProblemSummary {
                    platform: Platform::parse(platform)
                        .map(|p| p.as_str().to_owned())
                        .unwrap_or_else(|| platform.clone()),
                    problem_id: id.clone(),
                    url: url.clone(),
                    title: t.title.clone(),
                    difficulty: t.difficulty,
                    status: t.status,
                    session_id: session.map(|s| s.id.to_string()),
                    conversation_id: session
                        .and_then(|s| s.conversation_id.as_ref().map(|c| c.to_string())),
                    created_at: t.created_at,
                    task_id: t.id,
                })
            })
            .collect())
    }
}
