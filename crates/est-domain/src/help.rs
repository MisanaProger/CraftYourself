use serde::{Deserialize, Serialize};

/// Mentor help levels (architecture v0.2, §8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum HelpLevel {
    /// 0 — no hints.
    None = 0,
    /// 1 — guiding questions.
    GuidingQuestions = 1,
    /// 2 — conceptual hints.
    ConceptualHints = 2,
    /// 3 — partial example of a solution.
    PartialExample = 3,
    /// 4 — detailed walkthrough of a solution.
    DetailedWalkthrough = 4,
}

impl HelpLevel {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Levels that reveal part of the solution and therefore require the
    /// learner's own attempts first.
    pub fn reveals_solution(self) -> bool {
        self >= HelpLevel::PartialExample
    }

    pub fn label(self) -> &'static str {
        match self {
            HelpLevel::None => "no hints",
            HelpLevel::GuidingQuestions => "guiding questions",
            HelpLevel::ConceptualHints => "conceptual hints",
            HelpLevel::PartialExample => "partial example",
            HelpLevel::DetailedWalkthrough => "detailed walkthrough",
        }
    }
}

impl TryFrom<u8> for HelpLevel {
    type Error = String;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        Ok(match v {
            0 => HelpLevel::None,
            1 => HelpLevel::GuidingQuestions,
            2 => HelpLevel::ConceptualHints,
            3 => HelpLevel::PartialExample,
            4 => HelpLevel::DetailedWalkthrough,
            other => return Err(format!("help level must be in 0..=4, got {other}")),
        })
    }
}

impl From<HelpLevel> for u8 {
    fn from(l: HelpLevel) -> u8 {
        l.as_u8()
    }
}
