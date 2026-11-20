//! Help levels (architecture v0.2, §8.2).

use est_domain::HelpLevel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct HelpContext {
    pub requested: HelpLevel,
    /// Upper bound set by the task.
    pub max_allowed: HelpLevel,
    /// Own attempts observed so far (commits since the task's base revision).
    pub own_attempts: u32,
    pub min_attempts_before_solution: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HelpGrant {
    pub requested: HelpLevel,
    pub granted: HelpLevel,
    /// Why the grant is lower than the request, if it is.
    pub limited_by: Option<String>,
}

/// Grants the highest level that is ≤ the request and allowed by the task.
/// Levels that reveal the solution additionally require own attempts.
pub fn grant_help(ctx: &HelpContext) -> HelpGrant {
    let mut granted = ctx.requested;
    let mut limited_by = None;

    if granted > ctx.max_allowed {
        granted = ctx.max_allowed;
        limited_by = Some(format!(
            "this task allows help up to level {} ({})",
            ctx.max_allowed.as_u8(),
            ctx.max_allowed.label()
        ));
    }
    if granted.reveals_solution() && ctx.own_attempts < ctx.min_attempts_before_solution {
        granted = HelpLevel::ConceptualHints;
        limited_by = Some(format!(
            "solution examples unlock after {} own commits ({} so far)",
            ctx.min_attempts_before_solution, ctx.own_attempts
        ));
    }
    HelpGrant {
        requested: ctx.requested,
        granted,
        limited_by,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(requested: HelpLevel, max: HelpLevel, attempts: u32) -> HelpContext {
        HelpContext {
            requested,
            max_allowed: max,
            own_attempts: attempts,
            min_attempts_before_solution: 2,
        }
    }

    #[test]
    fn within_limits_is_granted() {
        let g = grant_help(&ctx(
            HelpLevel::ConceptualHints,
            HelpLevel::PartialExample,
            0,
        ));
        assert_eq!(g.granted, HelpLevel::ConceptualHints);
        assert!(g.limited_by.is_none());
    }

    #[test]
    fn capped_by_task() {
        let g = grant_help(&ctx(
            HelpLevel::DetailedWalkthrough,
            HelpLevel::GuidingQuestions,
            10,
        ));
        assert_eq!(g.granted, HelpLevel::GuidingQuestions);
        assert!(g.limited_by.is_some());
    }

    #[test]
    fn solution_levels_require_attempts() {
        for level in [HelpLevel::PartialExample, HelpLevel::DetailedWalkthrough] {
            let g = grant_help(&ctx(level, HelpLevel::DetailedWalkthrough, 1));
            assert_eq!(g.granted, HelpLevel::ConceptualHints);
            let g = grant_help(&ctx(level, HelpLevel::DetailedWalkthrough, 2));
            assert_eq!(g.granted, level);
        }
    }
}
