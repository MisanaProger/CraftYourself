//! Open-source Issues as learning tasks (spec v0.3, stage 3).
//!
//! The learner pastes the link of an Issue on GitHub or GitLab. EST reads
//! the Issue and its repository (with the learner's network permission, see
//! `est_ai_policy::gates`), checks whether it suits a learner, and turns it
//! into an ordinary task: title, description, type, difficulty and the
//! checklist of the Issue as acceptance criteria.
//!
//! This crate is pure: it recognises links, judges suitability with fixed
//! rules and drafts the task. Fetching is an adapter's job.

use chrono::Duration;
use est_core::Timestamp;
use est_domain::{Difficulty, TaskType};
use serde::{Deserialize, Serialize};

pub const MAX_DESCRIPTION_BYTES: usize = 24 * 1024;
pub const MAX_CRITERIA: usize = 20;
const MAX_CRITERION_CHARS: usize = 500;
const MAX_TITLE_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueError(pub String);

impl std::fmt::Display for IssueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for IssueError {}

fn invalid(why: impl Into<String>) -> IssueError {
    IssueError(why.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Forge {
    Github,
    Gitlab,
}

impl Forge {
    pub fn as_str(self) -> &'static str {
        match self {
            Forge::Github => "github",
            Forge::Gitlab => "gitlab",
        }
    }
}

/// A recognised Issue link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueRef {
    pub forge: Forge,
    /// `github.com`, `gitlab.com` or a self-hosted GitLab.
    pub host: String,
    /// `owner/repo` on GitHub; `group/subgroup/project` on GitLab.
    pub project: String,
    pub number: u64,
    /// Canonical link of the Issue.
    pub url: String,
}

impl IssueRef {
    /// Clone URL of the repository the Issue belongs to.
    pub fn clone_url(&self) -> String {
        format!("https://{}/{}.git", self.host, self.project)
    }
}

fn is_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Recognises `https://github.com/<owner>/<repo>/issues/<n>` and
/// `https://gitlab.com/<group>/…/<project>/-/issues/<n>`. `gitlab_hosts`
/// are self-hosted GitLab instances the learner configured.
pub fn parse_issue_url(raw: &str, gitlab_hosts: &[String]) -> Result<IssueRef, IssueError> {
    let url = url::Url::parse(raw.trim()).map_err(|e| invalid(format!("link: {e}")))?;
    if url.scheme() != "https" {
        return Err(invalid("the link must start with https://"));
    }
    if !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
        return Err(invalid("the link must not contain credentials or a port"));
    }
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_owned();
    let seg: Vec<&str> = url
        .path_segments()
        .map(|s| s.filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();
    let number = |s: &str| -> Result<u64, IssueError> {
        s.parse::<u64>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("the Issue number is missing"))
    };
    let gitlab = host == "gitlab.com"
        || gitlab_hosts
            .iter()
            .any(|h| h.trim().eq_ignore_ascii_case(&host));
    if host == "github.com" {
        let [owner, repo, "issues", n, ..] = seg.as_slice() else {
            return Err(invalid("not a GitHub Issue link"));
        };
        let repo = repo.trim_end_matches(".git");
        if !is_name(owner) || !is_name(repo) {
            return Err(invalid("not a GitHub Issue link"));
        }
        let number = number(n)?;
        let project = format!("{owner}/{repo}");
        return Ok(IssueRef {
            forge: Forge::Github,
            url: format!("https://github.com/{project}/issues/{number}"),
            host,
            project,
            number,
        });
    }
    if gitlab {
        let pos = seg
            .iter()
            .position(|s| *s == "issues")
            .ok_or_else(|| invalid("not a GitLab Issue link"))?;
        let mut path: Vec<&str> = seg[..pos].to_vec();
        if path.last() == Some(&"-") {
            path.pop();
        }
        if path.len() < 2 || path.len() > 20 || !path.iter().all(|s| is_name(s)) {
            return Err(invalid("not a GitLab Issue link"));
        }
        let number = number(seg.get(pos + 1).copied().unwrap_or(""))?;
        let project = path.join("/");
        return Ok(IssueRef {
            forge: Forge::Gitlab,
            url: format!("https://{host}/{project}/-/issues/{number}"),
            host,
            project,
            number,
        });
    }
    Err(invalid(
        "only GitHub and GitLab Issue links are supported (add a self-hosted GitLab in the configuration)",
    ))
}

/// What the forge reports about an Issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteIssue {
    pub title: String,
    #[serde(default)]
    pub body: String,
    pub open: bool,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub assignees: u32,
    #[serde(default)]
    pub comments: u32,
    /// GitHub lists pull requests among Issues.
    #[serde(default)]
    pub pull_request: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub created_at: Option<Timestamp>,
    #[serde(default)]
    pub updated_at: Option<Timestamp>,
    pub repository: RemoteRepository,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteRepository {
    /// Clone URL reported by the forge (https).
    pub clone_url: String,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Speaks for the Issue.
    Good,
    /// Worth knowing; does not prevent the import.
    Caution,
    /// The Issue cannot be imported.
    Blocker,
}

/// One finding of the suitability check; `code` is translated by the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Suitable,
    Caution,
    Unsuitable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suitability {
    pub verdict: Verdict,
    pub findings: Vec<Finding>,
}

impl Suitability {
    pub fn importable(&self) -> bool {
        self.verdict != Verdict::Unsuitable
    }
}

fn label_is(labels: &[String], names: &[&str]) -> Option<String> {
    labels
        .iter()
        .find(|l| {
            let l = l.to_ascii_lowercase();
            let l = l.rsplit([':', '/']).next().unwrap_or(&l).trim();
            names.contains(&l)
        })
        .cloned()
}

const BEGINNER_LABELS: &[&str] = &[
    "good first issue",
    "good-first-issue",
    "beginner",
    "beginner friendly",
    "easy",
    "starter",
    "first-timers-only",
    "quick fix",
];
const HELP_LABELS: &[&str] = &["help wanted", "help-wanted", "contributions welcome"];
const REJECTED_LABELS: &[&str] = &[
    "duplicate",
    "invalid",
    "wontfix",
    "won't fix",
    "spam",
    "not planned",
];
const DISCUSSION_LABELS: &[&str] = &[
    "question",
    "discussion",
    "needs discussion",
    "needs triage",
    "rfc",
    "proposal",
];

/// Fixed rules: no model is asked whether an Issue suits a learner.
pub fn assess(issue: &RemoteIssue, now: Timestamp) -> Suitability {
    let mut f = Vec::new();
    let mut add = |code: &str, severity, detail: Option<String>| {
        f.push(Finding {
            code: code.into(),
            severity,
            detail,
        })
    };
    use Severity::*;
    if issue.pull_request {
        add("pull_request", Blocker, None);
    }
    if !issue.open {
        add("closed", Blocker, None);
    }
    if issue.repository.archived {
        add("archived", Blocker, None);
    }
    if let Some(l) = label_is(&issue.labels, REJECTED_LABELS) {
        add("rejected_label", Blocker, Some(l));
    }
    if issue.locked {
        add("locked", Caution, None);
    }
    if issue.assignees > 0 {
        add("assigned", Caution, None);
    }
    let body = clean_body(&issue.body);
    let body_chars = body.chars().count();
    if body_chars == 0 {
        add("no_description", Caution, None);
    } else if body_chars < 80 {
        add("short_description", Caution, None);
    } else if body.len() > MAX_DESCRIPTION_BYTES {
        add("long_description", Caution, None);
    }
    if let Some(l) = label_is(&issue.labels, DISCUSSION_LABELS) {
        add("discussion", Caution, Some(l));
    }
    if issue.comments >= 30 {
        add("long_discussion", Caution, Some(issue.comments.to_string()));
    }
    if let Some(updated) = issue.updated_at
        && now - updated > Duration::days(730)
    {
        add(
            "stale",
            Caution,
            Some(updated.format("%Y-%m-%d").to_string()),
        );
    }
    if issue.repository.license.is_none() {
        add("no_license", Caution, None);
    }
    if let Some(l) = label_is(&issue.labels, BEGINNER_LABELS) {
        add("beginner_label", Good, Some(l));
    }
    if let Some(l) = label_is(&issue.labels, HELP_LABELS) {
        add("help_wanted", Good, Some(l));
    }
    if !criteria_from_body(&issue.body).is_empty() {
        add("checklist", Good, None);
    }
    let worst = f.iter().map(|x| x.severity).max().unwrap_or(Good);
    let verdict = match worst {
        Blocker => Verdict::Unsuitable,
        Caution => Verdict::Caution,
        Good => Verdict::Suitable,
    };
    f.sort_by_key(|x| std::cmp::Reverse(x.severity));
    Suitability {
        verdict,
        findings: f,
    }
}

/// Removes HTML comments (Issue templates leave them) and trims.
pub fn clean_body(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out.replace("\r\n", "\n").trim().to_owned()
}

/// Unchecked and checked task-list items (`- [ ] …`, `* [x] …`).
pub fn criteria_from_body(body: &str) -> Vec<(String, bool)> {
    clean_body(body)
        .lines()
        .filter_map(|line| {
            let t = line.trim_start();
            let t = t
                .strip_prefix("- ")
                .or_else(|| t.strip_prefix("* "))
                .or_else(|| t.strip_prefix("+ "))?;
            let (done, text) = match t.strip_prefix("[ ]") {
                Some(r) => (false, r),
                None => (
                    true,
                    t.strip_prefix("[x]").or_else(|| t.strip_prefix("[X]"))?,
                ),
            };
            let text: String = text.trim().chars().take(MAX_CRITERION_CHARS).collect();
            (!text.is_empty()).then_some((text, done))
        })
        .take(MAX_CRITERIA)
        .collect()
}

fn truncate_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// The task an Issue becomes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskDraft {
    pub title: String,
    pub description: String,
    pub task_type: TaskType,
    pub difficulty: Difficulty,
    /// Unchecked checklist items of the Issue.
    pub acceptance_criteria: Vec<String>,
    pub disciplines: Vec<String>,
}

pub fn task_type(labels: &[String]) -> TaskType {
    let has = |names: &[&str]| label_is(labels, names).is_some();
    if has(&["regression"]) {
        TaskType::Regression
    } else if has(&["bug", "kind/bug", "type: bug", "defect", "crash"]) {
        TaskType::BugFix
    } else if has(&["performance", "perf", "optimization", "speed"]) {
        TaskType::Performance
    } else if has(&["documentation", "docs", "doc"]) {
        TaskType::Documentation
    } else if has(&["test", "tests", "testing", "coverage"]) {
        TaskType::Testing
    } else if has(&[
        "refactor",
        "refactoring",
        "cleanup",
        "tech debt",
        "code quality",
    ]) {
        TaskType::Refactoring
    } else {
        TaskType::Feature
    }
}

pub fn draft_task(issue_ref: &IssueRef, issue: &RemoteIssue) -> TaskDraft {
    let task_type = task_type(&issue.labels);
    let difficulty = if label_is(&issue.labels, BEGINNER_LABELS).is_some() {
        Difficulty::Beginner
    } else {
        Difficulty::Intermediate
    };
    let title: String = issue.title.trim().chars().take(MAX_TITLE_CHARS).collect();
    let title = if title.is_empty() {
        format!("{}#{}", issue_ref.project, issue_ref.number)
    } else {
        title
    };
    let body = clean_body(&issue.body);
    let mut description = format!("{}\n\n", issue_ref.url);
    let room = MAX_DESCRIPTION_BYTES.saturating_sub(description.len());
    let clipped = truncate_bytes(&body, room);
    description.push_str(clipped);
    if clipped.len() < body.len() {
        description.push_str("\n\n…");
    }
    let disciplines = match task_type {
        TaskType::BugFix | TaskType::Regression | TaskType::Testing => {
            vec!["debugging-testing".to_owned()]
        }
        TaskType::Refactoring => vec!["architecture".to_owned()],
        TaskType::Performance => vec!["algorithms".to_owned()],
        _ => Vec::new(),
    };
    TaskDraft {
        title,
        description,
        task_type,
        difficulty,
        acceptance_criteria: criteria_from_body(&issue.body)
            .into_iter()
            .filter(|(_, done)| !done)
            .map(|(t, _)| t)
            .collect(),
        disciplines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue() -> RemoteIssue {
        RemoteIssue {
            title: "Parser panics on empty input".into(),
            body: "<!-- template: describe the bug -->\nCalling `parse(\"\")` panics instead of returning an error. Steps:\n\n- [ ] return `Err` for empty input\n- [x] reproduce\n* [ ] add a test\n".into(),
            open: true,
            labels: vec!["bug".into(), "good first issue".into()],
            assignees: 0,
            comments: 2,
            pull_request: false,
            locked: false,
            created_at: None,
            updated_at: Some(est_core::now()),
            repository: RemoteRepository {
                clone_url: "https://github.com/o/r.git".into(),
                default_branch: Some("main".into()),
                archived: false,
                license: Some("MIT".into()),
                language: Some("Rust".into()),
            },
        }
    }

    #[test]
    fn parses_github_and_gitlab_links() {
        let r = parse_issue_url("https://github.com/rust-lang/rust/issues/42?x=1#c", &[]).unwrap();
        assert_eq!(
            (r.forge, r.project.as_str(), r.number),
            (Forge::Github, "rust-lang/rust", 42)
        );
        assert_eq!(r.url, "https://github.com/rust-lang/rust/issues/42");
        assert_eq!(r.clone_url(), "https://github.com/rust-lang/rust.git");
        let r = parse_issue_url("https://gitlab.com/gnome/sub/app/-/issues/7", &[]).unwrap();
        assert_eq!(
            (r.forge, r.project.as_str(), r.number),
            (Forge::Gitlab, "gnome/sub/app", 7)
        );
        assert_eq!(r.url, "https://gitlab.com/gnome/sub/app/-/issues/7");
        let r = parse_issue_url(
            "https://git.corp.example/team/app/-/issues/3",
            &["git.corp.example".into()],
        )
        .unwrap();
        assert_eq!(r.host, "git.corp.example");
        for bad in [
            "http://github.com/o/r/issues/1",
            "https://github.com/o/r/pull/1",
            "https://github.com/o/r/issues/0",
            "https://github.com/o/r/issues/x",
            "https://github.com/../r/issues/1",
            "https://user:pw@github.com/o/r/issues/1",
            "https://example.com/o/r/issues/1",
            "https://gitlab.com/r/-/issues/1",
            "not a link",
        ] {
            assert!(parse_issue_url(bad, &[]).is_err(), "{bad}");
        }
    }

    #[test]
    fn good_first_issue_is_suitable() {
        let s = assess(&issue(), est_core::now());
        assert_eq!(s.verdict, Verdict::Suitable, "{s:?}");
        let codes: Vec<_> = s.findings.iter().map(|f| f.code.as_str()).collect();
        assert_eq!(codes, ["beginner_label", "checklist"]);
    }

    #[test]
    fn blockers_and_cautions() {
        let mut i = issue();
        i.open = false;
        i.labels.push("duplicate".into());
        let s = assess(&i, est_core::now());
        assert_eq!(s.verdict, Verdict::Unsuitable);
        assert!(!s.importable());
        assert_eq!(s.findings[0].severity, Severity::Blocker);

        let mut i = issue();
        i.assignees = 1;
        i.body = "Fix it".into();
        i.repository.license = None;
        i.updated_at = Some(est_core::now() - Duration::days(1000));
        let s = assess(&i, est_core::now());
        assert_eq!(s.verdict, Verdict::Caution);
        let codes: Vec<_> = s.findings.iter().map(|f| f.code.as_str()).collect();
        for c in ["assigned", "short_description", "no_license", "stale"] {
            assert!(codes.contains(&c), "{c} in {codes:?}");
        }
        let mut i = issue();
        i.pull_request = true;
        assert_eq!(assess(&i, est_core::now()).verdict, Verdict::Unsuitable);
    }

    #[test]
    fn drafts_a_task() {
        let r = parse_issue_url("https://github.com/o/r/issues/5", &[]).unwrap();
        let d = draft_task(&r, &issue());
        assert_eq!(d.task_type, TaskType::BugFix);
        assert_eq!(d.difficulty, Difficulty::Beginner);
        assert_eq!(
            d.acceptance_criteria,
            ["return `Err` for empty input", "add a test"]
        );
        assert!(
            d.description
                .starts_with("https://github.com/o/r/issues/5\n\nCalling")
        );
        assert!(!d.description.contains("template"));
        assert_eq!(d.disciplines, ["debugging-testing"]);

        let mut long = issue();
        long.body = "é".repeat(MAX_DESCRIPTION_BYTES);
        let d = draft_task(&r, &long);
        assert!(d.description.len() <= MAX_DESCRIPTION_BYTES + 8);
        assert!(d.description.ends_with('…'));
    }

    #[test]
    fn task_type_from_labels() {
        let t = |l: &[&str]| task_type(&l.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(t(&["type: documentation"]), TaskType::Documentation);
        assert_eq!(t(&["kind/bug", "regression"]), TaskType::Regression);
        assert_eq!(t(&["Performance"]), TaskType::Performance);
        assert_eq!(t(&["enhancement"]), TaskType::Feature);
    }
}
