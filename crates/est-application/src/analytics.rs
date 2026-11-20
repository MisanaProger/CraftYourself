//! ADI, Engineering Skill Matrix and privacy (architecture v0.3, §10–12, §20).
//!
//! ADI and the Skill Matrix are computed independently and never merged.

use est_core::{ConversationId, DomainEvent, events};
use est_dependency::{
    Aggregate, DecayParams, GAME_STATUS_DISCLAIMER, ObservedFeatures, ScoredIssue,
};
use est_discipline::MatrixDiscipline;
use est_domain::{LearningSession, TaskStatus};
use serde::Serialize;
use serde_json::json;

use crate::ports::{AdiAssessment, ProviderInfo};
use crate::{App, AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
pub struct AdiOverview {
    pub aggregate: Aggregate,
    pub status_label: Option<&'static str>,
    pub disclaimer: &'static str,
    pub params: DecayParams,
    /// Per-Issue assessments, newest first, with task titles.
    pub history: Vec<AdiHistoryItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdiHistoryItem {
    #[serde(flatten)]
    pub assessment: AdiAssessment,
    pub task_title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillMatrix {
    pub catalog_version: &'static str,
    pub disciplines: Vec<MatrixDiscipline>,
    pub note: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrivacySettings {
    pub storage: &'static str,
    pub llm: Option<ProviderInfo>,
    /// Long-term analytics based on history (always on in this version).
    pub history_analytics: bool,
}

impl App {
    /// Observed features of a completed Issue (v0.3 §10.3–10.4).
    async fn observed_features(&self, session: &LearningSession) -> AppResult<ObservedFeatures> {
        let mut f = ObservedFeatures {
            own_commits: self.commits_since_base(session).await?,
            ..Default::default()
        };
        let Some(conversation) = &session.conversation_id else {
            return Ok(f);
        };
        for e in self
            .ports
            .events
            .for_conversation(conversation, None)
            .await?
        {
            let p = &e.event.payload;
            match e.event.kind.as_str() {
                events::AI_ANSWER => {
                    let level = p["level"].as_u64().unwrap_or(0).min(4) as usize;
                    let kind = &p["query_kind"]["type"];
                    if kind == "acceptance_criteria" || kind == "translation" {
                        f.service_answers += 1;
                    } else if kind == "conceptual" {
                        f.conceptual_questions += 1;
                    } else {
                        f.answers_by_level[level] += 1;
                    }
                }
                events::DOCS_REDIRECT => f.documentation_redirects += 1,
                crate::sandbox::events::RUN_FINISHED if p["kind"] == "test" => f.test_runs += 1,
                events::ESCALATION_ACCEPTED => {
                    f.escalations_accepted += 1;
                    f.max_level = f.max_level.max(p["to"].as_u64().unwrap_or(0) as u8);
                }
                events::ESCALATION_REJECTED if p["reason"] == "declined" => {
                    f.escalations_rejected += 1
                }
                events::MESSAGE_CREATED if p["role"] == "user" => f.user_messages += 1,
                _ => {}
            }
        }
        Ok(f)
    }

    /// Final ADI of a completed Issue, stored with its scoring version.
    pub(crate) async fn assess_adi(&self, session: &LearningSession) -> AppResult<AdiAssessment> {
        let features = self.observed_features(session).await?;
        let s = est_dependency::score_issue(&features);
        let saved = self
            .ports
            .adi
            .save(&AdiAssessment {
                task_id: session.task_id.clone(),
                score: s.score,
                scoring_version: s.scoring_version,
                observed_features: serde_json::to_value(&features).unwrap_or_default(),
                factors: serde_json::to_value(&s.factors).unwrap_or_default(),
                explanation: s.explanation,
                confidence: serde_json::to_value(s.confidence)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                created_at: est_core::now(),
                sequence: 0,
            })
            .await?;
        self.emit(
            DomainEvent::new(
                events::ADI_ASSESSED,
                json!({ "score": saved.score, "scoring_version": saved.scoring_version }),
            )
            .task(&session.task_id)
            .session(&session.id),
        )
        .await?;
        Ok(saved)
    }

    pub async fn adi_overview(&self) -> AppResult<AdiOverview> {
        let assessments = self.ports.adi.list().await?;
        let scored: Vec<ScoredIssue> = assessments
            .iter()
            .map(|a| ScoredIssue {
                score: f64::from(a.score),
                completed_at: a.created_at,
                sequence: a.sequence as u64,
            })
            .collect();
        let aggregate =
            est_dependency::aggregate(&scored, est_core::now(), &self.settings.adi_decay);
        let mut history = Vec::new();
        for a in assessments.into_iter().rev() {
            let task_title = self.ports.tasks.get(&a.task_id).await?.map(|t| t.title);
            history.push(AdiHistoryItem {
                assessment: a,
                task_title,
            });
        }
        Ok(AdiOverview {
            status_label: aggregate.status.map(|s| s.label_ru()),
            aggregate,
            disclaimer: GAME_STATUS_DISCLAIMER,
            params: self.settings.adi_decay,
            history,
        })
    }

    pub async fn skill_matrix(&self, lang: &str) -> AppResult<SkillMatrix> {
        let done: Vec<_> = self
            .ports
            .tasks
            .list(None)
            .await?
            .into_iter()
            .filter(|t| t.status == TaskStatus::Done)
            .collect();
        let disciplines = est_discipline::matrix(lang, |id| {
            done.iter()
                .filter(|t| t.disciplines.iter().any(|d| d == id))
                .count() as u32
        });
        Ok(SkillMatrix {
            catalog_version: est_discipline::CATALOG_VERSION,
            disciplines,
            note: "Оценка навыков по evidence появится на этапе 7. Пока все навыки — «не оценено»: \
                   отсутствие данных не равно нулевому навыку.",
        })
    }

    pub fn privacy_settings(&self) -> PrivacySettings {
        PrivacySettings {
            storage: "local",
            llm: self.ports.llm.as_ref().map(|l| l.info()),
            history_analytics: true,
        }
    }

    /// Everything stored about the learner, as one JSON document.
    pub async fn export_data(&self) -> AppResult<serde_json::Value> {
        let mut data = self.ports.export.export_all().await?;
        if let Some(obj) = data.as_object_mut() {
            obj.insert("format".into(), json!("craft-yourself-export/1"));
            obj.insert("exported_at".into(), json!(est_core::now()));
        }
        Ok(data)
    }

    /// Deletes a conversation with its messages, engagement history, events
    /// and the ADI of its Issue.
    pub async fn delete_conversation(&self, id: &ConversationId) -> AppResult<()> {
        let c = self.get_conversation(id).await?;
        if let Some(task) = &c.task_id
            && self.ports.sessions.active_for_task(task).await?.is_some()
        {
            return Err(AppError::Conflict(
                "finish the Issue's active session before deleting its conversation".into(),
            ));
        }
        self.ports.conversations.delete(id).await?;
        self.ports
            .publisher
            .publish(&DomainEvent::new(events::CONVERSATION_DELETED, json!({})).conversation(id));
        Ok(())
    }
}
