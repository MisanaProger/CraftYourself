//! Interactive Learning Engine use cases (architecture v0.3, §18).
//!
//! Content comes from `est-learning`; progress is stored per learner.
//! Finishing a material by itself does not raise any Skill Matrix score:
//! quiz results, artifact results and practice outcomes are recorded so they
//! can become assessment evidence later.

use std::collections::BTreeMap;
use std::path::PathBuf;

use est_core::{DomainEvent, TaskId};
use est_domain::TaskStatus;
use est_learning::generated::GeneratedProgram;
use est_learning::{
    LearningError, LessonDef, LessonKind, Program, ProgramStatus, QuestionView, Quiz, QuizResult,
    pick,
};
use serde::Serialize;
use serde_json::json;

use crate::ports::{LessonProgress, LessonStatus};
use crate::service::{StartedSession, blocking};
use crate::{AddProject, App, AppError, AppResult};

impl From<LearningError> for AppError {
    fn from(e: LearningError) -> Self {
        match e {
            LearningError::NotFound(w) => AppError::NotFound(w),
            LearningError::Invalid(w) => AppError::Invalid(w),
        }
    }
}

pub fn normalize_lang(lang: Option<&str>) -> &'static str {
    match lang.map(|l| l.get(..2).unwrap_or(l).to_ascii_lowercase()) {
        Some(l) if l == "en" => "en",
        Some(l) if l == "uz" => "uz",
        _ => "ru",
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgramTile {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub status: ProgramStatus,
    pub icon: String,
    pub level: String,
    pub minutes: u32,
    pub disciplines: Vec<String>,
    pub lessons: u32,
    pub completed: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct LessonSummary {
    pub id: String,
    pub kind: LessonKind,
    pub title: String,
    pub minutes: u32,
    pub status: LessonStatus,
    pub score: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgramView {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub disciplines: Vec<String>,
    pub lessons: Vec<LessonSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LessonBody {
    Reading {
        markdown: String,
    },
    Quiz {
        questions: Vec<QuestionView>,
        pass_percent: u32,
    },
    Artifact {
        artifact_id: String,
        entrypoint: String,
        objectives: Vec<String>,
    },
    Practice {
        conversation_id: Option<String>,
        session_id: Option<String>,
        task_status: Option<TaskStatus>,
    },
    /// An HTML5 page of a generated program, served as an isolated document.
    Page {},
}

/// A program with its content: bundled, or generated and stored.
pub(crate) struct Loaded {
    pub program: Program,
    generated: Option<GeneratedProgram>,
}

impl Loaded {
    fn reading(&self, lesson: &LessonDef, lang: &str) -> AppResult<String> {
        Ok(match &self.generated {
            Some(g) => g.markdown(&lesson.id)?,
            None => est_learning::reading(&self.program, lesson, lang)?,
        })
    }

    fn quiz(&self, lesson: &LessonDef) -> AppResult<Quiz> {
        Ok(match &self.generated {
            Some(g) => g.quiz(&lesson.id)?,
            None => est_learning::quiz(&self.program, lesson)?,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LessonView {
    pub program_id: String,
    #[serde(flatten)]
    pub summary: LessonSummary,
    pub intro: String,
    pub body: LessonBody,
    pub previous: Option<String>,
    pub next: Option<String>,
}

fn not_started(program: &str, lesson: &str) -> LessonProgress {
    LessonProgress {
        program_id: program.into(),
        lesson_id: lesson.into(),
        status: LessonStatus::NotStarted,
        score: None,
        attempts: 0,
        detail: json!({}),
        updated_at: est_core::now(),
    }
}

impl App {
    pub(crate) async fn load_program(&self, id: &str) -> AppResult<Loaded> {
        Ok(match self.ready_generated(id).await? {
            Some(g) => Loaded {
                program: g.to_program(),
                generated: Some(g),
            },
            None => Loaded {
                program: est_learning::program(id)?,
                generated: None,
            },
        })
    }

    async fn progress_map(
        &self,
        program: Option<&str>,
    ) -> AppResult<BTreeMap<(String, String), LessonProgress>> {
        Ok(self
            .ports
            .learning
            .progress(&self.user.id, program)
            .await?
            .into_iter()
            .map(|p| ((p.program_id.clone(), p.lesson_id.clone()), p))
            .collect())
    }

    /// Tiles for the home page.
    pub async fn learning_programs(&self, lang: &str) -> AppResult<Vec<ProgramTile>> {
        let progress = self.progress_map(None).await?;
        let mut out = Vec::new();
        for e in est_learning::catalog()? {
            let (lessons, completed) = if e.status == ProgramStatus::Available {
                let p = est_learning::program(&e.id)?;
                let done = p
                    .lessons
                    .iter()
                    .filter(|l| {
                        progress
                            .get(&(e.id.clone(), l.id.clone()))
                            .is_some_and(|x| x.status == LessonStatus::Completed)
                    })
                    .count();
                (p.lessons.len() as u32, done as u32)
            } else {
                (0, 0)
            };
            out.push(ProgramTile {
                title: pick(&e.title, lang),
                summary: pick(&e.summary, lang),
                id: e.id,
                status: e.status,
                icon: e.icon,
                level: e.level,
                minutes: e.minutes,
                disciplines: e.disciplines,
                lessons,
                completed,
            });
        }
        Ok(out)
    }

    fn summary(
        &self,
        program: &Program,
        l: &LessonDef,
        lang: &str,
        p: Option<&LessonProgress>,
    ) -> LessonSummary {
        let _ = program;
        LessonSummary {
            id: l.id.clone(),
            kind: l.kind,
            title: pick(&l.title, lang),
            minutes: l.minutes,
            status: p.map(|x| x.status).unwrap_or(LessonStatus::NotStarted),
            score: p.and_then(|x| x.score),
        }
    }

    pub async fn learning_program(&self, id: &str, lang: &str) -> AppResult<ProgramView> {
        let program = self.load_program(id).await?.program;
        self.sync_practice_progress(&program).await?;
        let progress = self.progress_map(Some(id)).await?;
        Ok(ProgramView {
            id: program.id.clone(),
            title: pick(&program.title, lang),
            summary: pick(&program.summary, lang),
            disciplines: program.disciplines.clone(),
            lessons: program
                .lessons
                .iter()
                .map(|l| {
                    self.summary(
                        &program,
                        l,
                        lang,
                        progress.get(&(program.id.clone(), l.id.clone())),
                    )
                })
                .collect(),
        })
    }

    pub async fn learning_lesson(
        &self,
        program_id: &str,
        lesson_id: &str,
        lang: &str,
    ) -> AppResult<LessonView> {
        let loaded = self.load_program(program_id).await?;
        let program = &loaded.program;
        self.sync_practice_progress(program).await?;
        let lesson = est_learning::lesson(program, lesson_id)?;
        let progress = self.progress_map(Some(program_id)).await?;
        let p = progress.get(&(program.id.clone(), lesson.id.clone()));
        let body = match lesson.kind {
            LessonKind::Reading => LessonBody::Reading {
                markdown: loaded.reading(lesson, lang)?,
            },
            LessonKind::Page => LessonBody::Page {},
            LessonKind::Quiz => {
                let q = loaded.quiz(lesson)?;
                LessonBody::Quiz {
                    questions: est_learning::quiz_view(&q, lang),
                    pass_percent: q.pass_percent,
                }
            }
            LessonKind::Artifact => {
                let m = est_learning::artifact_manifest(
                    lesson.artifact.as_deref().unwrap_or_default(),
                )?;
                LessonBody::Artifact {
                    artifact_id: m.id.clone(),
                    entrypoint: m.entrypoint.clone(),
                    objectives: m
                        .learning_objectives
                        .iter()
                        .map(|o| pick(o, lang))
                        .collect(),
                }
            }
            LessonKind::Practice => {
                let task = self.practice_task(p).await?;
                let (conversation_id, session_id, task_status) = match &task {
                    Some(t) => {
                        let session = self
                            .list_sessions()
                            .await?
                            .into_iter()
                            .find(|s| s.task_id == t.id);
                        (
                            session
                                .as_ref()
                                .and_then(|s| s.conversation_id.as_ref().map(|c| c.to_string())),
                            session.map(|s| s.id.to_string()),
                            Some(t.status),
                        )
                    }
                    None => (None, None, None),
                };
                LessonBody::Practice {
                    conversation_id,
                    session_id,
                    task_status,
                }
            }
        };
        let idx = program
            .lessons
            .iter()
            .position(|l| l.id == lesson.id)
            .unwrap_or(0);
        Ok(LessonView {
            program_id: program.id.clone(),
            summary: self.summary(program, lesson, lang, p),
            intro: pick(&lesson.intro, lang),
            body,
            previous: idx
                .checked_sub(1)
                .and_then(|i| program.lessons.get(i))
                .map(|l| l.id.clone()),
            next: program.lessons.get(idx + 1).map(|l| l.id.clone()),
        })
    }

    async fn save(&self, p: &LessonProgress, event: &str) -> AppResult<()> {
        self.ports.learning.save_progress(&self.user.id, p).await?;
        self.emit(DomainEvent::new(
            event,
            json!({ "program_id": p.program_id, "lesson_id": p.lesson_id, "status": p.status, "score": p.score }),
        ))
        .await
    }

    async fn current(&self, program: &str, lesson: &str) -> AppResult<LessonProgress> {
        Ok(self
            .progress_map(Some(program))
            .await?
            .remove(&(program.to_owned(), lesson.to_owned()))
            .unwrap_or_else(|| not_started(program, lesson)))
    }

    async fn lesson_of(
        &self,
        program_id: &str,
        lesson_id: &str,
        kinds: &[LessonKind],
    ) -> AppResult<(Loaded, LessonDef)> {
        let loaded = self.load_program(program_id).await?;
        let lesson = est_learning::lesson(&loaded.program, lesson_id)?.clone();
        if !kinds.contains(&lesson.kind) {
            return Err(AppError::Invalid(
                format!("lesson `{lesson_id}` is not a {:?} lesson", kinds[0]).to_lowercase(),
            ));
        }
        Ok((loaded, lesson))
    }

    /// HTML of a page lesson, to be served as an isolated document.
    pub async fn lesson_page(&self, program_id: &str, lesson_id: &str) -> AppResult<String> {
        let (loaded, lesson) = self
            .lesson_of(program_id, lesson_id, &[LessonKind::Page])
            .await?;
        match &loaded.generated {
            Some(g) => Ok(g.html(&lesson.id)?),
            None => Err(AppError::NotFound(format!("page `{lesson_id}`"))),
        }
    }

    pub async fn complete_reading(
        &self,
        program_id: &str,
        lesson_id: &str,
    ) -> AppResult<LessonProgress> {
        // A page counts as read when the learner says so, even if it never
        // reports progress itself.
        self.lesson_of(
            program_id,
            lesson_id,
            &[LessonKind::Reading, LessonKind::Page],
        )
        .await?;
        let mut p = self.current(program_id, lesson_id).await?;
        p.status = LessonStatus::Completed;
        p.attempts += 1;
        p.updated_at = est_core::now();
        self.save(&p, "learning.lesson.completed").await?;
        Ok(p)
    }

    /// Grades a quiz on the server and keeps the best score.
    pub async fn submit_quiz(
        &self,
        program_id: &str,
        lesson_id: &str,
        answers: BTreeMap<String, usize>,
        lang: &str,
    ) -> AppResult<QuizResult> {
        let (loaded, lesson) = self
            .lesson_of(program_id, lesson_id, &[LessonKind::Quiz])
            .await?;
        let quiz = loaded.quiz(&lesson)?;
        let result = est_learning::grade(&quiz, &answers, lang);
        let mut p = self.current(program_id, lesson_id).await?;
        p.attempts += 1;
        p.score = Some(p.score.unwrap_or(0).max(result.score as u8));
        if result.passed {
            p.status = LessonStatus::Completed;
        } else if p.status == LessonStatus::NotStarted {
            p.status = LessonStatus::Started;
        }
        p.detail = json!({ "last_answers": answers, "last_score": result.score });
        p.updated_at = est_core::now();
        self.save(&p, "learning.quiz.submitted").await?;
        Ok(result)
    }

    /// Progress reported by a sandboxed artifact via the platform.
    pub async fn artifact_progress(
        &self,
        program_id: &str,
        lesson_id: &str,
        score: u32,
        completed: bool,
    ) -> AppResult<LessonProgress> {
        self.lesson_of(
            program_id,
            lesson_id,
            &[LessonKind::Artifact, LessonKind::Page],
        )
        .await?;
        let mut p = self.current(program_id, lesson_id).await?;
        p.attempts += 1;
        p.score = Some(p.score.unwrap_or(0).max(score.min(100) as u8));
        if completed {
            p.status = LessonStatus::Completed;
        } else if p.status == LessonStatus::NotStarted {
            p.status = LessonStatus::Started;
        }
        p.updated_at = est_core::now();
        self.save(&p, "learning.artifact.progress").await?;
        Ok(p)
    }

    async fn practice_task(
        &self,
        p: Option<&LessonProgress>,
    ) -> AppResult<Option<est_domain::Task>> {
        match p
            .and_then(|p| p.detail.get("task_id"))
            .and_then(|t| t.as_str())
        {
            Some(id) => self.ports.tasks.get(&TaskId::from(id)).await,
            None => Ok(None),
        }
    }

    /// A practice lesson is completed when its Issue is completed.
    async fn sync_practice_progress(&self, program: &Program) -> AppResult<()> {
        for lesson in program
            .lessons
            .iter()
            .filter(|l| l.kind == LessonKind::Practice)
        {
            let mut p = self.current(&program.id, &lesson.id).await?;
            if p.status == LessonStatus::Completed {
                continue;
            }
            if let Some(task) = self.practice_task(Some(&p)).await?
                && task.status == TaskStatus::Done
            {
                p.status = LessonStatus::Completed;
                p.updated_at = est_core::now();
                self.save(&p, "learning.lesson.completed").await?;
            }
        }
        Ok(())
    }

    /// Creates the practice project from its template (once), imports its
    /// exercise as an Issue and starts it in its own workspace.
    pub async fn start_practice(
        &self,
        program_id: &str,
        lesson_id: &str,
    ) -> AppResult<StartedSession> {
        let (loaded, lesson) = self
            .lesson_of(program_id, lesson_id, &[LessonKind::Practice])
            .await?;
        let template = est_learning::practice_template(&loaded.program, &lesson)?;
        let dir: PathBuf = self
            .ports
            .workspaces
            .practice_dir(&format!("{program_id}-{lesson_id}"));
        let git = self.ports.git.clone();
        let revision = {
            let dir = dir.clone();
            let files = template.files.clone();
            blocking(move || {
                if git.is_repository(&dir) {
                    return git.resolve(&dir, "HEAD")?.ok_or_else(|| {
                        AppError::Infrastructure("practice repository has no commits".into())
                    });
                }
                for (rel, data) in &files {
                    let path = dir.join(rel);
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| AppError::Infrastructure(e.to_string()))?;
                    }
                    std::fs::write(&path, data)
                        .map_err(|e| AppError::Infrastructure(e.to_string()))?;
                }
                git.init_with_commit(&dir, "Initial practice project")
            })
            .await?
        };
        let canonical = dir
            .canonicalize()
            .map_err(|e| AppError::Infrastructure(e.to_string()))?;
        if self
            .ports
            .projects
            .find_by_path(&canonical)
            .await?
            .is_none()
        {
            self.add_project(AddProject {
                source: canonical.to_string_lossy().into_owned(),
            })
            .await?;
        }
        let manifest = template
            .manifest
            .replace("{{repository}}", &format!("file://{}", canonical.display()))
            .replace("{{revision}}", &revision);
        let task = self.import_exercise(&manifest).await?;
        let started = self.start_task(&task.id).await?;

        let mut p = self.current(program_id, lesson_id).await?;
        if p.status == LessonStatus::NotStarted {
            p.status = LessonStatus::Started;
        }
        p.detail = json!({ "task_id": task.id });
        p.updated_at = est_core::now();
        self.save(&p, "learning.practice.started").await?;
        Ok(started)
    }
}
