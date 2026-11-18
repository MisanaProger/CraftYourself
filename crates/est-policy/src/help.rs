//! Help levels (spec 4.2).

use est_core::HelpLevel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct HelpContext {
    pub requested: HelpLevel,
    /// Upper bound from the exercise manifest.
    pub max_allowed: HelpLevel,
    /// Own attempts recorded so far (test runs, commits, submitted hypotheses).
    pub own_attempts: u32,
    pub min_attempts_for_demonstration: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HelpGrant {
    pub requested: HelpLevel,
    pub granted: HelpLevel,
    /// Why the grant is lower than the request, if it is.
    pub limited_by: Option<String>,
}

/// Grants the highest level that is ≤ the request and allowed by the
/// exercise. Demonstration additionally requires own attempts first.
pub fn grant_help(ctx: &HelpContext) -> HelpGrant {
    let mut granted = ctx.requested;
    let mut limited_by = None;

    if granted > ctx.max_allowed {
        granted = ctx.max_allowed;
        limited_by = Some(format!(
            "this exercise allows help up to level {}",
            ctx.max_allowed.as_u8()
        ));
    }
    if granted == HelpLevel::Demonstration && ctx.own_attempts < ctx.min_attempts_for_demonstration
    {
        granted = HelpLevel::Conceptual;
        limited_by = Some(format!(
            "a demonstration unlocks after {} own attempts ({} so far)",
            ctx.min_attempts_for_demonstration, ctx.own_attempts
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
            min_attempts_for_demonstration: 2,
        }
    }

    #[test]
    fn within_limits_is_granted() {
        let g = grant_help(&ctx(HelpLevel::Diagnostic, HelpLevel::Conceptual, 0));
        assert_eq!(g.granted, HelpLevel::Diagnostic);
        assert!(g.limited_by.is_none());
    }

    #[test]
    fn capped_by_exercise() {
        let g = grant_help(&ctx(HelpLevel::Demonstration, HelpLevel::Diagnostic, 10));
        assert_eq!(g.granted, HelpLevel::Diagnostic);
        assert!(g.limited_by.is_some());
    }

    #[test]
    fn demonstration_requires_attempts() {
        let g = grant_help(&ctx(HelpLevel::Demonstration, HelpLevel::Demonstration, 1));
        assert_eq!(g.granted, HelpLevel::Conceptual);
        let g = grant_help(&ctx(HelpLevel::Demonstration, HelpLevel::Demonstration, 2));
        assert_eq!(g.granted, HelpLevel::Demonstration);
    }
}
