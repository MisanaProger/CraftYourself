//! AI Dependency Index — ADI (architecture v0.3, §10).
//!
//! ADI measures *observed* dependence on AI while solving an Issue, 0–100.
//! It is not a measure of skill and is never merged with the Skill Matrix.
//!
//! * [`score_issue`] — deterministic per-Issue score from observed features
//!   (`scoring_version` [`SCORING_VERSION`]); a structured model assessment
//!   can be added later as a further input.
//! * [`aggregate`] — average with double decay by age and by number of
//!   Issues completed since (§10.6).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const SCORING_VERSION: &str = "observed-0.1";

/// Decay parameters (preliminary MVP values, configurable).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DecayParams {
    /// Half-life in days, `T_t`.
    pub half_life_days: f64,
    /// Half-life in completed Issues, `T_n`.
    pub half_life_issues: f64,
    /// Fewer completed Issues than this → the aggregate is provisional.
    pub min_issues_for_status: usize,
}

impl Default for DecayParams {
    fn default() -> Self {
        Self {
            half_life_days: 30.0,
            half_life_issues: 10.0,
            min_issues_for_status: 5,
        }
    }
}

/// Observed features of one completed Issue.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ObservedFeatures {
    /// Mentor answers delivered at each level L0..L4.
    pub answers_by_level: [u32; 5],
    /// Conceptual questions (small weight by design, §10.9).
    pub conceptual_questions: u32,
    /// Test runs started by the learner in the sandbox.
    #[serde(default)]
    pub test_runs: u32,
    /// Acceptance criteria and translations: learning support, no weight.
    #[serde(default)]
    pub service_answers: u32,
    /// Questions answered by routing to documentation / source code.
    pub documentation_redirects: u32,
    /// Escalations accepted / rejected by the learner.
    pub escalations_accepted: u32,
    pub escalations_rejected: u32,
    /// Commits made by the learner since the task's base revision.
    pub own_commits: u32,
    /// Messages from the learner in the Issue conversation.
    pub user_messages: u32,
    /// Highest engagement level reached.
    pub max_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Factor {
    pub description: String,
    /// Positive raises ADI (more dependence), negative lowers it.
    pub points: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IssueScore {
    pub score: u8,
    pub scoring_version: String,
    pub factors: Vec<Factor>,
    pub confidence: Confidence,
    pub explanation: String,
}

/// Scores one completed Issue from observed events only.
///
/// What counts is the help actually *delivered*, not escalations as such:
/// raising the level without using it costs nothing (§10.4).
pub fn score_issue(f: &ObservedFeatures) -> IssueScore {
    let mut factors = Vec::new();
    let mut add = |n: u32, per: f64, cap: f64, what: &str| {
        if n > 0 {
            factors.push(Factor {
                description: format!("{what}: {n}"),
                points: (f64::from(n) * per).clamp(-cap, cap),
            });
        }
    };
    add(
        f.answers_by_level[4],
        18.0,
        60.0,
        "подробные решения от ИИ (L4)",
    );
    add(
        f.answers_by_level[3],
        9.0,
        40.0,
        "фрагменты реализации от ИИ (L3)",
    );
    add(
        f.answers_by_level[2],
        3.0,
        18.0,
        "объяснения и диагностика от ИИ (L2)",
    );
    add(f.answers_by_level[1], 1.0, 8.0, "подсказки (L1)");
    add(f.conceptual_questions, 0.5, 4.0, "концептуальные вопросы");
    add(f.own_commits, -6.0, 18.0, "собственные коммиты");
    add(f.test_runs, -2.0, 8.0, "самостоятельные запуски тестов");
    add(
        f.escalations_rejected,
        -2.0,
        6.0,
        "отклонённые повышения уровня",
    );

    let raw: f64 = 15.0 + factors.iter().map(|x| x.points).sum::<f64>();
    let score = raw.round().clamp(0.0, 100.0) as u8;

    let observed = f.user_messages + f.own_commits;
    let confidence = match observed {
        0..=2 => Confidence::Low,
        3..=9 => Confidence::Medium,
        _ => Confidence::High,
    };
    let explanation = if factors.is_empty() {
        "Наблюдаемых действий мало: базовая оценка без поправок.".to_owned()
    } else {
        "Оценка по наблюдаемым событиям Issue: какой уровень помощи фактически использован \
         и сколько работы выполнено самостоятельно."
            .to_owned()
    };
    IssueScore {
        score,
        scoring_version: SCORING_VERSION.into(),
        factors,
        confidence,
        explanation,
    }
}

/// A finalized per-Issue score for aggregation.
#[derive(Debug, Clone, Copy)]
pub struct ScoredIssue {
    pub score: f64,
    pub completed_at: DateTime<Utc>,
    /// Completion order; ties in time are broken by this.
    pub sequence: u64,
}

/// `w = 2^(-Δt/T_t) · 2^(-Δn/T_n)` (§10.6).
pub fn decay_weight(age_days: f64, issues_after: f64, p: &DecayParams) -> f64 {
    2f64.powf(-age_days.max(0.0) / p.half_life_days)
        * 2f64.powf(-issues_after.max(0.0) / p.half_life_issues)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Aggregate {
    /// Weighted ADI, `None` when there are no completed Issues.
    pub value: Option<f64>,
    pub issues: usize,
    pub provisional: bool,
    pub confidence: Confidence,
    pub status: Option<GameStatus>,
    pub observed_since: Option<DateTime<Utc>>,
}

pub fn aggregate(scores: &[ScoredIssue], now: DateTime<Utc>, p: &DecayParams) -> Aggregate {
    if scores.is_empty() {
        return Aggregate {
            value: None,
            issues: 0,
            provisional: true,
            confidence: Confidence::Low,
            status: None,
            observed_since: None,
        };
    }
    let mut sorted: Vec<_> = scores.to_vec();
    sorted.sort_by_key(|s| (s.completed_at, s.sequence));
    let n = sorted.len();
    let (mut num, mut den) = (0.0, 0.0);
    for (i, s) in sorted.iter().enumerate() {
        let age = (now - s.completed_at).num_seconds() as f64 / 86_400.0;
        let after = (n - 1 - i) as f64;
        let w = decay_weight(age, after, p);
        num += s.score * w;
        den += w;
    }
    let value = num / den;
    let provisional = n < p.min_issues_for_status;
    Aggregate {
        value: Some((value * 10.0).round() / 10.0),
        issues: n,
        provisional,
        confidence: match n {
            0..=4 => Confidence::Low,
            5..=14 => Confidence::Medium,
            _ => Confidence::High,
        },
        status: Some(GameStatus::for_value(value)),
        observed_since: sorted.first().map(|s| s.completed_at),
    }
}

/// Humorous motivational scale (§10.8). Not a professional grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GameStatus {
    Lamer,
    VibeCoder,
    Junior,
    Middle,
    Senior,
    TrueEngineer,
}

pub const GAME_STATUS_DISCLAIMER: &str = "Игровой статус отражает наблюдаемую зависимость от ИИ в рамках EST. \
     Он не является оценкой профессионального уровня разработчика.";

impl GameStatus {
    pub fn for_value(adi: f64) -> Self {
        match adi.round() as i64 {
            76.. => GameStatus::Lamer,
            56..=75 => GameStatus::VibeCoder,
            41..=55 => GameStatus::Junior,
            26..=40 => GameStatus::Middle,
            11..=25 => GameStatus::Senior,
            _ => GameStatus::TrueEngineer,
        }
    }

    pub fn label_ru(self) -> &'static str {
        match self {
            GameStatus::Lamer => "Ламер",
            GameStatus::VibeCoder => "Вайбкодер",
            GameStatus::Junior => "Джун",
            GameStatus::Middle => "Мидл",
            GameStatus::Senior => "Сениор",
            GameStatus::TrueEngineer => "Настоящий инженер",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn p() -> DecayParams {
        DecayParams::default()
    }

    #[test]
    fn weights_halve() {
        assert!((decay_weight(0.0, 0.0, &p()) - 1.0).abs() < 1e-12);
        assert!((decay_weight(30.0, 0.0, &p()) - 0.5).abs() < 1e-12);
        assert!((decay_weight(0.0, 10.0, &p()) - 0.5).abs() < 1e-12);
        assert!((decay_weight(30.0, 10.0, &p()) - 0.25).abs() < 1e-12);
        assert_eq!(
            decay_weight(-5.0, 0.0, &p()),
            1.0,
            "future timestamps do not boost"
        );
    }

    #[test]
    fn aggregate_prefers_recent() {
        let now = Utc::now();
        let old = ScoredIssue {
            score: 90.0,
            completed_at: now - Duration::days(60),
            sequence: 1,
        };
        let new = ScoredIssue {
            score: 10.0,
            completed_at: now,
            sequence: 2,
        };
        let a = aggregate(&[old, new], now, &p());
        // weights: old = 2^-2 * 2^-0.1, new = 1
        let w_old = 0.25 * 2f64.powf(-0.1);
        let expected = (90.0 * w_old + 10.0) / (w_old + 1.0);
        assert!((a.value.unwrap() - (expected * 10.0).round() / 10.0).abs() < 1e-9);
        assert!(a.provisional);
        assert_eq!(a.issues, 2);
    }

    #[test]
    fn identical_timestamps_use_sequence() {
        let now = Utc::now();
        let a = ScoredIssue {
            score: 100.0,
            completed_at: now,
            sequence: 1,
        };
        let b = ScoredIssue {
            score: 0.0,
            completed_at: now,
            sequence: 2,
        };
        // b completed after a: a is weighted by 2^(-1/10), b by 1.
        let v1 = aggregate(&[a, b], now, &p()).value.unwrap();
        let v2 = aggregate(&[b, a], now, &p()).value.unwrap();
        assert_eq!(v1, v2, "order of input does not matter");
        assert!(v1 < 50.0);
    }

    #[test]
    fn empty_and_threshold() {
        assert_eq!(aggregate(&[], Utc::now(), &p()).value, None);
        let now = Utc::now();
        let five: Vec<_> = (0..5)
            .map(|i| ScoredIssue {
                score: 20.0,
                completed_at: now,
                sequence: i,
            })
            .collect();
        let a = aggregate(&five, now, &p());
        assert!(!a.provisional);
        assert_eq!(a.status, Some(GameStatus::Senior));
    }

    #[test]
    fn statuses() {
        for (v, s) in [
            (100.0, GameStatus::Lamer),
            (76.0, GameStatus::Lamer),
            (75.0, GameStatus::VibeCoder),
            (41.0, GameStatus::Junior),
            (40.0, GameStatus::Middle),
            (11.0, GameStatus::Senior),
            (10.0, GameStatus::TrueEngineer),
            (0.0, GameStatus::TrueEngineer),
        ] {
            assert_eq!(GameStatus::for_value(v), s, "{v}");
        }
    }

    #[test]
    fn scoring_reflects_delivered_help() {
        let independent = ObservedFeatures {
            own_commits: 3,
            user_messages: 4,
            answers_by_level: [3, 1, 0, 0, 0],
            ..Default::default()
        };
        let dependent = ObservedFeatures {
            own_commits: 0,
            user_messages: 4,
            answers_by_level: [0, 0, 1, 2, 2],
            escalations_accepted: 4,
            max_level: 4,
            ..Default::default()
        };
        let a = score_issue(&independent);
        let b = score_issue(&dependent);
        assert!(a.score <= 10, "{a:?}");
        assert!(b.score >= 70, "{b:?}");
        // Escalating without using the help costs nothing.
        let escalated_only = ObservedFeatures {
            escalations_accepted: 4,
            max_level: 4,
            ..Default::default()
        };
        assert_eq!(
            score_issue(&escalated_only).score,
            score_issue(&ObservedFeatures::default()).score
        );
        // Conceptual questions weigh little.
        let curious = ObservedFeatures {
            conceptual_questions: 20,
            ..Default::default()
        };
        assert!(score_issue(&curious).score <= 20);
    }
}
