//! AI Policy Engine (architecture v0.2, §8; ТЗ §4.2–4.4).
//!
//! Decides — in code, before any model is called — what the mentor may do:
//!
//! * [`tools`] — explicit permissions for every mentor tool and the context budget;
//! * [`help`] — help levels and the rules for unlocking them;
//! * [`boundary`] — the Documentation Boundary: API-reference questions are
//!   answered with links to primary sources, never by a model;
//! * [`constraints`] — whether code may be generated and how much of the
//!   solution may be revealed.
//!
//! The AI Gateway receives a [`RouteDecision`] and must honour it; the system
//! prompt is a second line of defence, not the first one.

pub mod boundary;
pub mod constraints;
pub mod help;
pub mod tools;

pub use boundary::{BoundaryDecision, MentorRequest, QueryKind, classify};
pub use constraints::{CodeAllowance, ResponseConstraints};
pub use help::{HelpContext, HelpGrant, grant_help};
pub use tools::{Decision, PolicyConfig, PolicyEngine, Tool, ToolRequest};

use serde::Serialize;

/// The complete policy decision for one learner question.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RouteDecision {
    pub kind: QueryKind,
    pub help: HelpGrant,
    pub boundary: BoundaryDecision,
    pub constraints: ResponseConstraints,
}

impl RouteDecision {
    /// Whether the request may be passed to a model at all.
    pub fn forward_to_model(&self) -> bool {
        matches!(
            self.boundary,
            BoundaryDecision::Answer | BoundaryDecision::ExplainExcerpt { .. }
        )
    }
}

pub fn route(request: &MentorRequest, help: &HelpContext) -> RouteDecision {
    let kind = classify(&request.question);
    let boundary = boundary::decide(&kind, request);
    let help = grant_help(help);
    let constraints = ResponseConstraints::for_request(&kind, help.granted);
    RouteDecision {
        kind,
        help,
        boundary,
        constraints,
    }
}
