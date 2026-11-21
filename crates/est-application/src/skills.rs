//! Skill Matrix evidence (architecture v0.3, §11.4; stage 7).
//!
//! Evidence comes from observed results only:
//! - quiz attempts (Theory) and artifact results (Practice) of lessons that
//!   name their skills in `program.toml`;
//! - completed practice projects and Issues, from their session summaries:
//!   Practice (last test run and acceptance criteria), Diagnosis (failing
//!   tests fixed) and Independence (highest help level used).
//!
//! Reading a lesson, abandoning an Issue or a missing measurement is not
//! evidence. The ADI is never an input: ADI and the Skill Matrix stay
//! independent. Scores and statuses are computed by
//! [`est_discipline::assess`].

use std::collections::{BTreeMap, BTreeSet};

use est_discipline::{Dimension, Evidence, EvidenceSource};
use est_domain::{RunStatus, SessionSummary};
use est_learning::{LessonKind, pick};

use crate::ports::{LessonProgress, LessonResult};
use crate::{App, AppResult};

/// Targets of a piece of evidence: `(discipline, skill)`.
type Targets = Vec<(String, Option<String>)>;

/// Independence by the highest help level used in the Issue.
pub fn independence_score(level: u8) -> u8 {
    match level {
        0 | 1 => 100,
        2 => 70,
        3 => 40,
        _ => 20,
    }
}

/// Evidence from one session summary; nothing for an abandoned session.
pub fn session_evidence(
    s: &SessionSummary,
    source: EvidenceSource,
    targets: &[(String, Option<String>)],
) -> Vec<Evidence> {
    if !s.is_completed() {
        return Vec::new();
    }
    let mut scores: Vec<(Dimension, u8)> = Vec::new();
    let mut parts: Vec<u32> = Vec::new();
    match s.runs.last {
        Some(RunStatus::Passed) => parts.push(100),
        Some(_) => parts.push(0),
        None => {}
    }
    if let Some(share) = (s.criteria_done * 100).checked_div(s.criteria_total) {
        parts.push(share);
    }
    if !parts.is_empty() {
        let n = parts.len() as u32;
        scores.push((
            Dimension::Practice,
            ((parts.iter().sum::<u32>() + n / 2) / n) as u8,
        ));
    }
    if s.runs.recovered {
        scores.push((Dimension::Diagnosis, 100));
    }
    // Without the conversation there is no record of the help received.
    if s.conversation_id.is_some() {
        let answered = (0..5u8)
            .rev()
            .find(|&l| s.help.answers_by_level[l as usize] > 0)
            .unwrap_or(0);
        scores.push((
            Dimension::Independence,
            independence_score(s.help.max_level.max(answered)),
        ));
    }
    let mut out = Vec::new();
    for (discipline, skill) in targets {
        for &(dimension, score) in &scores {
            out.push(Evidence {
                discipline: discipline.clone(),
                skill: skill.clone(),
                source,
                dimension,
                score,
                reference: s.session_id.to_string(),
                title: s.task_title.clone(),
                observed_at: s.finished_at,
            });
        }
    }
    out
}

/// Skills a lesson names, or its program's disciplines as a whole.
fn lesson_targets(program: &est_learning::Program, lesson: &est_learning::LessonDef) -> Targets {
    let refs = lesson.skill_refs();
    if refs.is_empty() {
        program
            .disciplines
            .iter()
            .map(|d| (d.clone(), None))
            .collect()
    } else {
        refs.into_iter()
            .map(|(d, s)| (d.to_owned(), Some(s.to_owned())))
            .collect()
    }
}

impl App {
    /// All evidence, oldest first. Titles are in `lang`.
    pub(crate) async fn skill_evidence(&self, lang: &str) -> AppResult<Vec<Evidence>> {
        let progress: Vec<LessonProgress> =
            self.ports.learning.progress(&self.user.id, None).await?;
        let mut results: Vec<LessonResult> = self
            .ports
            .lesson_results
            .lesson_results(&self.user.id)
            .await?;
        // Scores recorded before results were kept: the best one, once.
        let recorded: BTreeSet<(String, String)> = results
            .iter()
            .map(|r| (r.program_id.clone(), r.lesson_id.clone()))
            .collect();
        let mut programs: BTreeMap<String, Option<est_learning::Program>> = BTreeMap::new();
        let mut program = |id: &str| -> Option<est_learning::Program> {
            programs
                .entry(id.to_owned())
                // Generated programs name no disciplines yet: no evidence.
                .or_insert_with(|| est_learning::program(id).ok())
                .clone()
        };
        for p in &progress {
            let Some(score) = p.score else { continue };
            if recorded.contains(&(p.program_id.clone(), p.lesson_id.clone())) {
                continue;
            }
            let Some(prog) = program(&p.program_id) else {
                continue;
            };
            let source = match est_learning::lesson(&prog, &p.lesson_id).map(|l| l.kind) {
                Ok(LessonKind::Quiz) => EvidenceSource::Quiz,
                Ok(LessonKind::Artifact) => EvidenceSource::Artifact,
                _ => continue,
            };
            results.push(LessonResult {
                program_id: p.program_id.clone(),
                lesson_id: p.lesson_id.clone(),
                source,
                score,
                observed_at: p.updated_at,
            });
        }

        let mut evidence = Vec::new();
        for r in &results {
            let Some(prog) = program(&r.program_id) else {
                continue;
            };
            let Ok(lesson) = est_learning::lesson(&prog, &r.lesson_id) else {
                continue;
            };
            let dimension = match r.source {
                EvidenceSource::Quiz => Dimension::Theory,
                _ => Dimension::Practice,
            };
            for (discipline, skill) in lesson_targets(&prog, lesson) {
                evidence.push(Evidence {
                    discipline,
                    skill,
                    source: r.source,
                    dimension,
                    score: r.score,
                    reference: format!("{}/{}", r.program_id, r.lesson_id),
                    title: pick(&lesson.title, lang),
                    observed_at: r.observed_at,
                });
            }
        }

        // Practice lessons know their Issue by its task id.
        let mut practice: BTreeMap<String, Targets> = BTreeMap::new();
        for p in &progress {
            let Some(task) = p.detail.get("task_id").and_then(|t| t.as_str()) else {
                continue;
            };
            let Some(prog) = program(&p.program_id) else {
                continue;
            };
            if let Ok(lesson) = est_learning::lesson(&prog, &p.lesson_id)
                && lesson.kind == LessonKind::Practice
            {
                practice.insert(task.to_owned(), lesson_targets(&prog, lesson));
            }
        }
        for s in self.backfill_summaries().await? {
            let (source, targets) = match practice.get(s.task_id.as_str()) {
                Some(t) => (EvidenceSource::Practice, t.clone()),
                None => (
                    EvidenceSource::Issue,
                    s.disciplines.iter().map(|d| (d.clone(), None)).collect(),
                ),
            };
            evidence.extend(session_evidence(&s, source, &targets));
        }
        evidence.sort_by_key(|e| e.observed_at);
        Ok(evidence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use est_core::{ConversationId, SessionId, TaskId};
    use est_domain::{Difficulty, HelpStats, RunStats, SessionOutcome, TaskType};

    fn summary() -> SessionSummary {
        let now = est_core::now();
        SessionSummary {
            session_id: SessionId::from("s1"),
            task_id: TaskId::from("t1"),
            conversation_id: Some(ConversationId::from("c1")),
            task_title: "Fix range".into(),
            task_type: TaskType::BugFix,
            difficulty: Difficulty::Beginner,
            disciplines: vec!["debugging-testing".into()],
            topics: vec![],
            outcome: SessionOutcome::Completed,
            started_at: now,
            finished_at: now,
            duration_minutes: 5,
            commits: 1,
            help: HelpStats {
                answers_by_level: [2, 0, 1, 0, 0],
                ..Default::default()
            },
            runs: RunStats::from_runs([(RunStatus::Failed, None), (RunStatus::Passed, None)]),
            criteria_total: 2,
            criteria_done: 1,
            adi: Some(90),
            commit_quality: None,
            signals: vec![],
            created_at: now,
        }
    }

    fn scores(e: &[Evidence]) -> Vec<(Dimension, u8)> {
        e.iter().map(|e| (e.dimension, e.score)).collect()
    }

    #[test]
    fn sessions_give_evidence_without_the_adi() {
        let targets = [("debugging-testing".to_owned(), None)];
        let s = summary();
        let e = session_evidence(&s, EvidenceSource::Issue, &targets);
        // Practice: tests green (100) and half the criteria (50).
        assert_eq!(
            scores(&e),
            [
                (Dimension::Practice, 75),
                (Dimension::Diagnosis, 100),
                (Dimension::Independence, 70)
            ]
        );
        assert!(
            e.iter()
                .all(|e| e.reference == "s1" && e.title == "Fix range")
        );
        let mut low_adi = s.clone();
        low_adi.adi = Some(0);
        assert_eq!(
            scores(&session_evidence(&low_adi, EvidenceSource::Issue, &targets)),
            scores(&e),
            "the ADI is not an input"
        );

        // Nothing measured → no practice score; no conversation → no independence.
        let mut bare = s.clone();
        bare.runs = RunStats::default();
        bare.criteria_total = 0;
        bare.conversation_id = None;
        assert!(session_evidence(&bare, EvidenceSource::Issue, &targets).is_empty());

        let mut abandoned = s;
        abandoned.outcome = SessionOutcome::Abandoned;
        assert!(session_evidence(&abandoned, EvidenceSource::Issue, &targets).is_empty());
    }

    #[test]
    fn independence_by_level() {
        assert_eq!(
            [0, 1, 2, 3, 4].map(independence_score),
            [100, 100, 70, 40, 20]
        );
    }
}
