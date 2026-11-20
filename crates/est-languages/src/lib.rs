//! Language adapters (architecture v0.2, §6).
//!
//! An adapter recognises a language in a project and *describes* the
//! commands to build, test and check it. It never runs anything: command
//! descriptions go to the Sandbox Manager. Language-specific knowledge lives
//! only here, so the rest of EST stays language independent.
//!
//! Adapters work on the list of tracked files (from Git) rather than walking
//! the directory, so build output and ignored files never skew detection.

use std::path::Path;

use est_domain::{CommandKind, CommandSpec, LanguageDetection};

pub mod adapters;
mod summary;

pub use summary::summarize_tests;

pub trait LanguageAdapter: Send + Sync {
    fn id(&self) -> &'static str;

    /// `None` when the language is absent from the project.
    fn detect(&self, root: &Path, files: &[String]) -> Option<LanguageDetection>;

    fn discover_tests(&self, root: &Path, files: &[String]) -> Vec<CommandSpec>;

    fn discover_checks(&self, root: &Path, files: &[String]) -> Vec<CommandSpec>;

    fn build_commands(&self, root: &Path, files: &[String]) -> Vec<CommandSpec>;
}

/// All known adapters.
pub struct LanguageRegistry {
    adapters: Vec<Box<dyn LanguageAdapter>>,
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self {
            adapters: vec![
                Box::new(adapters::Rust),
                Box::new(adapters::C),
                Box::new(adapters::Cpp),
                Box::new(adapters::Python),
                Box::new(adapters::JavaScript),
            ],
        }
    }
}

impl LanguageRegistry {
    pub fn adapters(&self) -> impl Iterator<Item = &dyn LanguageAdapter> {
        self.adapters.iter().map(|a| a.as_ref())
    }

    /// Languages present in the project, most files first.
    pub fn detect(&self, root: &Path, files: &[String]) -> Vec<LanguageDetection> {
        let mut found: Vec<_> = self
            .adapters()
            .filter_map(|a| a.detect(root, files))
            .collect();
        found.sort_by(|a, b| {
            b.files
                .cmp(&a.files)
                .then_with(|| a.language.cmp(&b.language))
        });
        found
    }

    /// Build, test and check commands of every detected language.
    pub fn commands(&self, root: &Path, files: &[String]) -> Vec<CommandSpec> {
        let mut out = Vec::new();
        for a in self.adapters() {
            if a.detect(root, files).is_some() {
                out.extend(a.build_commands(root, files));
                out.extend(a.discover_tests(root, files));
                out.extend(a.discover_checks(root, files));
            }
        }
        out
    }
}

// ---- helpers shared by adapters --------------------------------------------

pub(crate) fn ext(path: &str) -> Option<&str> {
    Path::new(path).extension().and_then(|e| e.to_str())
}

pub(crate) fn count_ext(files: &[String], exts: &[&str]) -> u32 {
    files
        .iter()
        .filter(|f| ext(f).is_some_and(|e| exts.contains(&e)))
        .count() as u32
}

pub(crate) fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Directory of the shallowest tracked file named `name` (`""` = root).
pub(crate) fn shallowest(files: &[String], name: &str) -> Option<String> {
    files
        .iter()
        .filter(|f| file_name(f) == name)
        .min_by_key(|f| f.matches('/').count())
        .map(|f| {
            f.rsplit_once('/')
                .map(|(d, _)| d.to_owned())
                .unwrap_or_default()
        })
}

pub(crate) fn cmd(
    kind: CommandKind,
    language: &str,
    workdir: &str,
    program: &str,
    args: &[&str],
    description: &str,
) -> CommandSpec {
    CommandSpec {
        kind,
        language: language.to_owned(),
        program: program.to_owned(),
        args: args.iter().map(|s| (*s).to_owned()).collect(),
        workdir: workdir.to_owned(),
        description: description.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn detects_multiple_languages() {
        let dir = tempfile::tempdir().unwrap();
        let f = files(&[
            "Cargo.toml",
            "src/main.rs",
            "src/lib.rs",
            "native/CMakeLists.txt",
            "native/a.cpp",
            "scripts/gen.py",
            "README.md",
        ]);
        let langs = LanguageRegistry::default().detect(dir.path(), &f);
        let ids: Vec<_> = langs.iter().map(|l| l.language.as_str()).collect();
        assert_eq!(ids, vec!["rust", "cpp", "python"]);
        assert_eq!(langs[0].build_systems, vec!["cargo"]);
        assert_eq!(langs[1].build_systems, vec!["cmake"]);
    }

    #[test]
    fn shallowest_manifest() {
        let f = files(&["crates/a/Cargo.toml", "Cargo.toml", "x/y/Cargo.toml"]);
        assert_eq!(shallowest(&f, "Cargo.toml").as_deref(), Some(""));
        let f = files(&["crates/a/Cargo.toml"]);
        assert_eq!(shallowest(&f, "Cargo.toml").as_deref(), Some("crates/a"));
    }
}
