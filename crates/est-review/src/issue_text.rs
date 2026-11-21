//! A checklist for an Issue the learner writes (Issue engineering).
//!
//! A bug report needs a specific title, steps to reproduce, the expected
//! and the actual behaviour, the environment and ideally a minimal
//! example; a feature request needs the motivation, a proposal and the
//! alternatives considered. Markers are looked for in English, Russian and
//! Uzbek. The checklist points at what is missing; it does not write it.

use serde::{Deserialize, Serialize};

use crate::{Finding, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    Bug,
    Feature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    pub code: String,
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueTextReview {
    pub kind: IssueKind,
    pub checks: Vec<Check>,
    /// Share of satisfied checks, 0–100.
    pub score: u8,
    /// Problems that prevent publishing (empty title, no description).
    pub blockers: Vec<Finding>,
}

const VAGUE_TITLES: &[&str] = &[
    "bug",
    "error",
    "problem",
    "issue",
    "help",
    "doesn't work",
    "does not work",
    "not working",
    "broken",
    "crash",
    "question",
    "feature",
    "request",
    "ошибка",
    "баг",
    "не работает",
    "проблема",
    "помогите",
    "xato",
    "ishlamayapti",
];

fn has_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|m| text.contains(m))
}

/// A numbered list (`1.`) or a bulleted list of at least two items.
fn has_steps(body: &str) -> bool {
    let numbered = body
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            let digits = t.chars().take_while(char::is_ascii_digit).count();
            digits > 0 && matches!(t[digits..].chars().next(), Some('.' | ')'))
        })
        .count();
    numbered >= 2
}

fn has_version(text: &str) -> bool {
    // `1.2`, `v3.10.1`, `rustc 1.85` …
    text.split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .any(|w| {
            let parts: Vec<&str> = w.split('.').collect();
            parts.len() >= 2 && parts.iter().all(|p| !p.is_empty())
        })
}

pub fn review(kind: IssueKind, title: &str, body: &str) -> IssueTextReview {
    let title = title.trim();
    let lower_title = title.to_lowercase();
    let text = body.to_lowercase();
    let mut blockers = Vec::new();
    if title.is_empty() {
        blockers.push(Finding::new("title_empty", Severity::Problem, None));
    }
    if body.trim().chars().count() < 30 {
        blockers.push(Finding::new("description_missing", Severity::Problem, None));
    }
    let words = title.split_whitespace().count();
    let specific =
        words >= 4 && !VAGUE_TITLES.contains(&lower_title.trim_end_matches(['.', '!', '?']));
    let code_block = body.contains("```") || body.lines().any(|l| l.starts_with("    "));
    let mut checks = vec![("title_specific", specific)];
    match kind {
        IssueKind::Bug => {
            checks.push((
                "steps",
                has_steps(body)
                    || has_any(
                        &text,
                        &[
                            "steps to reproduce",
                            "to reproduce",
                            "reproduce",
                            "воспроизв",
                            "шаги",
                            "takrorlash",
                            "qadamlar",
                        ],
                    ),
            ));
            checks.push((
                "expected",
                has_any(
                    &text,
                    &["expected", "should", "ожида", "должн", "kutilgan", "kerak"],
                ),
            ));
            checks.push((
                "actual",
                has_any(
                    &text,
                    &[
                        "actual",
                        "instead",
                        "but ",
                        "panic",
                        "error:",
                        "вместо",
                        "на деле",
                        "фактически",
                        "получаю",
                        "haqiqiy",
                        "o'rniga",
                    ],
                ),
            ));
            checks.push((
                "environment",
                has_version(body)
                    || has_any(
                        &text,
                        &[
                            "version",
                            "os:",
                            "linux",
                            "windows",
                            "macos",
                            "версия",
                            "versiya",
                        ],
                    ),
            ));
            checks.push(("example", code_block));
        }
        IssueKind::Feature => {
            checks.push((
                "motivation",
                has_any(
                    &text,
                    &[
                        "because",
                        "motivation",
                        "use case",
                        "why",
                        "need",
                        "потому",
                        "зачем",
                        "чтобы",
                        "нужно",
                        "chunki",
                        "kerak",
                    ],
                ),
            ));
            checks.push((
                "proposal",
                has_any(
                    &text,
                    &[
                        "propose",
                        "proposal",
                        "suggest",
                        "could",
                        "would",
                        "add ",
                        "предлага",
                        "можно",
                        "добавить",
                        "taklif",
                    ],
                ),
            ));
            checks.push((
                "alternatives",
                has_any(
                    &text,
                    &[
                        "alternative",
                        "workaround",
                        "instead",
                        "альтернатив",
                        "обход",
                        "вместо",
                        "muqobil",
                    ],
                ),
            ));
        }
    }
    let ok = checks.iter().filter(|(_, ok)| *ok).count();
    IssueTextReview {
        kind,
        score: ((ok * 100 + checks.len() / 2) / checks.len()) as u8,
        checks: checks
            .into_iter()
            .map(|(code, ok)| Check {
                code: code.into(),
                ok,
            })
            .collect(),
        blockers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(r: &IssueTextReview) -> Vec<&str> {
        r.checks
            .iter()
            .filter(|c| c.ok)
            .map(|c| c.code.as_str())
            .collect()
    }

    #[test]
    fn a_complete_bug_report() {
        let body = "Steps:\n1. Call `parse_range(\"5..\")`\n2. Run it\n\nExpected: an error.\nActual: it panics.\n\nrustc 1.85 on Linux\n\n```rust\nparse_range(\"5..\");\n```";
        let r = review(IssueKind::Bug, "parse_range panics on an open range", body);
        assert_eq!(r.score, 100, "{:?}", r.checks);
        assert!(r.blockers.is_empty());
    }

    #[test]
    fn a_thin_bug_report() {
        let r = review(
            IssueKind::Bug,
            "Bug",
            "It does not work for me at all, please fix",
        );
        assert_eq!(ok(&r), Vec::<&str>::new());
        assert_eq!(r.score, 0);
        let r = review(IssueKind::Bug, "", "short");
        let codes: Vec<_> = r.blockers.iter().map(|b| b.code.as_str()).collect();
        assert_eq!(codes, ["title_empty", "description_missing"]);
    }

    #[test]
    fn feature_requests_in_russian() {
        let r = review(
            IssueKind::Feature,
            "Поддержать открытые диапазоны в parse_range",
            "Нужно, чтобы `5..` работал: так пишут в конфигурациях. Предлагаю вернуть итератор без конца. Альтернатива — ошибка с подсказкой.",
        );
        assert_eq!(r.score, 100, "{:?}", r.checks);
    }
}
