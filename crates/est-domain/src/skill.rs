use serde::{Deserialize, Serialize};

/// Measurable areas of engineering competence (architecture v0.2, §5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Skill {
    CodeReading,
    Debugging,
    Testing,
    ProblemSolving,
    DocumentationLiteracy,
    GitWorkflow,
    EngineeringCommunication,
    IndependentWork,
}

impl Skill {
    pub const ALL: [Skill; 8] = [
        Skill::CodeReading,
        Skill::Debugging,
        Skill::Testing,
        Skill::ProblemSolving,
        Skill::DocumentationLiteracy,
        Skill::GitWorkflow,
        Skill::EngineeringCommunication,
        Skill::IndependentWork,
    ];
}
