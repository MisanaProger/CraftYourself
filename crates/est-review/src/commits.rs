//! Commit review.
//!
//! General rules (a clear subject, a body for big changes, a blank line
//! between them, no generated files or secrets, reasonably small commits)
//! plus the conventions the repository itself follows, learned from its
//! history before the learner's work: Conventional Commits prefixes,
//! capitalisation, a trailing period, a 50-character subject limit,
//! `Signed-off-by` trailers. A convention counts only when most of the
//! history follows it, so a learner is never asked for something the
//! project does not do.

use serde::{Deserialize, Serialize};

use crate::{Finding, Severity};

/// Below this many commits the history teaches nothing.
pub const MIN_HISTORY: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    /// `None` for binary files.
    pub added: Option<u32>,
    pub deleted: Option<u32>,
}

/// One commit as Git reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitData {
    pub hash: String,
    /// The whole message, as written.
    pub message: String,
    pub files: Vec<FileChange>,
}

impl CommitData {
    pub fn subject(&self) -> &str {
        self.message.lines().next().unwrap_or("").trim()
    }

    /// Lines after the subject, without the separating blank line.
    pub fn body(&self) -> String {
        let mut lines = self
            .message
            .lines()
            .skip(1)
            .skip_while(|l| l.trim().is_empty());
        let mut body = String::new();
        for l in lines.by_ref() {
            body.push_str(l);
            body.push('\n');
        }
        body.trim().to_owned()
    }

    pub fn lines_changed(&self) -> u32 {
        self.files
            .iter()
            .map(|f| f.added.unwrap_or(0) + f.deleted.unwrap_or(0))
            .sum()
    }
}

/// What the repository's history does consistently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conventions {
    /// Commits the conventions were learned from.
    pub learned_from: u32,
    /// `type(scope): subject`.
    pub conventional: bool,
    /// `Some(true)`: subjects start with a capital letter; `Some(false)`:
    /// lowercase; `None`: no rule.
    pub capitalized: Option<bool>,
    /// Subjects end with a period.
    pub period: bool,
    /// Longest subject the project's history keeps to (50 or 72).
    pub subject_limit: usize,
    /// `Signed-off-by:` trailers (DCO).
    pub signed_off: bool,
}

impl Default for Conventions {
    fn default() -> Self {
        Self {
            learned_from: 0,
            conventional: false,
            capitalized: None,
            period: false,
            subject_limit: 72,
            signed_off: false,
        }
    }
}

/// `feat: …`, `fix(parser)!: …`.
pub fn conventional_prefix(subject: &str) -> Option<&str> {
    let (head, rest) = subject.split_once(": ")?;
    if rest.trim().is_empty() {
        return None;
    }
    let head = head.strip_suffix('!').unwrap_or(head);
    let kind = match head.split_once('(') {
        Some((kind, scope)) => {
            let scope = scope.strip_suffix(')')?;
            if scope.is_empty() || scope.contains(['(', ')', ' ']) {
                return None;
            }
            kind
        }
        None => head,
    };
    (!kind.is_empty() && kind.len() <= 12 && kind.chars().all(|c| c.is_ascii_lowercase()))
        .then_some(rest)
}

/// The subject without a Conventional Commits prefix.
fn subject_text(subject: &str) -> &str {
    conventional_prefix(subject).unwrap_or(subject)
}

fn share(n: usize, of: usize) -> f64 {
    if of == 0 { 0.0 } else { n as f64 / of as f64 }
}

/// Learns the conventions from the history before the learner's work.
/// Merge commits are ignored.
pub fn learn(history: &[CommitData]) -> Conventions {
    let commits: Vec<&CommitData> = history
        .iter()
        .filter(|c| !c.subject().starts_with("Merge ") && !c.subject().is_empty())
        .collect();
    let n = commits.len();
    if n < MIN_HISTORY {
        return Conventions {
            learned_from: n as u32,
            ..Default::default()
        };
    }
    let count = |f: &dyn Fn(&CommitData) -> bool| commits.iter().filter(|c| f(c)).count();
    let conventional = share(count(&|c| conventional_prefix(c.subject()).is_some()), n) >= 0.6;
    let first = |c: &CommitData| subject_text(c.subject()).chars().next();
    let lettered = count(&|c| first(c).is_some_and(char::is_alphabetic));
    let upper = count(&|c| first(c).is_some_and(char::is_uppercase));
    let lower = count(&|c| first(c).is_some_and(char::is_lowercase));
    let capitalized = if share(upper, lettered) >= 0.8 {
        Some(true)
    } else if share(lower, lettered) >= 0.8 {
        Some(false)
    } else {
        None
    };
    let mut lengths: Vec<usize> = commits
        .iter()
        .map(|c| c.subject().chars().count())
        .collect();
    lengths.sort_unstable();
    let p90 = lengths[(lengths.len() * 9 / 10).min(lengths.len() - 1)];
    Conventions {
        learned_from: n as u32,
        conventional,
        capitalized,
        period: share(count(&|c| c.subject().ends_with('.')), n) >= 0.6,
        subject_limit: if p90 <= 50 { 50 } else { 72 },
        signed_off: share(count(&|c| c.message.contains("Signed-off-by:")), n) >= 0.6,
    }
}

const VAGUE: &[&str] = &[
    "wip",
    "fix",
    "fixes",
    "fixed",
    "update",
    "updates",
    "updated",
    "change",
    "changes",
    "misc",
    "stuff",
    "tmp",
    "temp",
    "test",
    "tests",
    "commit",
    "work",
    "done",
    "minor",
    "cleanup",
    "refactor",
    "save",
    "first commit",
    "initial",
    "asdf",
    "правки",
    "фикс",
    "исправления",
    "изменения",
    "обновление",
    "tuzatish",
    "o'zgarishlar",
];

const NOT_IMPERATIVE: &[&str] = &[
    "added",
    "adds",
    "adding",
    "fixed",
    "fixes",
    "fixing",
    "updated",
    "updates",
    "updating",
    "removed",
    "removes",
    "removing",
    "changed",
    "changes",
    "changing",
    "implemented",
    "implements",
    "implementing",
    "refactored",
    "refactors",
    "refactoring",
    "created",
    "creates",
    "creating",
    "moved",
    "moves",
    "renamed",
    "renames",
    "improved",
    "improves",
    "deleted",
    "deletes",
    "made",
    "makes",
];

fn is_unwanted(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    let name = p.rsplit('/').next().unwrap_or(&p);
    p.split('/').any(|seg| {
        matches!(
            seg,
            "target"
                | "node_modules"
                | "__pycache__"
                | ".idea"
                | ".vscode"
                | ".pytest_cache"
                | ".mypy_cache"
        )
    }) || matches!(name, ".ds_store" | "thumbs.db")
        || name.ends_with(".pyc")
        || name.ends_with(".log")
        || name.ends_with(".o")
        || name.ends_with(".class")
        || name.ends_with(".swp")
}

fn is_secret(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    name == ".env"
        || (name.starts_with(".env.") && !name.ends_with(".example") && !name.ends_with(".sample"))
        || name.ends_with(".pem")
        || name.ends_with(".key")
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || name == ".netrc"
        || name.starts_with("credentials")
}

/// The review of one commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitReview {
    pub hash: String,
    pub subject: String,
    pub lines_changed: u32,
    pub files: u32,
    pub score: u8,
    pub findings: Vec<Finding>,
}

/// Checks one commit; each finding lowers the score by a fixed amount.
pub fn review(c: &CommitData, conv: &Conventions) -> CommitReview {
    let mut f: Vec<(Finding, u32)> = Vec::new();
    let mut add = |code: &str, sev: Severity, points: u32, detail: Option<String>| {
        f.push((Finding::new(code, sev, detail), points))
    };
    use Severity::*;
    let subject = c.subject();
    let text = subject_text(subject);
    let len = subject.chars().count();
    let lower = text.trim().to_lowercase();
    let lower = lower.trim_end_matches('.');
    if subject.is_empty() {
        add("subject_empty", Problem, 60, None);
    } else if subject.starts_with("fixup!") || subject.starts_with("squash!") {
        add("fixup", Warning, 15, None);
    } else if subject.starts_with("Merge ") {
        add("merge", Info, 0, None);
    } else {
        if VAGUE.contains(&lower) || lower.chars().count() < 8 {
            add("vague_subject", Warning, 20, None);
        }
        let first = lower.split_whitespace().next().unwrap_or("");
        if NOT_IMPERATIVE.contains(&first) {
            add("not_imperative", Warning, 10, Some(first.to_owned()));
        }
        if len > 72 {
            add("subject_too_long", Warning, 15, Some(len.to_string()));
        } else if len > conv.subject_limit {
            add("subject_over_limit", Info, 5, Some(len.to_string()));
        }
        if subject.ends_with('.') && !conv.period {
            add("subject_period", Info, 5, None);
        }
        if conv.conventional && conventional_prefix(subject).is_none() {
            add("convention_prefix", Warning, 10, None);
        }
        let first_char = text.chars().next();
        match conv.capitalized {
            Some(true) if first_char.is_some_and(char::is_lowercase) => {
                add("convention_capital", Info, 5, None)
            }
            Some(false) if first_char.is_some_and(char::is_uppercase) => {
                add("convention_lowercase", Info, 5, None)
            }
            _ => {}
        }
    }
    if c.message
        .lines()
        .nth(1)
        .is_some_and(|l| !l.trim().is_empty())
    {
        add("no_blank_line", Warning, 10, None);
    }
    let lines = c.lines_changed();
    let body = c.body();
    if lines > 100 && body.is_empty() {
        add("large_without_body", Warning, 15, Some(lines.to_string()));
    }
    if lines > 500 || c.files.len() > 20 {
        add(
            "too_large",
            Warning,
            15,
            Some(format!("{lines}/{}", c.files.len())),
        );
    }
    let tops: std::collections::BTreeSet<&str> = c
        .files
        .iter()
        .filter_map(|x| x.path.split_once('/').map(|(top, _)| top))
        .filter(|t| !matches!(*t, "tests" | "test" | "docs" | "doc"))
        .collect();
    if tops.len() > 3 {
        add("mixed_changes", Info, 5, Some(tops.len().to_string()));
    }
    if let Some(p) = c.files.iter().find(|x| is_secret(&x.path)) {
        add("secret_file", Problem, 40, Some(p.path.clone()));
    }
    if let Some(p) = c.files.iter().find(|x| is_unwanted(&x.path)) {
        add("generated_file", Warning, 15, Some(p.path.clone()));
    }
    if conv.signed_off && !c.message.contains("Signed-off-by:") {
        add("convention_signoff", Warning, 10, None);
    }
    let penalty: u32 = f.iter().map(|(_, p)| p).sum();
    let mut findings: Vec<Finding> = f.into_iter().map(|(x, _)| x).collect();
    findings.sort_by_key(|x| std::cmp::Reverse(x.severity));
    CommitReview {
        hash: c.hash.clone(),
        subject: subject.to_owned(),
        lines_changed: lines,
        files: c.files.len() as u32,
        score: 100u32.saturating_sub(penalty) as u8,
        findings,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    pub conventions: Conventions,
    /// Oldest first.
    pub commits: Vec<CommitReview>,
    /// Mean of the commits' scores (merges excluded); `None` without commits.
    pub score: Option<u8>,
}

/// Reviews the learner's commits (oldest first).
pub fn review_all(commits: &[CommitData], conv: &Conventions) -> Review {
    let reviews: Vec<CommitReview> = commits.iter().map(|c| review(c, conv)).collect();
    let scored: Vec<u32> = reviews
        .iter()
        .filter(|r| !r.findings.iter().any(|f| f.code == "merge"))
        .map(|r| u32::from(r.score))
        .collect();
    let score = (!scored.is_empty()).then(|| {
        ((scored.iter().sum::<u32>() + scored.len() as u32 / 2) / scored.len() as u32) as u8
    });
    Review {
        conventions: conv.clone(),
        commits: reviews,
        score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(message: &str, files: &[(&str, u32)]) -> CommitData {
        CommitData {
            hash: "abc".into(),
            message: message.into(),
            files: files
                .iter()
                .map(|(p, n)| FileChange {
                    path: (*p).into(),
                    added: Some(*n),
                    deleted: Some(0),
                })
                .collect(),
        }
    }

    fn codes(r: &CommitReview) -> Vec<&str> {
        r.findings.iter().map(|f| f.code.as_str()).collect()
    }

    #[test]
    fn a_good_commit_scores_full() {
        let r = review(
            &commit(
                "Return an error for an open range\n\n`5..` used to panic; callers expect Err.",
                &[("src/lib.rs", 12), ("tests/range.rs", 20)],
            ),
            &Conventions::default(),
        );
        assert_eq!((r.score, codes(&r)), (100, vec![]));
    }

    #[test]
    fn finds_common_problems() {
        let r = review(
            &commit("fixed stuff", &[("src/a.rs", 300), ("target/debug/x", 1)]),
            &Conventions::default(),
        );
        let c = codes(&r);
        for code in ["not_imperative", "large_without_body", "generated_file"] {
            assert!(c.contains(&code), "{code} in {c:?}");
        }
        assert!(r.score < 70, "{}", r.score);
        let r = review(
            &commit("Fix the parser\nno blank line", &[]),
            &Conventions::default(),
        );
        assert!(codes(&r).contains(&"no_blank_line"));
        let r = review(&commit("wip", &[(".env", 1)]), &Conventions::default());
        assert_eq!(r.findings[0].code, "secret_file");
        assert!(codes(&r).contains(&"vague_subject"));
        let long = format!("Add {}", "x".repeat(80));
        assert!(
            codes(&review(&commit(&long, &[]), &Conventions::default()))
                .contains(&"subject_too_long")
        );
    }

    #[test]
    fn learns_conventions_from_history() {
        let mut history: Vec<CommitData> = (0..12)
            .map(|i| {
                commit(
                    &format!("feat(parser): support case {i}\n\nSigned-off-by: A <a@x>"),
                    &[],
                )
            })
            .collect();
        history.push(commit("Merge branch 'x'", &[]));
        let conv = learn(&history);
        assert!(conv.conventional && conv.signed_off);
        assert_eq!(conv.capitalized, Some(false));
        assert_eq!(conv.subject_limit, 50);
        let r = review(&commit("Return an error for an open range", &[]), &conv);
        let c = codes(&r);
        assert!(
            c.contains(&"convention_prefix") && c.contains(&"convention_signoff"),
            "{c:?}"
        );
        assert!(c.contains(&"convention_lowercase"));
        // Too little history: no conventions.
        assert_eq!(
            learn(&history[..5]),
            Conventions {
                learned_from: 5,
                ..Default::default()
            }
        );
    }

    #[test]
    fn prefixes() {
        assert_eq!(conventional_prefix("fix(parser)!: x"), Some("x"));
        assert_eq!(conventional_prefix("docs: readme"), Some("readme"));
        assert_eq!(conventional_prefix("Note: something"), None);
        assert_eq!(conventional_prefix("fix(a b): x"), None);
    }

    #[test]
    fn overall_score_ignores_merges() {
        let good = commit("Return an error for an open range", &[]);
        let merge = commit("Merge branch 'main'", &[]);
        let r = review_all(&[good.clone(), merge], &Conventions::default());
        assert_eq!(r.score, Some(100));
        assert_eq!(review_all(&[], &Conventions::default()).score, None);
    }
}
