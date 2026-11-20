//! AI Policy Engine (architecture v0.3, §7–8).
//!
//! Decides — in code, before any model is called — what the mentor may do
//! with a message:
//!
//! * [`boundary`] — Documentation Boundary: API-reference questions are
//!   answered with links to primary sources, never by a model;
//! * [`engagement`] — the engagement level a request needs; a request above
//!   the current level is blocked and a one-step escalation is proposed;
//! * [`constraints`] — whether code may be generated and how much of the
//!   solution may be revealed at the current level;
//! * [`tools`] — explicit permissions for every mentor tool.
//!
//! The model can neither raise its own level nor assume consent.

pub mod boundary;
pub mod constraints;
pub mod engagement;
pub mod tools;

pub use boundary::{BoundaryDecision, MentorRequest, QueryKind, classify};
pub use constraints::{CodeAllowance, ResponseConstraints};
pub use engagement::required_level;
pub use tools::{Decision, PolicyConfig, PolicyEngine, Tool, ToolRequest};

use est_domain::{EngagementLevel, EngagementState};
use serde::Serialize;

/// What to do with one learner message.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Outcome {
    /// Answer with the model at the current level.
    Answer,
    /// Reply with primary sources; no model involved.
    Redirect { symbols: Vec<String> },
    /// The request needs a higher level: block it and propose one step.
    ProposeEscalation { to: EngagementLevel },
    /// Refuse (e.g. oversized documentation paste).
    Refuse { reason: String },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RouteDecision {
    pub kind: QueryKind,
    /// Level the request needs.
    pub required: EngagementLevel,
    /// Level the answer is produced at (the current level).
    pub level: EngagementLevel,
    pub outcome: Outcome,
    pub constraints: ResponseConstraints,
    /// The request needed more than the task allows; answered within limits.
    pub capped: bool,
}

/// Routes a message. With `allow_escalation = false` (the learner declined
/// the proposal) the request is answered within the current level.
pub fn route(
    request: &MentorRequest,
    state: &EngagementState,
    allow_escalation: bool,
) -> RouteDecision {
    let kind = classify(&request.question);
    let level = state.level;
    let constraints = ResponseConstraints::for_request(&kind, level);
    let decision = |required, outcome, capped| RouteDecision {
        kind: kind.clone(),
        required,
        level,
        outcome,
        constraints: constraints.clone(),
        capped,
    };

    match boundary::decide(&kind, request) {
        BoundaryDecision::Redirect { symbols } => {
            return decision(level, Outcome::Redirect { symbols }, false);
        }
        BoundaryDecision::Refuse { reason } => {
            return decision(level, Outcome::Refuse { reason }, false);
        }
        BoundaryDecision::ExplainExcerpt { .. } | BoundaryDecision::Answer => {}
    }

    // Domain concepts can be explained at any level (§8.3); only requests
    // about the task's solution are bound by the engagement level.
    let required = match kind {
        QueryKind::Mentoring => required_level(&request.question),
        _ => level,
    };
    if required <= level {
        return decision(required, Outcome::Answer, false);
    }
    match state.escalation_target(required) {
        Some(to) if allow_escalation => {
            decision(required, Outcome::ProposeEscalation { to }, false)
        }
        Some(_) => decision(required, Outcome::Answer, false),
        None => decision(required, Outcome::Answer, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use EngagementLevel::*;
    use est_core::ConversationId;

    fn state(level: EngagementLevel, max: EngagementLevel) -> EngagementState {
        EngagementState {
            level,
            ..EngagementState::initial(ConversationId::from("c"), max)
        }
    }

    fn ask(q: &str) -> MentorRequest {
        MentorRequest {
            question: q.into(),
            excerpt: None,
        }
    }

    #[test]
    fn answer_within_level() {
        let d = route(&ask("Подскажи, где искать?"), &state(Hinting, Active), true);
        assert_eq!(d.outcome, Outcome::Answer);
        assert_eq!(d.level, Hinting);
    }

    #[test]
    fn above_level_proposes_one_step() {
        let d = route(&ask("Дай полное решение"), &state(Socratic, Active), true);
        assert_eq!(d.required, Active);
        assert_eq!(d.outcome, Outcome::ProposeEscalation { to: Hinting });
        let d = route(
            &ask("Дай полное решение"),
            &state(Demonstration, Active),
            true,
        );
        assert_eq!(d.outcome, Outcome::ProposeEscalation { to: Active });
    }

    #[test]
    fn declined_or_capped_answers_within_level() {
        let d = route(
            &ask("Покажи пример исправления"),
            &state(Hinting, Active),
            false,
        );
        assert_eq!((d.outcome.clone(), d.level), (Outcome::Answer, Hinting));
        assert_eq!(d.constraints.code, CodeAllowance::None);
        let d = route(
            &ask("Покажи пример исправления"),
            &state(Explaining, Explaining),
            true,
        );
        assert_eq!(d.outcome, Outcome::Answer);
        assert!(d.capped);
    }

    #[test]
    fn concepts_and_docs_never_escalate() {
        let d = route(&ask("Что такое итератор?"), &state(Socratic, Active), true);
        assert_eq!(d.outcome, Outcome::Answer);
        assert_eq!(d.constraints.code, CodeAllowance::Illustrative);
        let d = route(
            &ask("Что возвращает `str::find`?"),
            &state(Socratic, Active),
            true,
        );
        assert_eq!(
            d.outcome,
            Outcome::Redirect {
                symbols: vec!["str::find".into()]
            }
        );
    }
}
