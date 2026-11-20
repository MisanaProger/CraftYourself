//! Exercise Manifest (spec 3.3).
//!
//! A manifest is a declarative TOML document that fully describes one
//! exercise: where the code comes from (pinned to an exact commit), what the
//! learner should achieve, how to build and test it, how the result is
//! assessed and how far the AI Mentor is allowed to go.
//!
//! The format is versioned through the top-level `manifest_version` key so
//! that manifests can be exchanged between Local and Cloud editions.

use std::collections::HashSet;

use est_core::{ProjectId, is_slug};
use est_domain::{
    Difficulty, DocReference, EngagementLevel, MentorSettings, NewTask, TaskSource, TaskType,
};
use serde::{Deserialize, Serialize};
use url::Url;

/// Manifest format versions this build understands.
pub const SUPPORTED_MANIFEST_VERSIONS: &[u32] = &[1];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExerciseManifest {
    /// Version of the manifest *format*, not of the exercise content.
    pub manifest_version: u32,
    pub exercise: ExerciseInfo,
    pub source: SourceSpec,
    #[serde(default)]
    pub conditions: Conditions,
    pub commands: Commands,
    #[serde(default, rename = "criteria")]
    pub criteria: Vec<Criterion>,
    #[serde(default)]
    pub mentor: MentorLimits,
    #[serde(default, rename = "docs")]
    pub docs: Vec<DocLink>,
    #[serde(default, rename = "materials")]
    pub materials: Vec<MaterialRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExerciseInfo {
    pub id: String,
    /// Version of the exercise content (bumped by the author on change).
    pub version: String,
    pub title: String,
    pub description: String,
    pub task_type: TaskType,
    pub difficulty: Difficulty,
    #[serde(default)]
    pub learning_goals: Vec<String>,
    #[serde(default)]
    /// Discipline ids from the Engineering Skill Matrix catalog.
    pub disciplines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSpec {
    /// Clone URL of the upstream repository.
    pub repository: String,
    /// Full 40-character commit hash the exercise is pinned to.
    pub revision: String,
    /// Optional branch the revision lives on (informational / for fetching).
    #[serde(default)]
    pub branch: Option<String>,
    /// SPDX license expression of the upstream project.
    pub license: String,
    /// Primary programming language, e.g. `rust`.
    pub language: String,
    /// Upstream issue this exercise is based on, if any.
    #[serde(default)]
    pub issue_url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conditions {
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub time_limit_minutes: Option<u32>,
    /// Paths (globs) the learner is expected to modify. Informational.
    #[serde(default)]
    pub scope: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Commands {
    #[serde(default)]
    pub setup: Option<String>,
    #[serde(default)]
    pub build: Option<String>,
    pub test: String,
    #[serde(default)]
    pub run: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CriterionKind {
    /// A command that must exit with status 0.
    Command,
    /// Manual / reviewer assessment.
    Manual,
    /// Assessment of the commit history (messages, granularity).
    Commits,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Criterion {
    pub id: String,
    pub kind: CriterionKind,
    pub description: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default = "default_weight")]
    pub weight: u32,
}

fn default_weight() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MentorLimits {
    /// Highest engagement level reachable by escalation (default L4).
    /// Every Issue still starts at L0.
    #[serde(default = "default_max_level")]
    pub max_level: EngagementLevel,
    /// Paths (globs, relative to the repository root) the mentor must never read,
    /// e.g. hidden reference tests.
    #[serde(default)]
    pub denied_paths: Vec<String>,
}

fn default_max_level() -> EngagementLevel {
    EngagementLevel::Active
}

impl Default for MentorLimits {
    fn default() -> Self {
        Self {
            max_level: default_max_level(),
            denied_paths: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocLink {
    pub title: String,
    pub url: String,
    /// Symbols / topics this link is the primary source for. Used by the
    /// Documentation Router to send the learner to a precise location.
    #[serde(default)]
    pub topics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialRef {
    pub artifact_id: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("manifest is not valid TOML or has an unexpected shape: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("unsupported manifest_version {found}; supported: {supported:?}")]
    UnsupportedVersion {
        found: u32,
        supported: &'static [u32],
    },
    #[error("manifest validation failed:\n  - {}", .0.join("\n  - "))]
    Invalid(Vec<String>),
}

impl ExerciseManifest {
    /// Parses and validates a manifest from TOML text.
    pub fn from_toml(text: &str) -> Result<Self, ManifestError> {
        // Check the format version first so that a future format produces a
        // clear error instead of a confusing "unknown field" message.
        #[derive(Deserialize)]
        struct VersionProbe {
            manifest_version: u32,
        }
        let probe: VersionProbe = toml::from_str(text)?;
        if !SUPPORTED_MANIFEST_VERSIONS.contains(&probe.manifest_version) {
            return Err(ManifestError::UnsupportedVersion {
                found: probe.manifest_version,
                supported: SUPPORTED_MANIFEST_VERSIONS,
            });
        }
        let manifest: ExerciseManifest = toml::from_str(text)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("manifest is always serializable")
    }

    /// Semantic validation that the type system cannot express.
    pub fn validate(&self) -> Result<(), ManifestError> {
        let mut errors = Vec::new();
        let ex = &self.exercise;

        if !is_slug(&ex.id) {
            errors.push(format!(
                "exercise.id `{}` must be a lowercase slug (a-z, 0-9, single dashes)",
                ex.id
            ));
        }
        if ex.version.trim().is_empty() {
            errors.push("exercise.version must not be empty".into());
        }
        if ex.title.trim().is_empty() {
            errors.push("exercise.title must not be empty".into());
        }
        if ex.description.trim().is_empty() {
            errors.push("exercise.description must not be empty".into());
        }
        if ex.learning_goals.is_empty() {
            errors.push("exercise.learning_goals must list at least one goal".into());
        }

        for d in &ex.disciplines {
            if !est_discipline::is_known_discipline(d) {
                errors.push(format!("exercise.disciplines: unknown discipline `{d}`"));
            }
        }

        let src = &self.source;
        match Url::parse(&src.repository) {
            Ok(u) if matches!(u.scheme(), "https" | "ssh" | "git" | "file") => {}
            Ok(u) => errors.push(format!(
                "source.repository uses unsupported scheme `{}`",
                u.scheme()
            )),
            Err(e) => errors.push(format!("source.repository is not a valid URL: {e}")),
        }
        if !is_full_commit_hash(&src.revision) {
            errors.push(
                "source.revision must be a full 40-character commit hash so the exercise is reproducible"
                    .into(),
            );
        }
        if src.license.trim().is_empty() {
            errors.push(
                "source.license must contain the SPDX license of the upstream project".into(),
            );
        }
        if src.language.trim().is_empty() {
            errors.push("source.language must not be empty".into());
        }
        if let Some(issue) = &src.issue_url
            && Url::parse(issue).is_err()
        {
            errors.push(format!("source.issue_url `{issue}` is not a valid URL"));
        }

        if self.commands.test.trim().is_empty() {
            errors.push("commands.test must not be empty".into());
        }

        if self.criteria.is_empty() {
            errors.push("at least one [[criteria]] entry is required".into());
        }
        let mut seen = HashSet::new();
        for c in &self.criteria {
            if !is_slug(&c.id) {
                errors.push(format!("criteria id `{}` must be a slug", c.id));
            }
            if !seen.insert(c.id.as_str()) {
                errors.push(format!("duplicate criteria id `{}`", c.id));
            }
            if c.weight == 0 {
                errors.push(format!("criteria `{}` must have a positive weight", c.id));
            }
            match (c.kind, &c.command) {
                (CriterionKind::Command, None) => errors.push(format!(
                    "criteria `{}` of kind `command` needs a `command`",
                    c.id
                )),
                (CriterionKind::Manual | CriterionKind::Commits, Some(_)) => errors.push(format!(
                    "criteria `{}`: `command` is only allowed for kind `command`",
                    c.id
                )),
                _ => {}
            }
        }

        for path in &self.mentor.denied_paths {
            if path.starts_with('/') || path.split('/').any(|seg| seg == "..") {
                errors.push(format!(
                    "mentor.denied_paths entry `{path}` must be relative to the repository root"
                ));
            }
        }

        for doc in &self.docs {
            match Url::parse(&doc.url) {
                Ok(u) if u.scheme() == "https" => {}
                _ => errors.push(format!(
                    "docs url `{}` must be an absolute https URL",
                    doc.url
                )),
            }
        }

        for m in &self.materials {
            if !is_slug(&m.artifact_id) {
                errors.push(format!(
                    "materials artifact_id `{}` must be a slug",
                    m.artifact_id
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ManifestError::Invalid(errors))
        }
    }
}

impl ExerciseManifest {
    /// The task this exercise becomes inside a connected project.
    pub fn to_new_task(&self, project_id: ProjectId) -> NewTask {
        let ex = &self.exercise;
        NewTask {
            project_id,
            title: ex.title.clone(),
            description: ex.description.trim().to_owned(),
            task_type: ex.task_type,
            difficulty: ex.difficulty,
            source: TaskSource::Exercise {
                id: ex.id.clone(),
                version: ex.version.clone(),
            },
            learning_goals: ex.learning_goals.clone(),
            disciplines: ex.disciplines.clone(),
            base_revision: Some(self.source.revision.clone()),
            test_command: Some(self.commands.test.clone()),
            mentor: Some(MentorSettings {
                max_level: self.mentor.max_level,
                denied_paths: self.mentor.denied_paths.clone(),
            }),
            docs: self
                .docs
                .iter()
                .map(|d| DocReference {
                    title: d.title.clone(),
                    url: d.url.clone(),
                    topics: d.topics.clone(),
                })
                .collect(),
        }
    }
}

pub fn is_full_commit_hash(s: &str) -> bool {
    s.len() == 40
        && s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
manifest_version = 1

[exercise]
id = "fix-off-by-one"
version = "1.0.0"
title = "Fix off-by-one in range parser"
description = "The parser drops the last element of an inclusive range."
task_type = "bug-fix"
difficulty = "beginner"
learning_goals = ["Reproduce a bug with a failing test", "Read unfamiliar parsing code"]
disciplines = ["debugging-testing"]

[source]
repository = "https://github.com/example/ranges"
revision = "0123456789abcdef0123456789abcdef01234567"
license = "MIT"
language = "rust"

[commands]
build = "cargo build"
test = "cargo test"

[[criteria]]
id = "tests-pass"
kind = "command"
description = "All tests pass"
command = "cargo test"
weight = 3

[[criteria]]
id = "regression-test"
kind = "manual"
description = "A regression test covering the bug was added"

[mentor]
max_level = 3
denied_paths = ["tests/hidden/**"]

[[docs]]
title = "RangeInclusive"
url = "https://doc.rust-lang.org/std/ops/struct.RangeInclusive.html"
topics = ["RangeInclusive", "..="]
"#;

    #[test]
    fn parses_valid_manifest() {
        let m = ExerciseManifest::from_toml(VALID).unwrap();
        assert_eq!(m.exercise.id, "fix-off-by-one");
        assert_eq!(m.mentor.max_level, EngagementLevel::Demonstration);
        assert_eq!(m.criteria[1].weight, 1);
        assert_eq!(m.exercise.task_type, TaskType::BugFix);
    }

    #[test]
    fn roundtrips_through_toml() {
        let m = ExerciseManifest::from_toml(VALID).unwrap();
        let again = ExerciseManifest::from_toml(&m.to_toml()).unwrap();
        assert_eq!(m, again);
    }

    #[test]
    fn converts_to_task() {
        let m = ExerciseManifest::from_toml(VALID).unwrap();
        let t = m.to_new_task(ProjectId::from("p"));
        t.validate().unwrap();
        assert_eq!(t.base_revision.as_deref(), Some(m.source.revision.as_str()));
        assert_eq!(t.mentor.unwrap().denied_paths, vec!["tests/hidden/**"]);
        assert!(matches!(t.source, TaskSource::Exercise { ref id, .. } if id == "fix-off-by-one"));
    }

    #[test]
    fn rejects_unknown_version() {
        let text = VALID.replace("manifest_version = 1", "manifest_version = 99");
        assert!(matches!(
            ExerciseManifest::from_toml(&text),
            Err(ManifestError::UnsupportedVersion { found: 99, .. })
        ));
    }

    #[test]
    fn rejects_unknown_fields() {
        let text = VALID.replace("license = \"MIT\"", "license = \"MIT\"\nlicence = \"typo\"");
        assert!(matches!(
            ExerciseManifest::from_toml(&text),
            Err(ManifestError::Parse(_))
        ));
    }

    #[test]
    fn collects_semantic_errors() {
        let text = VALID
            .replace("0123456789abcdef0123456789abcdef01234567", "main")
            .replace("id = \"fix-off-by-one\"", "id = \"Fix Off By One\"")
            .replace(
                "disciplines = [\"debugging-testing\"]",
                "disciplines = [\"magic\"]",
            )
            .replace("tests/hidden/**", "../outside");
        let Err(ManifestError::Invalid(errors)) = ExerciseManifest::from_toml(&text) else {
            panic!("expected validation errors");
        };
        assert!(errors.iter().any(|e| e.contains("exercise.id")));
        assert!(errors.iter().any(|e| e.contains("source.revision")));
        assert!(errors.iter().any(|e| e.contains("denied_paths")));
        assert!(
            errors
                .iter()
                .any(|e| e.contains("unknown discipline `magic`"))
        );
    }

    #[test]
    fn command_criterion_needs_command() {
        let text = VALID.replace("command = \"cargo test\"\n", "");
        let Err(ManifestError::Invalid(errors)) = ExerciseManifest::from_toml(&text) else {
            panic!("expected validation errors");
        };
        assert!(errors.iter().any(|e| e.contains("needs a `command`")));
    }

    #[test]
    fn rejects_invalid_help_level() {
        let text = VALID.replace("max_level = 3", "max_level = 7");
        assert!(ExerciseManifest::from_toml(&text).is_err());
    }
}

#[cfg(test)]
mod example_manifests {
    use super::ExerciseManifest;

    #[test]
    fn bundled_examples_are_valid() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../exercises/examples");
        let mut checked = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "toml") {
                let text = std::fs::read_to_string(&path).unwrap();
                if let Err(e) = ExerciseManifest::from_toml(&text) {
                    panic!("{}: {e}", path.display());
                }
                checked += 1;
            }
        }
        assert!(checked > 0, "no example manifests found");
    }
}
