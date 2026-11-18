//! AI Policy Engine (spec 4.2–4.4).
//!
//! Everything the AI Mentor may do is decided here, in ordinary code, before
//! a model is ever called. The system prompt is a second line of defence, not
//! the first one:
//!
//! * [`tools`] — explicit permissions for every tool the mentor can use
//!   (file access, repository search, docs, terminal, writes, commands) and
//!   limits on how much context is sent to a model.
//! * [`help`] — the five help levels and the rules for unlocking them.
//! * [`boundary`] — the Documentation Boundary: API-reference questions are
//!   answered with links to primary sources instead of a model's paraphrase.
//! * [`docs`] — the Documentation Router that builds those precise links.

pub mod boundary;
pub mod docs;
pub mod help;
pub mod tools;

pub use boundary::{classify, BoundaryDecision, MentorRequest, QueryKind};
pub use docs::{DocLink, DocLinkKind, DocRouter, DocSource};
pub use help::{grant_help, HelpContext, HelpGrant};
pub use tools::{Decision, PolicyConfig, PolicyEngine, Tool, ToolRequest};

use serde::Serialize;

/// The complete routing decision for one learner question.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RouteDecision {
    pub kind: QueryKind,
    pub help: HelpGrant,
    pub boundary: BoundaryDecision,
}

/// Classifies the request, applies the documentation boundary and grants a
/// help level.
pub fn route(request: &MentorRequest, help: &HelpContext) -> RouteDecision {
    let kind = classify(&request.question);
    let boundary = boundary::decide(&kind, request);
    let help = grant_help(help);
    RouteDecision {
        kind,
        help,
        boundary,
    }
}
