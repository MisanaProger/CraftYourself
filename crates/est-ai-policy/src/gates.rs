//! Permission gates for actions outside the learner's machine (spec v0.3,
//! stage 5).
//!
//! Two kinds of external actions exist:
//!
//! * **fetch** — reading public data from a code forge (GitHub, GitLab) or an
//!   online judge (Codeforces, LeetCode). Only the address of the page leaves
//!   the machine. Allowed when the learner started it, gave explicit network
//!   permission for this request, and the host is a known one.
//! * **publish** — writing to a forge on the learner's behalf (a comment on
//!   an Issue, a new Issue, a pull request). Never in one step: the
//!   application prepares a draft, shows exactly what will be sent where,
//!   and publishes only when the learner confirms *that* draft (same digest)
//!   before it expires. A confirmation is used once.
//!
//! The mentor (a model) can initiate neither: an action it asks for is
//! denied, whatever the engagement level.

use est_core::Timestamp;
use est_domain::EngagementLevel;
use serde::{Deserialize, Serialize};

/// Hosts EST reads from with the learner's permission.
pub const FETCH_HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "gitlab.com",
    "codeforces.com",
    "leetcode.com",
];

/// Hosts EST may publish to (with confirmation).
pub const PUBLISH_HOSTS: &[&str] = &["github.com", "gitlab.com"];

/// A draft can be confirmed within this many seconds.
pub const CONFIRMATION_TTL_SECS: i64 = 15 * 60;

/// Longest text EST publishes.
pub const MAX_PUBLISH_CHARS: usize = 8_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Initiator {
    /// The learner pressed a button or sent a request.
    Learner,
    /// The model asked for the action in an answer.
    Mentor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishKind {
    IssueComment,
    Issue,
    PullRequest,
}

impl PublishKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PublishKind::IssueComment => "issue_comment",
            PublishKind::Issue => "issue",
            PublishKind::PullRequest => "pull_request",
        }
    }
}

/// Something EST would send to a forge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishDraft {
    pub kind: PublishKind,
    pub host: String,
    /// Page the text is published on, e.g. the Issue's link.
    pub target: String,
    pub body: String,
}

impl PublishDraft {
    /// Identifies the exact content; the confirmation must name it.
    /// FNV-1a over the kind, host, target and body: the draft is stored on
    /// the server, the digest only proves the learner confirmed this text.
    pub fn digest(&self) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for part in [self.kind.as_str(), &self.host, &self.target, &self.body] {
            for b in part.bytes().chain([0u8]) {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        }
        format!("{h:016x}")
    }
}

/// Facts about the work the draft belongs to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublishContext {
    /// Highest engagement level reached in the Issue's conversation.
    pub highest_level: Option<EngagementLevel>,
}

/// The learner's confirmation of a draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirmation {
    pub digest: String,
    pub drafted_at: Timestamp,
    pub now: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Warning {
    /// The mentor showed code (L3+) in this Issue; the project's
    /// contribution rules may require saying so.
    MentorAssisted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum GateDecision {
    Allow,
    /// The learner has to allow network access for this request.
    NeedsPermission {
        reason: String,
    },
    /// The learner has to confirm this draft.
    NeedsConfirmation {
        digest: String,
        warnings: Vec<Warning>,
    },
    Deny {
        reason: String,
    },
}

impl GateDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, GateDecision::Allow)
    }

    fn deny(reason: impl Into<String>) -> Self {
        GateDecision::Deny {
            reason: reason.into(),
        }
    }
}

fn normalized_host(host: &str) -> String {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    host.strip_prefix("www.").unwrap_or(&host).to_owned()
}

fn known(host: &str, builtin: &[&str], extra: &[String]) -> bool {
    let host = normalized_host(host);
    builtin.contains(&host.as_str()) || extra.iter().any(|h| normalized_host(h) == host)
}

/// May EST read from `host` now? `extra_hosts` are hosts the learner added
/// in the configuration (e.g. a self-hosted GitLab).
pub fn check_fetch(
    initiator: Initiator,
    host: &str,
    network_permitted: bool,
    extra_hosts: &[String],
) -> GateDecision {
    if initiator == Initiator::Mentor {
        return GateDecision::deny("the mentor cannot access the network");
    }
    if !known(host, FETCH_HOSTS, extra_hosts) {
        return GateDecision::deny(format!("`{host}` is not a known forge or judge"));
    }
    if !network_permitted {
        return GateDecision::NeedsPermission {
            reason: format!(
                "reading from {} needs network access",
                normalized_host(host)
            ),
        };
    }
    GateDecision::Allow
}

/// Text that looks like a credential is never published.
fn looks_like_secret(text: &str) -> bool {
    const MARKERS: &[&str] = &[
        "-----BEGIN",
        "ghp_",
        "gho_",
        "ghs_",
        "github_pat_",
        "glpat-",
        "sk-ant-",
        "AKIA",
        "xoxb-",
        "xoxp-",
    ];
    if MARKERS.iter().any(|m| text.contains(m)) {
        return true;
    }
    // `sk-` followed by a long token (OpenAI-style keys).
    text.match_indices("sk-").any(|(i, _)| {
        text[i + 3..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .count()
            >= 20
    })
}

/// May EST publish `draft` now? Without a matching confirmation the answer
/// is at best [`GateDecision::NeedsConfirmation`].
pub fn check_publish(
    initiator: Initiator,
    draft: &PublishDraft,
    context: &PublishContext,
    confirmation: Option<&Confirmation>,
    extra_hosts: &[String],
) -> GateDecision {
    if initiator == Initiator::Mentor {
        return GateDecision::deny("the mentor cannot publish on the learner's behalf");
    }
    if !known(&draft.host, PUBLISH_HOSTS, extra_hosts) {
        return GateDecision::deny(format!("cannot publish to `{}`", draft.host));
    }
    let body = draft.body.trim();
    if body.is_empty() {
        return GateDecision::deny("nothing to publish");
    }
    if body.chars().count() > MAX_PUBLISH_CHARS {
        return GateDecision::deny(format!(
            "the text is longer than {MAX_PUBLISH_CHARS} characters"
        ));
    }
    if looks_like_secret(&draft.body) || looks_like_secret(&draft.target) {
        return GateDecision::deny("the text looks like it contains a key or token");
    }
    let digest = draft.digest();
    let Some(c) = confirmation else {
        let mut warnings = Vec::new();
        if context
            .highest_level
            .is_some_and(|l| l >= EngagementLevel::Demonstration)
        {
            warnings.push(Warning::MentorAssisted);
        }
        return GateDecision::NeedsConfirmation { digest, warnings };
    };
    if c.digest != digest {
        return GateDecision::deny("the confirmation is for a different text");
    }
    let age = (c.now - c.drafted_at).num_seconds();
    if !(0..=CONFIRMATION_TTL_SECS).contains(&age) {
        return GateDecision::deny("the draft has expired; prepare it again");
    }
    GateDecision::Allow
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn draft(body: &str) -> PublishDraft {
        PublishDraft {
            kind: PublishKind::IssueComment,
            host: "github.com".into(),
            target: "https://github.com/o/r/issues/1".into(),
            body: body.into(),
        }
    }

    fn confirm(d: &PublishDraft, age: i64) -> Confirmation {
        let now = est_core::now();
        Confirmation {
            digest: d.digest(),
            drafted_at: now - Duration::seconds(age),
            now,
        }
    }

    #[test]
    fn fetch_needs_learner_permission_and_known_host() {
        use Initiator::*;
        assert!(check_fetch(Learner, "api.github.com", true, &[]).is_allowed());
        assert!(check_fetch(Learner, "www.Codeforces.com", true, &[]).is_allowed());
        assert!(matches!(
            check_fetch(Learner, "leetcode.com", false, &[]),
            GateDecision::NeedsPermission { .. }
        ));
        assert!(matches!(
            check_fetch(Mentor, "github.com", true, &[]),
            GateDecision::Deny { .. }
        ));
        assert!(matches!(
            check_fetch(Learner, "evil.example", true, &[]),
            GateDecision::Deny { .. }
        ));
        assert!(
            check_fetch(
                Learner,
                "git.corp.example",
                true,
                &["git.corp.example".into()]
            )
            .is_allowed()
        );
    }

    #[test]
    fn publishing_is_two_step() {
        let d = draft("I'd like to work on this.");
        let ctx = PublishContext::default();
        let first = check_publish(Initiator::Learner, &d, &ctx, None, &[]);
        assert_eq!(
            first,
            GateDecision::NeedsConfirmation {
                digest: d.digest(),
                warnings: vec![]
            }
        );
        assert!(
            check_publish(Initiator::Learner, &d, &ctx, Some(&confirm(&d, 5)), &[]).is_allowed()
        );
        // Edited after the confirmation was issued.
        let other = draft("Something else.");
        assert!(matches!(
            check_publish(Initiator::Learner, &other, &ctx, Some(&confirm(&d, 5)), &[]),
            GateDecision::Deny { .. }
        ));
        // Expired.
        assert!(matches!(
            check_publish(
                Initiator::Learner,
                &d,
                &ctx,
                Some(&confirm(&d, CONFIRMATION_TTL_SECS + 1)),
                &[]
            ),
            GateDecision::Deny { .. }
        ));
    }

    #[test]
    fn mentor_never_publishes() {
        let d = draft("hello");
        assert!(matches!(
            check_publish(
                Initiator::Mentor,
                &d,
                &PublishContext::default(),
                Some(&confirm(&d, 1)),
                &[]
            ),
            GateDecision::Deny { .. }
        ));
    }

    #[test]
    fn refuses_secrets_empty_text_and_unknown_hosts() {
        let ctx = PublishContext::default();
        for body in [
            "",
            "   ",
            "token ghp_abcdefghijklmnop",
            "key: sk-proj-abcdefghijklmnopqrstuvwxyz",
            "-----BEGIN OPENSSH PRIVATE KEY-----",
        ] {
            assert!(
                matches!(
                    check_publish(Initiator::Learner, &draft(body), &ctx, None, &[]),
                    GateDecision::Deny { .. }
                ),
                "{body}"
            );
        }
        // A short `sk-` is ordinary text.
        assert!(matches!(
            check_publish(
                Initiator::Learner,
                &draft("task-sk-1 done"),
                &ctx,
                None,
                &[]
            ),
            GateDecision::NeedsConfirmation { .. }
        ));
        let mut d = draft("hi");
        d.host = "example.com".into();
        assert!(matches!(
            check_publish(Initiator::Learner, &d, &ctx, None, &[]),
            GateDecision::Deny { .. }
        ));
        let long = draft(&"x".repeat(MAX_PUBLISH_CHARS + 1));
        assert!(matches!(
            check_publish(Initiator::Learner, &long, &ctx, None, &[]),
            GateDecision::Deny { .. }
        ));
    }

    #[test]
    fn warns_when_the_mentor_showed_code() {
        let ctx = PublishContext {
            highest_level: Some(EngagementLevel::Demonstration),
        };
        let GateDecision::NeedsConfirmation { warnings, .. } =
            check_publish(Initiator::Learner, &draft("PR ready"), &ctx, None, &[])
        else {
            panic!()
        };
        assert_eq!(warnings, vec![Warning::MentorAssisted]);
    }
}
