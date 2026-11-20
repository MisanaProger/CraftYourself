//! Session Summary and Knowledge Profile (stage 6).
//!
//! A summary is built in code when a session ends — from the same observed
//! events as the ADI, the session's sandbox runs, its commits and the task's
//! acceptance criteria — and stored. Sessions that ended before summaries
//! existed, or whose summary was dropped with a deleted conversation, are
//! summarized on the next request from what is still stored.
//!
//! The profile is computed from the summaries on request
//! ([`est_domain::KnowledgeProfile::build`]). Neither changes ADI or the
//! Skill Matrix.

use est_core::{DomainEvent, SessionId};
use est_domain::{
    HelpStats, KnowledgeProfile, LearningSession, RunKind, RunStats, SessionOutcome, SessionStatus,
    SessionSummary, TaskSource, TopicKind, normalize_topics,
};
use serde::Serialize;
use serde_json::json;

use crate::learning::normalize_lang;
use crate::{App, AppError, AppResult};

pub mod events {
    pub const SESSION_SUMMARY_CREATED: &str = "session.summary.created";
}

/// How many recent summaries the profile view includes.
const RECENT: usize = 10;

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeProfileView {
    #[serde(flatten)]
    pub profile: KnowledgeProfile,
    /// Newest first.
    pub recent: Vec<SessionSummary>,
    pub note: &'static str,
}

impl App {
    /// Builds the summary of a finished session from stored data.
    async fn build_summary(
        &self,
        session: &LearningSession,
        adi: Option<u8>,
    ) -> AppResult<SessionSummary> {
        let finished_at = session.finished_at.ok_or_else(|| {
            AppError::Conflict("the session is still active: finish it first".into())
        })?;
        let outcome = match session.status {
            SessionStatus::Completed => SessionOutcome::Completed,
            SessionStatus::Abandoned => SessionOutcome::Abandoned,
            SessionStatus::Active => {
                return Err(AppError::Conflict(
                    "the session is still active: finish it first".into(),
                ));
            }
        };
        let task = self.get_task(&session.task_id).await?;
        let f = self.observed_features(session, Some(finished_at)).await?;
        let final_level = match &session.conversation_id {
            Some(c) => self
                .ports
                .engagement
                .state(c)
                .await?
                .map(|s| s.level.as_u8())
                .unwrap_or(0),
            None => 0,
        };
        let answered = (0..5u8)
            .rev()
            .find(|&l| f.answers_by_level[l as usize] > 0)
            .unwrap_or(0);
        let max_level = f.max_level.max(final_level).max(answered).min(4);
        let mut runs = self.ports.sandbox_runs.for_session(&session.id).await?;
        runs.reverse();
        let runs = RunStats::from_runs(
            runs.iter()
                .filter(|r| r.kind == RunKind::Test)
                .map(|r| (r.status, r.summary)),
        );
        let mut topics: Vec<&str> = task
            .docs
            .iter()
            .flat_map(|d| d.topics.iter().map(String::as_str))
            .collect();
        if let TaskSource::Problem { platform, .. } = &task.source {
            topics.push(platform);
        }
        let mut summary = SessionSummary {
            session_id: session.id.clone(),
            task_id: task.id.clone(),
            conversation_id: session.conversation_id.clone(),
            task_title: task.title.clone(),
            task_type: task.task_type,
            difficulty: task.difficulty,
            disciplines: task.disciplines.clone(),
            topics: normalize_topics(topics),
            outcome,
            started_at: session.started_at,
            finished_at,
            duration_minutes: u32::try_from(
                (finished_at - session.started_at).num_minutes().max(0),
            )
            .unwrap_or(u32::MAX),
            commits: f.own_commits,
            help: HelpStats {
                final_level: final_level.min(4),
                max_level,
                answers_by_level: f.answers_by_level,
                conceptual_questions: f.conceptual_questions,
                service_answers: f.service_answers,
                documentation_redirects: f.documentation_redirects,
                user_messages: f.user_messages,
                escalations_accepted: f.escalations_accepted,
                escalations_rejected: f.escalations_rejected,
            },
            runs,
            criteria_total: task.acceptance_criteria.len() as u32,
            criteria_done: task.acceptance_criteria.iter().filter(|c| c.done).count() as u32,
            adi: adi.filter(|_| outcome == SessionOutcome::Completed),
            signals: Vec::new(),
            created_at: est_core::now(),
        };
        summary.signals = summary.derive_signals();
        Ok(summary)
    }

    /// Writes the summary of a session that has just finished.
    pub(crate) async fn summarize_session(
        &self,
        session: &LearningSession,
        adi: Option<u8>,
    ) -> AppResult<SessionSummary> {
        let summary = self.build_summary(session, adi).await?;
        self.ports
            .summaries
            .save_summary(&self.user.id, &summary)
            .await?;
        self.emit(
            DomainEvent::new(
                events::SESSION_SUMMARY_CREATED,
                json!({ "outcome": summary.outcome, "signals": summary.signals }),
            )
            .session(&session.id)
            .task(&session.task_id),
        )
        .await?;
        Ok(summary)
    }

    /// Summarizes finished sessions that have no summary yet. The task's ADI
    /// is attributed to its most recent completed session only.
    pub(crate) async fn backfill_summaries(&self) -> AppResult<Vec<SessionSummary>> {
        let mut stored = self.ports.summaries.summaries(&self.user.id).await?;
        let sessions = self.list_sessions().await?;
        let missing: Vec<&LearningSession> = sessions
            .iter()
            .filter(|s| !s.is_active() && s.finished_at.is_some())
            .filter(|s| !stored.iter().any(|x| x.session_id == s.id))
            .collect();
        if missing.is_empty() {
            return Ok(stored);
        }
        let adi = self.ports.adi.list().await?;
        for s in missing {
            let latest_completed = sessions
                .iter()
                .filter(|x| x.task_id == s.task_id && x.status == SessionStatus::Completed)
                .max_by_key(|x| x.finished_at)
                .map(|x| &x.id);
            let score = (latest_completed == Some(&s.id))
                .then(|| adi.iter().find(|a| a.task_id == s.task_id).map(|a| a.score))
                .flatten();
            let summary = match self.build_summary(s, score).await {
                Ok(summary) => summary,
                // A task removed since; nothing to summarize.
                Err(AppError::NotFound(_)) => continue,
                Err(e) => return Err(e),
            };
            self.ports
                .summaries
                .save_summary(&self.user.id, &summary)
                .await?;
            stored.push(summary);
        }
        stored.sort_by_key(|s| std::cmp::Reverse(s.finished_at));
        Ok(stored)
    }

    /// The summary of a finished session.
    pub async fn session_summary(&self, id: &SessionId) -> AppResult<SessionSummary> {
        let session = self.get_session(id).await?;
        if session.is_active() {
            return Err(AppError::Conflict(
                "the session is still active: finish it first".into(),
            ));
        }
        if let Some(s) = self.ports.summaries.summary(id).await? {
            return Ok(s);
        }
        self.backfill_summaries()
            .await?
            .into_iter()
            .find(|s| &s.session_id == id)
            .ok_or_else(|| AppError::NotFound("session summary".into()))
    }

    /// The Knowledge Profile built from all summaries, with discipline
    /// names in `lang`.
    pub async fn knowledge_profile(&self, lang: Option<&str>) -> AppResult<KnowledgeProfileView> {
        let lang = normalize_lang(lang);
        let summaries = self.backfill_summaries().await?;
        let mut profile = KnowledgeProfile::build(&summaries);
        let catalog = est_discipline::catalog();
        for t in profile
            .topics
            .iter_mut()
            .filter(|t| t.kind == TopicKind::Discipline)
        {
            if let Some(d) = catalog.iter().find(|d| d.id == t.id) {
                t.label = est_discipline::name_in(&d.name, lang).to_owned();
            }
        }
        Ok(KnowledgeProfileView {
            profile,
            recent: summaries.into_iter().take(RECENT).collect(),
            note: "Built in code from session summaries (observed events only). \
                   Advice, not assessment: it changes neither ADI nor the Skill Matrix.",
        })
    }
}
