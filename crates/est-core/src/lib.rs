//! Shared domain vocabulary used by every EST subsystem.
//!
//! This crate is intentionally dependency-light: it holds plain types that
//! cross subsystem boundaries (exercise engine, policy engine, storage, API).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Estimated difficulty of an exercise or task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Difficulty {
    Beginner,
    Intermediate,
    Advanced,
}

/// Task categories supported by the Task Engine (spec 3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskCategory {
    BugFix,
    Refactoring,
    Testing,
    Performance,
    Documentation,
    FeatureRequest,
}

/// AI Mentor help levels (spec 4.2). Ordered from least to most help.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum HelpLevel {
    /// Level 0: the mentor does not propose solutions on its own.
    Observer = 0,
    /// Level 1: leading questions only.
    Direction = 1,
    /// Level 2: directions of investigation, possible causes.
    Diagnostic = 2,
    /// Level 3: concepts and analogous examples.
    Conceptual = 3,
    /// Level 4: example solution, only after an own attempt.
    Demonstration = 4,
}

impl HelpLevel {
    pub const MAX: HelpLevel = HelpLevel::Demonstration;

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

impl TryFrom<u8> for HelpLevel {
    type Error = CoreError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => HelpLevel::Observer,
            1 => HelpLevel::Direction,
            2 => HelpLevel::Diagnostic,
            3 => HelpLevel::Conceptual,
            4 => HelpLevel::Demonstration,
            other => return Err(CoreError::InvalidHelpLevel(other)),
        })
    }
}

impl From<HelpLevel> for u8 {
    fn from(level: HelpLevel) -> u8 {
        level.as_u8()
    }
}

/// Dotted competency identifier, e.g. `rust.ownership` or `git.commits`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Competency(String);

impl Competency {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Competency {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let valid = !s.is_empty()
            && s.split('.').all(|seg| {
                !seg.is_empty()
                    && seg.chars().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'
                    })
            });
        if valid {
            Ok(Competency(s.to_owned()))
        } else {
            Err(CoreError::InvalidCompetency(s.to_owned()))
        }
    }
}

impl TryFrom<String> for Competency {
    type Error = CoreError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<Competency> for String {
    fn from(c: Competency) -> String {
        c.0
    }
}

impl fmt::Display for Competency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Returns true for identifiers made of lowercase ASCII letters, digits and
/// single dashes (`my-exercise-1`). Used for exercise and artifact ids.
pub fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CoreError {
    #[error("help level must be in 0..=4, got {0}")]
    InvalidHelpLevel(u8),
    #[error(
        "invalid competency id `{0}`: expected dotted lowercase segments like `rust.ownership`"
    )]
    InvalidCompetency(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_levels_are_ordered() {
        assert!(HelpLevel::Observer < HelpLevel::Demonstration);
        assert_eq!(HelpLevel::try_from(3).unwrap(), HelpLevel::Conceptual);
        assert!(HelpLevel::try_from(5).is_err());
    }

    #[test]
    fn competency_validation() {
        assert!("rust.ownership".parse::<Competency>().is_ok());
        assert!("git.commit-messages".parse::<Competency>().is_ok());
        assert!("Rust.Ownership".parse::<Competency>().is_err());
        assert!("rust..x".parse::<Competency>().is_err());
        assert!("".parse::<Competency>().is_err());
    }

    #[test]
    fn slugs() {
        assert!(is_slug("fix-unicode-boundary-2"));
        assert!(!is_slug("-bad"));
        assert!(!is_slug("Bad"));
        assert!(!is_slug("a--b"));
    }
}
