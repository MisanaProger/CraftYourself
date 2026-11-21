//! Session Summary and Knowledge Profile (architecture v0.3, stage 6).
//!
//! A summary is written in code when a session ends, from observed data
//! only: help levels and answers, escalations, sandbox test runs, commits,
//! acceptance criteria and the final ADI. It holds counts, ids and the task's
//! title — never messages or code. Signals are fixed codes derived from those
//! counts by the rules below, so the interface can translate them and the
//! model can only read them.
//!
//! The Knowledge Profile aggregates summaries per topic (ESM discipline,
//! problem tag, task type). It is advice for the learner and the
//! recommendations, not an assessment: it changes neither ADI nor the Skill
//! Matrix, and a topic without completed or abandoned work is absent rather
//! than zero.

use std::collections::BTreeMap;

use est_core::{ConversationId, SessionId, TaskId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::{Difficulty, RunStatus, SessionOutcome, TaskType, TestSummary};

/// At most this many topic tags are kept per summary.
pub const MAX_TOPICS: usize = 10;

/// Help the learner received in the Issue's conversation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelpStats {
    /// Engagement level when the session ended (0–4).
    pub final_level: u8,
    /// Highest level reached by consent.
    pub max_level: u8,
    /// Mentor answers about the solution, per level L0..L4.
    pub answers_by_level: [u32; 5],
    pub conceptual_questions: u32,
    /// Acceptance criteria and translations (no ADI weight).
    pub service_answers: u32,
    pub documentation_redirects: u32,
    pub user_messages: u32,
    pub escalations_accepted: u32,
    pub escalations_rejected: u32,
}

impl HelpStats {
    /// Answers at L3–L4: pseudocode, code fragments, a detailed solution.
    pub fn solution_answers(&self) -> u32 {
        self.answers_by_level[3] + self.answers_by_level[4]
    }
}

/// Test runs in the sandbox during the session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStats {
    pub test_runs: u32,
    pub passed: u32,
    pub failed: u32,
    /// Status of the last finished test run.
    pub last: Option<RunStatus>,
    /// Counts of the last run whose output was recognised.
    pub last_tests: Option<TestSummary>,
    /// A failing run was later followed by a passing one.
    pub recovered: bool,
}

impl RunStats {
    /// `runs`: finished test runs, oldest first.
    pub fn from_runs(runs: impl IntoIterator<Item = (RunStatus, Option<TestSummary>)>) -> Self {
        let mut s = RunStats::default();
        let mut failed_before = false;
        for (status, summary) in runs {
            match status {
                RunStatus::Running => continue,
                RunStatus::Passed => {
                    s.passed += 1;
                    s.recovered |= failed_before;
                }
                _ => {
                    s.failed += 1;
                    failed_before = true;
                }
            }
            s.test_runs += 1;
            s.last = Some(status);
            if summary.is_some() {
                s.last_tests = summary;
            }
        }
        s
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    /// Completed without answers above L1 and without raising the level past it.
    WorkedIndependently,
    /// Answers at L3–L4 were used.
    HeavyHelp,
    /// The learner declined a proposed escalation.
    DeclinedHelp,
    UsedDocumentation,
    /// The last test run passed.
    TestsGreen,
    /// Tests failed at some point and passed later.
    TestsRecovered,
    /// The last test run failed.
    TestsRedAtFinish,
    NoTestRuns,
    CriteriaMet,
    CriteriaOpen,
    OwnCommits,
    NoCommits,
    Abandoned,
}

/// What happened in one finished session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: SessionId,
    pub task_id: TaskId,
    pub conversation_id: Option<ConversationId>,
    pub task_title: String,
    pub task_type: TaskType,
    pub difficulty: Difficulty,
    /// ESM discipline ids of the task.
    pub disciplines: Vec<String>,
    /// Topic tags of the task (problem tags, documentation topics), normalized.
    pub topics: Vec<String>,
    pub outcome: SessionOutcome,
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
    pub duration_minutes: u32,
    pub commits: u32,
    pub help: HelpStats,
    pub runs: RunStats,
    pub criteria_total: u32,
    pub criteria_done: u32,
    /// Final ADI of the Issue, for a completed session.
    pub adi: Option<u8>,
    /// Commit review score of the learner's commits (stage 9), when any.
    #[serde(default)]
    pub commit_quality: Option<u8>,
    pub signals: Vec<Signal>,
    pub created_at: Timestamp,
}

/// Lowercase, trimmed, letters/digits/spaces/dashes, at most 40 characters.
pub fn normalize_topic(t: &str) -> Option<String> {
    let t: String = t
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-'))
        .take(40)
        .collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    (!t.is_empty()).then_some(t)
}

/// Normalized, deduplicated topics, at most [`MAX_TOPICS`].
pub fn normalize_topics<'a>(topics: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in topics.into_iter().filter_map(normalize_topic) {
        if !out.contains(&t) && out.len() < MAX_TOPICS {
            out.push(t);
        }
    }
    out
}

impl SessionSummary {
    /// Derives [`Signal`]s from the counts (fixed rules, no model).
    pub fn derive_signals(&self) -> Vec<Signal> {
        use Signal::*;
        let h = &self.help;
        let completed = self.outcome == SessionOutcome::Completed;
        let mut s = Vec::new();
        if !completed {
            s.push(Abandoned);
        }
        let above_l1: u32 = h.answers_by_level[2..].iter().sum();
        if completed && above_l1 == 0 && h.max_level <= 1 {
            s.push(WorkedIndependently);
        }
        if h.solution_answers() > 0 {
            s.push(HeavyHelp);
        }
        if h.escalations_rejected > 0 {
            s.push(DeclinedHelp);
        }
        if h.documentation_redirects > 0 {
            s.push(UsedDocumentation);
        }
        match self.runs.last {
            None => s.push(NoTestRuns),
            Some(RunStatus::Passed) => s.push(TestsGreen),
            Some(_) => s.push(TestsRedAtFinish),
        }
        if self.runs.recovered {
            s.push(TestsRecovered);
        }
        if self.criteria_total > 0 {
            s.push(if self.criteria_done == self.criteria_total {
                CriteriaMet
            } else {
                CriteriaOpen
            });
        }
        if self.commits > 0 {
            s.push(OwnCommits);
        } else if completed {
            s.push(NoCommits);
        }
        s.sort();
        s
    }

    pub fn is_completed(&self) -> bool {
        self.outcome == SessionOutcome::Completed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopicKind {
    Discipline,
    Tag,
    TaskType,
}

/// How the learner stands on a topic, by the rules of [`Standing::from_counts`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Standing {
    /// Most completed Issues were solved without help above L1.
    Independent,
    Developing,
    /// Nothing completed, more abandoned than completed, or most completed
    /// Issues needed L3–L4 answers.
    NeedsPractice,
}

impl Standing {
    pub fn from_counts(completed: u32, abandoned: u32, independent: u32, heavy_help: u32) -> Self {
        if completed == 0 || abandoned > completed || heavy_help * 2 > completed {
            Standing::NeedsPractice
        } else if independent * 2 >= completed {
            Standing::Independent
        } else {
            Standing::Developing
        }
    }
}

/// How much evidence a topic has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

impl Confidence {
    pub fn from_sessions(n: u32) -> Self {
        match n {
            0..=2 => Confidence::Low,
            3..=5 => Confidence::Medium,
            _ => Confidence::High,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopicKnowledge {
    pub kind: TopicKind,
    pub id: String,
    /// Display name; the id unless the application knows a better one.
    pub label: String,
    pub sessions: u32,
    pub completed: u32,
    pub abandoned: u32,
    /// Completed with [`Signal::WorkedIndependently`].
    pub independent: u32,
    /// Sessions with [`Signal::HeavyHelp`].
    pub heavy_help: u32,
    /// Completed sessions whose last test run passed.
    pub tests_green: u32,
    /// Mean final ADI of completed sessions that have one.
    pub average_adi: Option<f64>,
    pub last_practised: Timestamp,
    pub standing: Standing,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeProfile {
    pub sessions: u32,
    pub completed: u32,
    pub abandoned: u32,
    /// Most recently practised first.
    pub topics: Vec<TopicKnowledge>,
}

#[derive(Default)]
struct Acc {
    sessions: u32,
    completed: u32,
    abandoned: u32,
    independent: u32,
    heavy_help: u32,
    tests_green: u32,
    adi: Vec<u8>,
    last: Option<Timestamp>,
}

impl KnowledgeProfile {
    /// Aggregates summaries per topic. Order of `summaries` does not matter.
    pub fn build(summaries: &[SessionSummary]) -> Self {
        let mut topics: BTreeMap<(TopicKind, String), Acc> = BTreeMap::new();
        for s in summaries {
            let task_type = serde_json::to_value(s.task_type)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            let keys = s
                .disciplines
                .iter()
                .map(|d| (TopicKind::Discipline, d.clone()))
                .chain(s.topics.iter().map(|t| (TopicKind::Tag, t.clone())))
                .chain(std::iter::once((TopicKind::TaskType, task_type)));
            let signals = &s.signals;
            for key in keys {
                let a = topics.entry(key).or_default();
                a.sessions += 1;
                if s.is_completed() {
                    a.completed += 1;
                    a.independent += u32::from(signals.contains(&Signal::WorkedIndependently));
                    a.tests_green += u32::from(signals.contains(&Signal::TestsGreen));
                    a.adi.extend(s.adi);
                } else {
                    a.abandoned += 1;
                }
                a.heavy_help += u32::from(signals.contains(&Signal::HeavyHelp));
                a.last = a.last.max(Some(s.finished_at));
            }
        }
        let mut topics: Vec<TopicKnowledge> = topics
            .into_iter()
            .filter_map(|((kind, id), a)| {
                Some(TopicKnowledge {
                    kind,
                    label: id.clone(),
                    id,
                    sessions: a.sessions,
                    completed: a.completed,
                    abandoned: a.abandoned,
                    independent: a.independent,
                    heavy_help: a.heavy_help,
                    tests_green: a.tests_green,
                    average_adi: (!a.adi.is_empty()).then(|| {
                        let sum: f64 = a.adi.iter().map(|&x| f64::from(x)).sum();
                        (sum / a.adi.len() as f64 * 10.0).round() / 10.0
                    }),
                    last_practised: a.last?,
                    standing: Standing::from_counts(
                        a.completed,
                        a.abandoned,
                        a.independent,
                        a.heavy_help,
                    ),
                    confidence: Confidence::from_sessions(a.sessions),
                })
            })
            .collect();
        topics.sort_by(|a, b| {
            b.last_practised
                .cmp(&a.last_practised)
                .then(a.kind.cmp(&b.kind))
                .then(a.id.cmp(&b.id))
        });
        let completed = summaries.iter().filter(|s| s.is_completed()).count() as u32;
        KnowledgeProfile {
            sessions: summaries.len() as u32,
            completed,
            abandoned: summaries.len() as u32 - completed,
            topics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(outcome: SessionOutcome, help: HelpStats, runs: RunStats) -> SessionSummary {
        let now = est_core::now();
        let mut s = SessionSummary {
            session_id: SessionId::generate(),
            task_id: TaskId::generate(),
            conversation_id: None,
            task_title: "Fix range".into(),
            task_type: TaskType::BugFix,
            difficulty: Difficulty::Beginner,
            disciplines: vec!["debugging-testing".into()],
            topics: vec!["math".into()],
            outcome,
            started_at: now,
            finished_at: now,
            duration_minutes: 0,
            commits: 1,
            help,
            runs,
            criteria_total: 2,
            criteria_done: 2,
            adi: (outcome == SessionOutcome::Completed).then_some(20),
            commit_quality: None,
            signals: vec![],
            created_at: now,
        };
        s.signals = s.derive_signals();
        s
    }

    #[test]
    fn runs_track_recovery() {
        use RunStatus::*;
        let t = Some(TestSummary {
            passed: 3,
            failed: 0,
            ignored: 0,
        });
        let r = RunStats::from_runs([(Failed, None), (Running, None), (Passed, t)]);
        assert_eq!((r.test_runs, r.passed, r.failed), (2, 1, 1));
        assert!(r.recovered);
        assert_eq!((r.last, r.last_tests), (Some(Passed), t));
        let r = RunStats::from_runs([(Passed, None), (TimedOut, None)]);
        assert!(!r.recovered);
        assert_eq!(r.last, Some(TimedOut));
        assert_eq!(RunStats::from_runs([]), RunStats::default());
    }

    #[test]
    fn signals_follow_the_counts() {
        use Signal::*;
        let green = RunStats::from_runs([(RunStatus::Failed, None), (RunStatus::Passed, None)]);
        let s = summary(SessionOutcome::Completed, HelpStats::default(), green);
        assert_eq!(
            s.signals,
            [
                WorkedIndependently,
                TestsGreen,
                TestsRecovered,
                CriteriaMet,
                OwnCommits
            ]
        );

        let help = HelpStats {
            answers_by_level: [1, 0, 2, 1, 0],
            max_level: 3,
            escalations_rejected: 1,
            documentation_redirects: 2,
            ..Default::default()
        };
        let mut s = summary(SessionOutcome::Abandoned, help, RunStats::default());
        s.commits = 0;
        s.criteria_done = 1;
        assert_eq!(
            s.derive_signals(),
            [
                HeavyHelp,
                DeclinedHelp,
                UsedDocumentation,
                NoTestRuns,
                CriteriaOpen,
                Abandoned
            ]
        );

        // L2 answers rule out "independent" even at a low final level.
        let help = HelpStats {
            answers_by_level: [0, 0, 1, 0, 0],
            ..Default::default()
        };
        let s = summary(SessionOutcome::Completed, help, RunStats::default());
        assert!(!s.signals.contains(&WorkedIndependently));
    }

    #[test]
    fn topics_are_normalized() {
        assert_eq!(
            normalize_topics(["  Binary  Search ", "DP", "dp", "<b>", "", "Графы"]),
            ["binary search", "dp", "b", "графы"]
        );
        assert_eq!(normalize_topics(["a"; 3].into_iter().chain(["b"])).len(), 2);
    }

    #[test]
    fn standing_rules() {
        use Standing::*;
        assert_eq!(Standing::from_counts(0, 1, 0, 0), NeedsPractice);
        assert_eq!(Standing::from_counts(1, 2, 1, 0), NeedsPractice);
        assert_eq!(Standing::from_counts(2, 0, 2, 2), NeedsPractice);
        assert_eq!(Standing::from_counts(2, 1, 1, 0), Independent);
        assert_eq!(Standing::from_counts(3, 0, 1, 1), Developing);
    }

    #[test]
    fn profile_aggregates_per_topic() {
        let independent = summary(
            SessionOutcome::Completed,
            HelpStats::default(),
            RunStats::from_runs([(RunStatus::Passed, None)]),
        );
        let mut heavy = summary(
            SessionOutcome::Completed,
            HelpStats {
                answers_by_level: [0, 0, 0, 0, 2],
                max_level: 4,
                ..Default::default()
            },
            RunStats::default(),
        );
        heavy.topics = vec!["graphs".into()];
        heavy.adi = Some(70);
        let mut abandoned = summary(
            SessionOutcome::Abandoned,
            HelpStats::default(),
            RunStats::default(),
        );
        abandoned.topics = vec!["graphs".into()];
        abandoned.task_type = TaskType::Algorithm;

        let p = KnowledgeProfile::build(&[independent, heavy, abandoned]);
        assert_eq!((p.sessions, p.completed, p.abandoned), (3, 2, 1));
        let topic = |kind, id: &str| {
            p.topics
                .iter()
                .find(|t| t.kind == kind && t.id == id)
                .unwrap()
                .clone()
        };
        let d = topic(TopicKind::Discipline, "debugging-testing");
        assert_eq!((d.sessions, d.completed, d.abandoned), (3, 2, 1));
        assert_eq!((d.independent, d.heavy_help, d.tests_green), (1, 1, 1));
        assert_eq!(d.average_adi, Some(45.0));
        assert_eq!(d.standing, Standing::Independent);
        assert_eq!(d.confidence, Confidence::Medium);
        assert_eq!(
            topic(TopicKind::Tag, "math").standing,
            Standing::Independent
        );
        let graphs = topic(TopicKind::Tag, "graphs");
        assert_eq!(graphs.standing, Standing::NeedsPractice);
        assert_eq!(graphs.average_adi, Some(70.0));
        assert_eq!(topic(TopicKind::TaskType, "bug-fix").sessions, 2);
        assert_eq!(
            topic(TopicKind::TaskType, "algorithm").standing,
            Standing::NeedsPractice
        );
        assert!(KnowledgeProfile::build(&[]).topics.is_empty());
    }
}
