//! Limits on what a mentor answer may contain (architecture v0.2, §8.3:
//! "возможность генерации кода", "объём раскрываемого решения").

use est_domain::EngagementLevel;
use serde::Serialize;

use crate::QueryKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeAllowance {
    /// No code at all.
    None,
    /// Small illustrative examples that are not about the task's code.
    Illustrative,
    /// Part of a solution to the task.
    PartialSolution,
    /// A full worked solution, explained step by step.
    FullWalkthrough,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResponseConstraints {
    pub code: CodeAllowance,
    /// Upper bound on lines of code in the answer; `None` = unlimited.
    pub max_code_lines: Option<u32>,
    /// The answer may only ask questions back (L0, Socratic).
    pub questions_only: bool,
}

impl ResponseConstraints {
    pub fn for_request(kind: &QueryKind, level: EngagementLevel) -> Self {
        use EngagementLevel::*;
        let (mut code, mut max_code_lines) = match level {
            Socratic | Hinting => (CodeAllowance::None, Some(0)),
            Explaining => (CodeAllowance::Illustrative, Some(15)),
            Demonstration => (CodeAllowance::PartialSolution, Some(30)),
            Active => (CodeAllowance::FullWalkthrough, None),
        };
        // Domain concepts may always be illustrated, independent of the task.
        if *kind == QueryKind::Conceptual && code == CodeAllowance::None {
            code = CodeAllowance::Illustrative;
            max_code_lines = Some(15);
        }
        Self {
            code,
            max_code_lines,
            questions_only: level == Socratic && *kind == QueryKind::Mentoring,
        }
    }
}

/// Counts code lines in fenced blocks of a Markdown answer. The AI Gateway
/// uses it to reject answers that exceed [`ResponseConstraints`].
pub fn fenced_code_lines(markdown: &str) -> u32 {
    let mut inside = false;
    let mut lines = 0;
    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            inside = !inside;
        } else if inside && !line.trim().is_empty() {
            lines += 1;
        }
    }
    lines
}

impl ResponseConstraints {
    /// Checks a generated answer against the constraints.
    pub fn check_answer(&self, markdown: &str) -> Result<(), String> {
        let lines = fenced_code_lines(markdown);
        match self.max_code_lines {
            Some(max) if lines > max => Err(format!(
                "answer contains {lines} lines of code, the limit at this help level is {max}"
            )),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_map_to_code_allowance() {
        let m = QueryKind::Mentoring;
        let c = ResponseConstraints::for_request(&m, EngagementLevel::Socratic);
        assert_eq!(c.code, CodeAllowance::None);
        assert!(c.questions_only);
        assert!(!ResponseConstraints::for_request(&m, EngagementLevel::Hinting).questions_only);
        assert_eq!(
            ResponseConstraints::for_request(&m, EngagementLevel::Demonstration).code,
            CodeAllowance::PartialSolution
        );
        let concept =
            ResponseConstraints::for_request(&QueryKind::Conceptual, EngagementLevel::Socratic);
        assert_eq!(concept.code, CodeAllowance::Illustrative);
        assert!(!concept.questions_only);
    }

    #[test]
    fn checks_answers() {
        let c = ResponseConstraints::for_request(&QueryKind::Mentoring, EngagementLevel::Hinting);
        assert!(c.check_answer("What happens to the last element?").is_ok());
        assert!(c.check_answer("Try:\n```rust\nlet x = 1;\n```").is_err());
        assert_eq!(fenced_code_lines("```\na\n\nb\n```\ntext\n```\nc\n```"), 3);
    }
}
