//! Explicit tool permissions for the AI Mentor (spec 4.4).

use std::collections::BTreeSet;
use std::path::{Component, Path};

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};

/// Every capability the mentor might use. Anything not listed in
/// [`PolicyConfig::allowed_tools`] is denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    ReadFile,
    SearchRepo,
    ReadDocs,
    Terminal,
    WriteFile,
    ExecCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub allowed_tools: BTreeSet<Tool>,
    /// Globs (relative to the workspace root) the mentor may never touch.
    pub denied_paths: Vec<String>,
    /// Maximum bytes of file content per request forwarded to a model.
    pub max_context_bytes: usize,
}

impl Default for PolicyConfig {
    /// Read-only mentor: may read and search code and docs, may not write,
    /// open a terminal or execute commands.
    fn default() -> Self {
        Self {
            allowed_tools: [Tool::ReadFile, Tool::SearchRepo, Tool::ReadDocs].into(),
            denied_paths: vec![
                ".git/**".into(),
                "**/.env".into(),
                "**/.env.*".into(),
                "**/*.pem".into(),
                "**/*.key".into(),
                "**/id_rsa*".into(),
                "**/id_ed25519*".into(),
                "**/.netrc".into(),
                "**/credentials*".into(),
            ],
            max_context_bytes: 32 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolRequest<'a> {
    pub tool: Tool,
    /// Target path relative to the workspace root, for file tools.
    pub path: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Deny { reason: String },
}

impl Decision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Decision::Allow)
    }

    fn deny(reason: impl Into<String>) -> Self {
        Decision::Deny {
            reason: reason.into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid path glob `{glob}`: {source}")]
pub struct GlobError {
    glob: String,
    #[source]
    source: globset::Error,
}

#[derive(Debug, Clone)]
pub struct PolicyEngine {
    config: PolicyConfig,
    denied: GlobSet,
}

impl PolicyEngine {
    pub fn new(config: PolicyConfig) -> Result<Self, GlobError> {
        let mut builder = GlobSetBuilder::new();
        for g in &config.denied_paths {
            let glob = Glob::new(g).map_err(|source| GlobError {
                glob: g.clone(),
                source,
            })?;
            builder.add(glob);
        }
        let denied = builder.build().map_err(|source| GlobError {
            glob: "<set>".into(),
            source,
        })?;
        Ok(Self { config, denied })
    }

    /// Adds exercise-specific denied paths (e.g. hidden reference tests).
    pub fn with_extra_denied(
        mut config: PolicyConfig,
        extra: &[String],
    ) -> Result<Self, GlobError> {
        config.denied_paths.extend(extra.iter().cloned());
        Self::new(config)
    }

    pub fn config(&self) -> &PolicyConfig {
        &self.config
    }

    pub fn check(&self, req: &ToolRequest<'_>) -> Decision {
        if !self.config.allowed_tools.contains(&req.tool) {
            return Decision::deny(format!(
                "tool `{:?}` is not permitted for the mentor",
                req.tool
            ));
        }
        let needs_path = matches!(req.tool, Tool::ReadFile | Tool::WriteFile);
        match (needs_path, req.path) {
            (true, None) => Decision::deny("file tools require a path"),
            (_, Some(path)) => self.check_path(path),
            (false, None) => Decision::Allow,
        }
    }

    fn check_path(&self, path: &str) -> Decision {
        let p = Path::new(path);
        if path.is_empty() || p.is_absolute() {
            return Decision::deny("paths must be relative to the workspace root");
        }
        if p.components()
            .any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)))
        {
            return Decision::deny("paths must not leave the workspace");
        }
        let normalized: std::path::PathBuf = p
            .components()
            .filter(|c| !matches!(c, Component::CurDir))
            .collect();
        if self.denied.is_match(&normalized) {
            return Decision::deny(format!("`{path}` is outside the mentor's permitted files"));
        }
        Decision::Allow
    }

    /// Truncates content to the configured context budget on a UTF-8
    /// boundary. Returns the clipped text and whether it was truncated.
    pub fn clip_context<'t>(&self, text: &'t str) -> (&'t str, bool) {
        let max = self.config.max_context_bytes;
        if text.len() <= max {
            return (text, false);
        }
        let mut end = max;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        (&text[..end], true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> PolicyEngine {
        PolicyEngine::with_extra_denied(PolicyConfig::default(), &["tests/hidden/**".into()])
            .unwrap()
    }

    fn read(path: &str) -> Decision {
        engine().check(&ToolRequest {
            tool: Tool::ReadFile,
            path: Some(path),
        })
    }

    #[test]
    fn default_is_read_only() {
        let e = engine();
        assert!(
            e.check(&ToolRequest {
                tool: Tool::SearchRepo,
                path: None
            })
            .is_allowed()
        );
        for tool in [Tool::WriteFile, Tool::Terminal, Tool::ExecCommand] {
            assert!(
                !e.check(&ToolRequest {
                    tool,
                    path: Some("src/lib.rs")
                })
                .is_allowed()
            );
        }
    }

    #[test]
    fn path_rules() {
        assert!(read("src/lib.rs").is_allowed());
        assert!(read("./src/lib.rs").is_allowed());
        assert!(!read("/etc/passwd").is_allowed());
        assert!(!read("../other-user/secret").is_allowed());
        assert!(!read("src/../../x").is_allowed());
        assert!(!read(".git/config").is_allowed());
        assert!(!read("config/.env").is_allowed());
        assert!(!read(".env.local").is_allowed());
        assert!(!read("tests/hidden/reference.rs").is_allowed());
        assert!(!read("./tests/hidden/reference.rs").is_allowed());
        assert!(read("tests/visible.rs").is_allowed());
        assert!(
            !engine()
                .check(&ToolRequest {
                    tool: Tool::ReadFile,
                    path: None
                })
                .is_allowed()
        );
    }

    #[test]
    fn clips_on_char_boundary() {
        let e = PolicyEngine::new(PolicyConfig {
            max_context_bytes: 5,
            ..Default::default()
        })
        .unwrap();
        let (clipped, truncated) = e.clip_context("абвгд");
        assert!(truncated);
        assert_eq!(clipped, "аб");
        assert_eq!(e.clip_context("abc"), ("abc", false));
    }
}
