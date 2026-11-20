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
    /// Only code copied from the text being translated.
    PreserveSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResponseConstraints {
    pub code: CodeAllowance,
    /// Upper bound on lines of code in the answer; `None` = unlimited.
    pub max_code_lines: Option<u32>,
    /// The answer may only ask questions back (L0, Socratic).
    pub questions_only: bool,
    /// How many interactive HTML5 pages (```html blocks) the answer may
    /// carry outside the code budget. With 0, pages are not allowed at all.
    pub max_pages: u32,
}

/// Upper bound on the size of one HTML page in an answer.
pub const MAX_PAGE_LINES: u32 = 400;
pub const MAX_PAGE_BYTES: usize = 64 * 1024;

impl ResponseConstraints {
    pub fn for_request(kind: &QueryKind, level: EngagementLevel) -> Self {
        use EngagementLevel::*;
        let (mut code, mut max_code_lines) = match level {
            Socratic | Hinting => (CodeAllowance::None, Some(0)),
            Explaining => (CodeAllowance::Illustrative, Some(15)),
            Demonstration => (CodeAllowance::PartialSolution, Some(30)),
            Active => (CodeAllowance::FullWalkthrough, None),
        };
        match kind {
            QueryKind::AcceptanceCriteria => {
                return Self {
                    code: CodeAllowance::None,
                    max_code_lines: Some(0),
                    questions_only: false,
                    max_pages: 0,
                };
            }
            QueryKind::Translation => {
                // The real limit (code lines in the source) is set by `route`.
                return Self {
                    code: CodeAllowance::PreserveSource,
                    max_code_lines: Some(0),
                    questions_only: false,
                    max_pages: 0,
                };
            }
            _ => {}
        }
        // Domain concepts may always be illustrated, independent of the task.
        if *kind == QueryKind::Conceptual && code == CodeAllowance::None {
            code = CodeAllowance::Illustrative;
            max_code_lines = Some(15);
        }
        // A page is a whole program: one is allowed from L2, where
        // explanations begin, for any kind of question. Below L2 there are
        // no pages, not even for domain concepts or as "illustrative" code.
        let max_pages = match level {
            Socratic | Hinting => 0,
            Explaining | Demonstration | Active => 1,
        };
        Self {
            code,
            max_code_lines,
            questions_only: level == Socratic && *kind == QueryKind::Mentoring,
            max_pages,
        }
    }
}

/// A fenced block of a Markdown answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FencedBlock {
    /// First word after the opening fence, lowercased.
    pub lang: String,
    pub body: String,
}

impl FencedBlock {
    /// An HTML5 page the interface may render in an isolated frame.
    pub fn is_page(&self) -> bool {
        self.lang == "html"
    }

    fn code_lines(&self) -> u32 {
        self.body.lines().filter(|l| !l.trim().is_empty()).count() as u32
    }
}

/// Fenced blocks in order. An unclosed block runs to the end.
pub fn fenced_blocks(markdown: &str) -> Vec<FencedBlock> {
    let mut out = Vec::new();
    let mut current: Option<FencedBlock> = None;
    for line in markdown.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("```") {
            match current.take() {
                Some(block) => out.push(block),
                None => {
                    current = Some(FencedBlock {
                        lang: rest
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .to_ascii_lowercase(),
                        body: String::new(),
                    })
                }
            }
        } else if let Some(block) = current.as_mut() {
            block.body.push_str(line);
            block.body.push('\n');
        }
    }
    out.extend(current);
    out
}

/// Removes ```html blocks, keeping the rest of the answer.
pub fn strip_pages(markdown: &str) -> String {
    let mut out = String::new();
    let mut inside: Option<bool> = None;
    for line in markdown.lines() {
        let fence = line.trim_start().strip_prefix("```");
        let keep = match (fence, inside) {
            (Some(_), Some(page)) => {
                inside = None;
                !page
            }
            (Some(rest), None) => {
                let page = rest
                    .split_whitespace()
                    .next()
                    .is_some_and(|l| l.eq_ignore_ascii_case("html"));
                inside = Some(page);
                !page
            }
            (None, state) => state != Some(true),
        };
        if keep {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim_end().to_owned()
}

/// HTML pages of an answer, in order (the interface addresses them by index).
pub fn html_pages(markdown: &str) -> Vec<String> {
    fenced_blocks(markdown)
        .into_iter()
        .filter(FencedBlock::is_page)
        .map(|b| b.body)
        .collect()
}

/// Counts code lines in fenced blocks of a Markdown answer. The AI Gateway
/// uses it to reject answers that exceed [`ResponseConstraints`].
pub fn fenced_code_lines(markdown: &str) -> u32 {
    fenced_blocks(markdown)
        .iter()
        .map(FencedBlock::code_lines)
        .sum()
}

impl ResponseConstraints {
    /// Checks a generated answer against the constraints.
    pub fn check_answer(&self, markdown: &str) -> Result<(), String> {
        let blocks = fenced_blocks(markdown);
        let (pages, code): (Vec<_>, Vec<_>) = blocks.iter().partition(|b| b.is_page());
        if self.max_pages == 0 && !pages.is_empty() {
            return Err("HTML pages are allowed from level L2".into());
        }
        if pages.len() as u32 > self.max_pages {
            return Err(format!(
                "answer contains {} HTML pages, the limit is {}",
                pages.len(),
                self.max_pages
            ));
        }
        if let Some(p) = pages
            .iter()
            .find(|p| p.code_lines() > MAX_PAGE_LINES || p.body.len() > MAX_PAGE_BYTES)
        {
            return Err(format!(
                "an HTML page has {} lines, the limit is {MAX_PAGE_LINES}",
                p.code_lines()
            ));
        }
        let lines: u32 = code.iter().map(|b| b.code_lines()).sum();
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

    #[test]
    fn html_pages_have_their_own_budget() {
        let page = "See:\n```html\n<!doctype html>\n<p>x</p>\n```\nand\n```rust\nlet a = 1;\n```";
        assert_eq!(html_pages(page), ["<!doctype html>\n<p>x</p>\n"]);
        // Below L2 there are no pages for any kind of question, not even as
        // a short block within the illustrative code budget.
        for kind in [QueryKind::Mentoring, QueryKind::Conceptual] {
            for level in [EngagementLevel::Socratic, EngagementLevel::Hinting] {
                let c = ResponseConstraints::for_request(&kind, level);
                assert_eq!(c.max_pages, 0);
                assert!(c.check_answer("```html\n<p>x</p>\n```").is_err());
            }
        }
        let concept =
            ResponseConstraints::for_request(&QueryKind::Conceptual, EngagementLevel::Socratic);
        assert!(concept.check_answer("```rust\nlet a = 1;\n```").is_ok());
        assert_eq!(strip_pages(page), "See:\nand\n```rust\nlet a = 1;\n```");
        // From L2 one page is allowed outside the code budget.
        let concept =
            ResponseConstraints::for_request(&QueryKind::Conceptual, EngagementLevel::Explaining);
        assert!(concept.check_answer(page).is_ok());
        let long = format!(
            "```html\n{}```",
            "<p>x</p>\n".repeat(MAX_PAGE_LINES as usize + 1)
        );
        assert!(concept.check_answer(&long).is_err());
        let two = "```html\n<p>a</p>\n```\n```html\n<p>b</p>\n```";
        assert!(concept.check_answer(two).is_err());
        let l2 =
            ResponseConstraints::for_request(&QueryKind::Mentoring, EngagementLevel::Explaining);
        assert_eq!(l2.max_pages, 1);
        // Translations and acceptance criteria never carry pages.
        for k in [QueryKind::Translation, QueryKind::AcceptanceCriteria] {
            assert_eq!(
                ResponseConstraints::for_request(&k, EngagementLevel::Active).max_pages,
                0
            );
        }
    }
}
