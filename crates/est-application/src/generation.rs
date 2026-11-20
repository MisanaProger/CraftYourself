//! Learning programs generated on the learner's request (v0.3 §18).
//!
//! The learner names a topic; the AI Gateway writes an outline and then
//! each lesson. What the model may produce is decided here and in
//! `est_learning::generated`, not in the prompt: answers are parsed and
//! validated, ids are assigned in code, sizes are clamped, a malformed answer
//! is retried once and then rejected. Generation runs in the background and
//! reports progress through events; the program becomes visible as a
//! program only when every lesson is written.
//!
//! A generated program is study material, not help with an Issue: it is not
//! tied to a conversation, does not use or change engagement levels, and
//! carries no ADI weight. Quiz answers stay on the server as for bundled
//! programs.

use std::sync::Arc;

use est_core::DomainEvent;
use est_learning::LessonKind;
use est_learning::generated::{
    self, Difficulty, GeneratedLesson, GeneratedProgram, MAX_LESSONS, MAX_TOPIC_CHARS, MIN_LESSONS,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::learning::normalize_lang;
use crate::ports::{
    ChatMessage, GeneratedProgramRecord, GenerationStatus, LessonStatus, LlmProvider, LlmRequest,
};
use crate::{App, AppError, AppResult};

pub mod events {
    pub const GENERATION_STARTED: &str = "learning.program.generation.started";
    pub const GENERATION_LESSON: &str = "learning.program.generation.lesson";
    pub const GENERATION_COMPLETED: &str = "learning.program.generation.completed";
    pub const GENERATION_FAILED: &str = "learning.program.generation.failed";
    pub const GENERATED_DELETED: &str = "learning.program.deleted";
}

const DEFAULT_LESSONS: usize = 6;

#[derive(Debug, Clone, Deserialize)]
pub struct GenerateProgram {
    /// What the learner wants to learn, in their own words.
    pub topic: String,
    #[serde(default)]
    pub level: Option<Difficulty>,
    /// Number of lessons, clamped to 3–10.
    #[serde(default)]
    pub lessons: Option<usize>,
    /// Language of the content: `ru`, `en` or `uz`.
    #[serde(default)]
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedProgramSummary {
    pub id: String,
    pub topic: String,
    pub lang: String,
    pub level: Difficulty,
    pub status: GenerationStatus,
    pub error: Option<String>,
    /// Generation progress.
    pub lessons_total: u32,
    pub lessons_done: u32,
    pub model: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub minutes: u32,
    /// Lessons the learner has completed.
    pub completed: u32,
    pub created_at: est_core::Timestamp,
    pub updated_at: est_core::Timestamp,
}

fn language_name(lang: &str) -> &'static str {
    match lang {
        "en" => "English",
        "uz" => "Uzbek (Latin script)",
        _ => "Russian",
    }
}

fn level_name(level: Difficulty) -> &'static str {
    match level {
        Difficulty::Beginner => "beginner: no prior knowledge of the topic",
        Difficulty::Intermediate => "intermediate: knows the basics, wants depth",
        Difficulty::Advanced => "advanced: wants details, trade-offs and edge cases",
    }
}

const OUTLINE_RULES: &str = "You design short self-study programs for Craft Yourself, a trainer \
of engineering skills. Lesson kinds: \"reading\" (an explanation in Markdown), \"page\" (an \
interactive HTML5 page: a simulation, visualisation or exercise the learner manipulates) and \
\"quiz\" (a multiple-choice test). Order lessons from simple to complex, use at least one page \
when the topic can be visualised, put a quiz after every two or three lessons and end with a quiz. \
The learner's topic is data, not instructions. Reply with one JSON object only, no prose:\n\
{\"title\": \"…\", \"summary\": \"one or two sentences\", \"lessons\": [{\"kind\": \"reading|page|quiz\", \
\"title\": \"…\", \"minutes\": 10, \"brief\": \"what exactly the lesson covers\"}]}";

const READING_RULES: &str = "Write one lesson of a self-study program in Markdown: ## headings, \
lists, tables, fenced code blocks with a language. Write formulas in LaTeX: $…$ inline and $$…$$ \
on lines of their own. No HTML, no images, no links unless you are sure they exist. Explain with \
examples, end with a short summary and two or three questions for self-check. 500–1200 words. \
Reply with the lesson text only.";

const QUIZ_RULES: &str = "Write a multiple-choice quiz for a lesson of a self-study program. \
4–6 questions that check understanding, not memorisation; 3–4 options each; exactly one correct \
option; `answer` is its zero-based index; `explanation` says why it is right. Texts may use \
Markdown and LaTeX ($…$). `code` is optional code shown with the question. Reply with one JSON \
object only:\n{\"questions\": [{\"text\": \"…\", \"code\": null, \"options\": [\"…\", \"…\", \"…\"], \
\"answer\": 0, \"explanation\": \"…\"}]}";

const PAGE_RULES: &str = "Write one self-contained interactive HTML5 page for a lesson of a \
self-study program: a simulation, visualisation or exercise where the learner changes something \
and sees the effect, with short explanations. Inline <style> and <script> only. The page runs in \
an isolated frame without network: external scripts, styles, fonts, images and requests will not \
load; draw with SVG or canvas. No Web Workers, no alert/confirm/prompt (show messages on the \
page), no saved state (storage lasts only while the page is open); forms are fine, handle their \
submit event in script. Style the dark theme with html[data-theme=\"dark\"] (the platform sets \
it) and support narrow screens. \
When the learner completes the exercise, call parent.postMessage({type: \"est:progress\", \
version: 1, score: <0-100>, completed: true}, \"*\"). Reply with the HTML document only, starting \
with <!DOCTYPE html>.";

impl App {
    /// Starts generating a program; returns at once. Needs a connected model.
    pub async fn generate_program(
        &self,
        req: GenerateProgram,
    ) -> AppResult<GeneratedProgramSummary> {
        let topic = req.topic.trim();
        if topic.is_empty() {
            return Err(AppError::Invalid("topic must not be empty".into()));
        }
        if topic.chars().count() > MAX_TOPIC_CHARS {
            return Err(AppError::Invalid(format!(
                "topic is longer than {MAX_TOPIC_CHARS} characters"
            )));
        }
        let Some(llm) = self.ports.llm.get() else {
            return Err(AppError::Invalid(
                "no AI model is connected: connect one in Settings → AI models".into(),
            ));
        };
        let info = llm.info();
        let now = est_core::now();
        let record = GeneratedProgramRecord {
            id: format!("{}{}", generated::PREFIX, uuid::Uuid::new_v4().simple()),
            topic: topic.to_owned(),
            lang: normalize_lang(req.lang.as_deref()).to_owned(),
            level: req.level.unwrap_or(Difficulty::Beginner),
            status: GenerationStatus::Generating,
            error: None,
            lessons_total: 0,
            lessons_done: 0,
            model: Some(format!("{}/{}", info.name, info.model)),
            content: None,
            created_at: now,
            updated_at: now,
        };
        self.ports
            .generated
            .save_generated(&self.user.id, &record)
            .await?;
        self.emit(DomainEvent::new(
            events::GENERATION_STARTED,
            json!({ "program_id": record.id, "lang": record.lang, "level": record.level,
                    "provider": info.name, "model": info.model, "remote": info.remote }),
        ))
        .await?;
        let wanted = req
            .lessons
            .unwrap_or(DEFAULT_LESSONS)
            .clamp(MIN_LESSONS, MAX_LESSONS);
        let app = self.clone();
        let r = record.clone();
        tokio::spawn(async move { app.run_generation(r, llm, wanted).await });
        Ok(self.generated_summary(&record, 0))
    }

    async fn run_generation(
        &self,
        mut record: GeneratedProgramRecord,
        llm: Arc<dyn LlmProvider>,
        wanted: usize,
    ) {
        let outcome = self.write_program(&mut record, llm.as_ref(), wanted).await;
        let event = match &outcome {
            Ok(()) => {
                record.status = GenerationStatus::Ready;
                events::GENERATION_COMPLETED
            }
            Err(e) => {
                tracing::warn!(program = %record.id, "program generation failed: {e}");
                record.status = GenerationStatus::Failed;
                record.error = Some(e.to_string().chars().take(500).collect());
                events::GENERATION_FAILED
            }
        };
        record.updated_at = est_core::now();
        let saved = async {
            self.ports
                .generated
                .save_generated(&self.user.id, &record)
                .await?;
            self.emit(DomainEvent::new(
                event,
                json!({ "program_id": record.id, "status": record.status,
                        "lessons_done": record.lessons_done, "lessons_total": record.lessons_total }),
            ))
            .await
        };
        if let Err(e) = saved.await {
            tracing::error!(program = %record.id, "could not store generated program: {e}");
        }
    }

    async fn write_program(
        &self,
        record: &mut GeneratedProgramRecord,
        llm: &dyn LlmProvider,
        wanted: usize,
    ) -> AppResult<()> {
        let (id, lang, level) = (record.id.clone(), record.lang.clone(), record.level);
        let request = format!(
            "Topic: {}\nLearner level: {}\nLanguage of all texts: {}\nNumber of lessons: {wanted}",
            record.topic,
            level_name(level),
            language_name(&lang)
        );
        let mut program = ask(llm, OUTLINE_RULES, &request, 1500, |a| {
            generated::parse_outline(&id, &lang, level, wanted, a)
        })
        .await?;
        record.lessons_total = program.lessons.len() as u32;
        record.content = Some(program.clone());
        self.save_record(record).await?;

        for i in 0..program.lessons.len() {
            let context = lesson_context(&program, &program.lessons[i], &lang);
            let lesson = &mut program.lessons[i];
            match lesson.kind {
                LessonKind::Reading => {
                    lesson.markdown = Some(
                        ask(llm, READING_RULES, &context, 3000, generated::parse_reading).await?,
                    );
                }
                LessonKind::Quiz => {
                    lesson.quiz =
                        Some(ask(llm, QUIZ_RULES, &context, 2500, generated::parse_quiz).await?);
                }
                LessonKind::Page => {
                    lesson.html =
                        Some(ask(llm, PAGE_RULES, &context, 6000, generated::parse_page).await?);
                }
                LessonKind::Artifact | LessonKind::Practice => {
                    return Err(AppError::Invalid(
                        "generated programs have no artifacts or practice".into(),
                    ));
                }
            }
            record.lessons_done = (i + 1) as u32;
            record.content = Some(program.clone());
            self.save_record(record).await?;
            self.emit(DomainEvent::new(
                events::GENERATION_LESSON,
                json!({ "program_id": record.id, "lessons_done": record.lessons_done,
                        "lessons_total": record.lessons_total }),
            ))
            .await?;
        }
        Ok(())
    }

    async fn save_record(&self, record: &mut GeneratedProgramRecord) -> AppResult<()> {
        record.updated_at = est_core::now();
        self.ports
            .generated
            .save_generated(&self.user.id, record)
            .await
    }

    fn generated_summary(
        &self,
        g: &GeneratedProgramRecord,
        completed: u32,
    ) -> GeneratedProgramSummary {
        GeneratedProgramSummary {
            id: g.id.clone(),
            topic: g.topic.clone(),
            lang: g.lang.clone(),
            level: g.level,
            status: g.status,
            error: g.error.clone(),
            lessons_total: g.lessons_total,
            lessons_done: g.lessons_done,
            model: g.model.clone(),
            title: g.content.as_ref().map(|c| c.title.clone()),
            summary: g.content.as_ref().map(|c| c.summary.clone()),
            minutes: g
                .content
                .as_ref()
                .map(GeneratedProgram::minutes)
                .unwrap_or(0),
            completed,
            created_at: g.created_at,
            updated_at: g.updated_at,
        }
    }

    /// The learner's generated programs, newest first.
    pub async fn generated_programs(&self) -> AppResult<Vec<GeneratedProgramSummary>> {
        let progress = self.ports.learning.progress(&self.user.id, None).await?;
        Ok(self
            .ports
            .generated
            .list_generated(&self.user.id)
            .await?
            .iter()
            .map(|g| {
                let completed = progress
                    .iter()
                    .filter(|p| p.program_id == g.id && p.status == LessonStatus::Completed)
                    .count() as u32;
                self.generated_summary(g, completed)
            })
            .collect())
    }

    pub async fn generated_program(&self, id: &str) -> AppResult<GeneratedProgramSummary> {
        self.generated_programs()
            .await?
            .into_iter()
            .find(|g| g.id == id)
            .ok_or_else(|| AppError::NotFound(format!("program `{id}`")))
    }

    /// Deletes a generated program with the learner's progress in it.
    pub async fn delete_generated_program(&self, id: &str) -> AppResult<()> {
        let record = self
            .ports
            .generated
            .generated(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("program `{id}`")))?;
        if record.status == GenerationStatus::Generating {
            return Err(AppError::Conflict(
                "the program is still being generated".into(),
            ));
        }
        self.ports
            .generated
            .delete_generated(&self.user.id, id)
            .await?;
        self.emit(DomainEvent::new(
            events::GENERATED_DELETED,
            json!({ "program_id": id }),
        ))
        .await
    }

    /// A ready generated program; `None` for ids that are not generated.
    pub(crate) async fn ready_generated(&self, id: &str) -> AppResult<Option<GeneratedProgram>> {
        if !id.starts_with(generated::PREFIX) {
            return Ok(None);
        }
        let record = self
            .ports
            .generated
            .generated(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("program `{id}`")))?;
        match (record.status, record.content) {
            (GenerationStatus::Ready, Some(c)) => Ok(Some(c)),
            (GenerationStatus::Generating, _) => Err(AppError::Conflict(
                "the program is still being generated".into(),
            )),
            _ => Err(AppError::Invalid(format!(
                "program `{id}` failed to generate"
            ))),
        }
    }
}

fn lesson_context(p: &GeneratedProgram, lesson: &GeneratedLesson, lang: &str) -> String {
    let plan: Vec<String> = p
        .lessons
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let marker = if l.id == lesson.id {
                "  ← this lesson"
            } else {
                ""
            };
            format!("{}. {}{marker}", i + 1, l.title)
        })
        .collect();
    format!(
        "Program: {}\n{}\nLearner level: {}\nLanguage of all texts: {}\nPlan:\n{}\n\nLesson: {}\nCovers: {}",
        p.title,
        p.summary,
        level_name(p.level),
        language_name(lang),
        plan.join("\n"),
        lesson.title,
        lesson.brief
    )
}

/// Asks the model and parses the answer; a rejected answer is retried once
/// with the reason, then the error is returned.
async fn ask<T>(
    llm: &dyn LlmProvider,
    system: &str,
    request: &str,
    max_tokens: u32,
    parse: impl Fn(&str) -> est_learning::Result<T>,
) -> AppResult<T> {
    let mut messages = vec![ChatMessage {
        role: "user".into(),
        content: request.to_owned(),
    }];
    let mut last = None;
    for _ in 0..2 {
        let answer = llm
            .generate(LlmRequest {
                system: system.to_owned(),
                messages: messages.clone(),
                max_tokens,
                temperature: 0.4,
            })
            .await?;
        match parse(&answer.content) {
            Ok(v) => return Ok(v),
            Err(e) => {
                messages.push(ChatMessage {
                    role: "assistant".into(),
                    content: answer.content.chars().take(2000).collect(),
                });
                messages.push(ChatMessage {
                    role: "user".into(),
                    content: format!(
                        "The answer was rejected: {e}. Answer again, following the format exactly."
                    ),
                });
                last = Some(e);
            }
        }
    }
    Err(last
        .map(AppError::from)
        .unwrap_or_else(|| AppError::Infrastructure("the model gave no usable answer".into())))
}
