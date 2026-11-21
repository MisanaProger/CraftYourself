//! Pull request drafts.
//!
//! The draft is assembled from observed data only — the learner's commits,
//! the acceptance criteria, the last test run in the sandbox, the highest
//! help level — so it describes what was done, not what a model imagined.
//! The learner edits it before anything is published (through the publish
//! gate). Readiness is a fixed checklist.

use serde::{Deserialize, Serialize};

/// The last test run in the sandbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestRun {
    pub command: String,
    pub passed: bool,
    /// E.g. `3 passed, 0 failed`.
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrInput {
    pub task_title: String,
    /// Link of the Issue the task was imported from.
    pub issue_url: Option<String>,
    /// The Issue's number when it is in the pull request's repository.
    pub closes: Option<u64>,
    /// Subjects of the learner's commits, oldest first.
    pub commits: Vec<String>,
    pub criteria: Vec<(String, bool)>,
    pub last_test: Option<TestRun>,
    pub uncommitted_files: u32,
    /// Commit review score.
    pub commit_score: Option<u8>,
    /// Some commit is `fixup!`/`squash!`.
    pub has_fixups: bool,
    /// Highest engagement level used in the Issue (0–4).
    pub highest_level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessItem {
    pub code: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrDraft {
    pub title: String,
    pub body: String,
    pub readiness: Vec<ReadinessItem>,
    /// Every readiness item is satisfied.
    pub ready: bool,
}

pub const COMMIT_SCORE_READY: u8 = 70;

fn clip(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let mut t: String = s.chars().take(max - 1).collect();
        t.push('…');
        t
    }
}

/// Builds the draft and its readiness checklist.
pub fn draft(input: &PrInput) -> PrDraft {
    let title = match input.commits.as_slice() {
        [only] => clip(only, 72),
        _ => clip(&input.task_title, 72),
    };
    let mut body = String::new();
    match (&input.closes, &input.issue_url) {
        (Some(n), _) => body.push_str(&format!("Closes #{n}.\n\n")),
        (None, Some(url)) => body.push_str(&format!("Refs {url}\n\n")),
        _ => {}
    }
    if !input.commits.is_empty() {
        body.push_str("## Changes\n\n");
        for c in &input.commits {
            body.push_str(&format!("- {c}\n"));
        }
        body.push('\n');
    }
    if !input.criteria.is_empty() {
        body.push_str("## Acceptance criteria\n\n");
        for (text, done) in &input.criteria {
            body.push_str(&format!("- [{}] {text}\n", if *done { "x" } else { " " }));
        }
        body.push('\n');
    }
    if let Some(t) = &input.last_test {
        body.push_str("## Tests\n\n");
        body.push_str(&format!(
            "`{}` — {}{}\n\n",
            t.command,
            if t.passed { "passed" } else { "failed" },
            t.summary
                .as_deref()
                .map(|s| format!(" ({s})"))
                .unwrap_or_default()
        ));
    }
    if input.highest_level >= 3 {
        body.push_str(
            "## AI assistance\n\nParts of this change were made with an AI mentor that showed code. \
             I reviewed and tested everything I am submitting.\n",
        );
    }

    let item = |code: &str, ok: bool, detail: Option<String>| ReadinessItem {
        code: code.into(),
        ok,
        detail,
    };
    let done = input.criteria.iter().filter(|(_, d)| *d).count();
    let readiness = vec![
        item(
            "has_commits",
            !input.commits.is_empty(),
            Some(input.commits.len().to_string()),
        ),
        item(
            "clean_worktree",
            input.uncommitted_files == 0,
            (input.uncommitted_files > 0).then(|| input.uncommitted_files.to_string()),
        ),
        item(
            "criteria_done",
            done == input.criteria.len(),
            (!input.criteria.is_empty()).then(|| format!("{done}/{}", input.criteria.len())),
        ),
        item(
            "tests_passed",
            input.last_test.as_ref().is_some_and(|t| t.passed),
            input.last_test.as_ref().map(|t| t.command.clone()),
        ),
        item(
            "commit_quality",
            input.commit_score.is_some_and(|s| s >= COMMIT_SCORE_READY),
            input.commit_score.map(|s| s.to_string()),
        ),
        item("no_fixups", !input.has_fixups, None),
    ];
    PrDraft {
        title,
        body: body.trim_end().to_owned() + "\n",
        ready: readiness.iter().all(|r| r.ok),
        readiness,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> PrInput {
        PrInput {
            task_title: "parse_range panics on `5..`".into(),
            issue_url: Some("https://github.com/o/r/issues/5".into()),
            closes: Some(5),
            commits: vec!["Return an error for an open range".into()],
            criteria: vec![("`5..` returns an error".into(), true)],
            last_test: Some(TestRun {
                command: "cargo test".into(),
                passed: true,
                summary: Some("3 passed, 0 failed".into()),
            }),
            uncommitted_files: 0,
            commit_score: Some(95),
            has_fixups: false,
            highest_level: 1,
        }
    }

    #[test]
    fn a_ready_draft() {
        let d = draft(&input());
        assert!(d.ready, "{:?}", d.readiness);
        assert_eq!(d.title, "Return an error for an open range");
        assert!(
            d.body
                .starts_with("Closes #5.\n\n## Changes\n\n- Return an error")
        );
        assert!(d.body.contains("- [x] `5..` returns an error"));
        assert!(
            d.body
                .contains("`cargo test` — passed (3 passed, 0 failed)")
        );
        assert!(!d.body.contains("AI assistance"));
    }

    #[test]
    fn not_ready_and_disclosed() {
        let mut i = input();
        i.commits.push("fixup! Return an error".into());
        i.has_fixups = true;
        i.uncommitted_files = 2;
        i.last_test = None;
        i.closes = None;
        i.highest_level = 3;
        let d = draft(&i);
        assert!(!d.ready);
        assert_eq!(d.title, "parse_range panics on `5..`");
        let failed: Vec<_> = d
            .readiness
            .iter()
            .filter(|r| !r.ok)
            .map(|r| r.code.as_str())
            .collect();
        assert_eq!(failed, ["clean_worktree", "tests_passed", "no_fixups"]);
        assert!(d.body.starts_with("Refs https://github.com/o/r/issues/5"));
        assert!(d.body.contains("## AI assistance"));
    }
}
