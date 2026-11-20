//! Workspace Manager (architecture v0.2, §7).
//!
//! EST has no editor of its own. Every task gets a separate working
//! directory, which the learner opens in their usual IDE:
//!
//! ```text
//! ~/.est/
//! ├── config/      config.toml, providers.toml
//! ├── secrets/     API keys entered in the UI (0700, files 0600)
//! ├── database/    est.db
//! ├── cache/       clones of remote projects
//! ├── models/      local LLM weights (AI Gateway)
//! ├── learning/    learning artifacts
//! └── workspaces/
//!     └── <project>/
//!         └── <task>/
//! ```

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use est_application::ports::{IdeLauncher, WorkspaceLayout};
use est_application::{AppError, AppResult};
use est_core::slugify;
use est_domain::{Project, Task};

/// The `~/.est` directory tree.
#[derive(Debug, Clone)]
pub struct EstHome {
    root: PathBuf,
}

impl EstHome {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `$EST_HOME`, else `~/.est`.
    pub fn from_env() -> Self {
        if let Some(p) = std::env::var_os("EST_HOME") {
            return Self::new(p);
        }
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        Self::new(home.join(".est"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn config_dir(&self) -> PathBuf {
        self.root.join("config")
    }
    pub fn config_file(&self) -> PathBuf {
        self.config_dir().join("config.toml")
    }
    pub fn database_dir(&self) -> PathBuf {
        self.root.join("database")
    }
    pub fn database_file(&self) -> PathBuf {
        self.database_dir().join("est.db")
    }
    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }
    pub fn models_dir(&self) -> PathBuf {
        self.root.join("models")
    }
    pub fn learning_dir(&self) -> PathBuf {
        self.root.join("learning")
    }
    pub fn workspaces_dir(&self) -> PathBuf {
        self.root.join("workspaces")
    }
    /// API keys the learner entered in the UI. Never part of config,
    /// events, logs or exports.
    pub fn secrets_dir(&self) -> PathBuf {
        self.root.join("secrets")
    }
    pub fn key_store(&self) -> KeyStore {
        KeyStore {
            dir: self.secrets_dir().join("llm"),
        }
    }

    /// Creates the directory tree. Idempotent.
    pub fn init(&self) -> std::io::Result<()> {
        for d in [
            self.config_dir(),
            self.database_dir(),
            self.cache_dir().join("projects"),
            self.models_dir(),
            self.learning_dir(),
            self.workspaces_dir(),
        ] {
            std::fs::create_dir_all(d)?;
        }
        Ok(())
    }
}

/// API keys of AI providers, one file per provider, readable only by the
/// owner. The keys are write-only from the API's point of view: the UI can
/// set or remove a key and learn whether one exists, never read it back.
#[derive(Debug, Clone)]
pub struct KeyStore {
    dir: PathBuf,
}

impl KeyStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path(&self, id: &str) -> AppResult<PathBuf> {
        if !est_core::is_slug(id) {
            return Err(AppError::Invalid(format!("invalid key id {id:?}")));
        }
        Ok(self.dir.join(format!("{id}.key")))
    }

    pub fn get(&self, id: &str) -> AppResult<Option<String>> {
        match std::fs::read_to_string(self.path(id)?) {
            Ok(k) => Ok(Some(k.trim().to_owned()).filter(|k| !k.is_empty())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(AppError::Infrastructure(format!("key store: {e}"))),
        }
    }

    pub fn has(&self, id: &str) -> bool {
        self.get(id).ok().flatten().is_some()
    }

    pub fn set(&self, id: &str, key: &str) -> AppResult<()> {
        let key = key.trim();
        if key.is_empty() || key.len() > 4096 || key.chars().any(char::is_control) {
            return Err(AppError::Invalid(
                "the API key is empty or malformed".into(),
            ));
        }
        let path = self.path(id)?;
        let io = |e: std::io::Error| AppError::Infrastructure(format!("key store: {e}"));
        create_private_dir(&self.dir).map_err(io)?;
        let tmp = path.with_extension("key.tmp");
        write_private(&tmp, key).map_err(io)?;
        std::fs::rename(&tmp, &path).map_err(io)
    }

    pub fn remove(&self, id: &str) -> AppResult<()> {
        match std::fs::remove_file(self.path(id)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(AppError::Infrastructure(format!("key store: {e}"))),
        }
    }
}

#[cfg(unix)]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

#[cfg(unix)]
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()
}

#[cfg(not(unix))]
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, text)
}

fn label(name: &str, id: &str) -> String {
    let slug = slugify(name, 40);
    let short = &id[..id.len().min(8)];
    if slug.is_empty() {
        short.to_owned()
    } else {
        format!("{slug}-{short}")
    }
}

impl WorkspaceLayout for EstHome {
    fn workspace_for(&self, project: &Project, task: &Task) -> PathBuf {
        self.workspaces_dir()
            .join(label(&project.name, project.id.as_str()))
            .join(label(&task.title, task.id.as_str()))
    }

    fn project_cache(&self, project_name: &str) -> PathBuf {
        self.cache_dir()
            .join("projects")
            .join(slugify(project_name, 60))
    }

    fn practice_dir(&self, name: &str) -> PathBuf {
        self.learning_dir().join("practice").join(slugify(name, 60))
    }
}

/// Opens workspaces in an external editor.
#[derive(Debug, Clone, Default)]
pub struct EditorLauncher {
    /// Explicit command, e.g. `code` or `codium --new-window`. When `None`,
    /// `$EST_EDITOR` and then a list of common GUI editors are tried.
    pub command: Option<String>,
}

const KNOWN_EDITORS: &[&str] = &["code", "codium", "zed", "clion", "idea", "subl"];

fn in_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| {
            let p = dir.join(program);
            p.is_file() || p.with_extension("exe").is_file() || p.with_extension("cmd").is_file()
        })
    })
}

impl EditorLauncher {
    fn resolve(&self) -> Option<Vec<String>> {
        let configured = self
            .command
            .clone()
            .or_else(|| std::env::var("EST_EDITOR").ok())
            .filter(|c| !c.trim().is_empty());
        if let Some(c) = configured {
            return Some(c.split_whitespace().map(str::to_owned).collect());
        }
        KNOWN_EDITORS
            .iter()
            .find(|e| in_path(e))
            .map(|e| vec![(*e).to_owned()])
    }
}

impl IdeLauncher for EditorLauncher {
    fn open(&self, path: &Path) -> AppResult<String> {
        let argv = self.resolve().ok_or_else(|| {
            AppError::Invalid(format!(
                "no editor configured: set `editor` in config.toml or EST_EDITOR, \
                 or open {} yourself",
                path.display()
            ))
        })?;
        let mut child = Command::new(&argv[0])
            .args(&argv[1..])
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| AppError::Infrastructure(format!("cannot start `{}`: {e}", argv[0])))?;
        // Reap the editor launcher when it exits so it does not linger as a zombie.
        std::thread::spawn(move || child.wait());
        Ok(format!("{} {}", argv.join(" "), path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_store_is_private_and_write_only() {
        let dir = tempfile::tempdir().unwrap();
        let store = EstHome::new(dir.path()).key_store();
        assert_eq!(store.get("claude-1").unwrap(), None);
        assert!(store.set("../x", "k").is_err());
        assert!(store.set("claude-1", "  ").is_err());
        store.set("claude-1", " sk-test \n").unwrap();
        assert_eq!(store.get("claude-1").unwrap().as_deref(), Some("sk-test"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let p = dir.path().join("secrets/llm/claude-1.key");
            assert_eq!(
                std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        store.remove("claude-1").unwrap();
        assert!(!store.has("claude-1"));
    }

    #[test]
    fn layout() {
        let home = EstHome::new("/home/u/.est");
        assert_eq!(
            home.database_file(),
            PathBuf::from("/home/u/.est/database/est.db")
        );
        assert_eq!(
            home.project_cache("ripgrep-1a2b3c4d"),
            PathBuf::from("/home/u/.est/cache/projects/ripgrep-1a2b3c4d")
        );
        assert_eq!(label("Fix: parser", "0123456789"), "fix-parser-01234567");
        assert_eq!(label("Задача", "0123456789"), "01234567");
    }

    #[test]
    fn init_creates_tree() {
        let dir = tempfile::tempdir().unwrap();
        let home = EstHome::new(dir.path().join(".est"));
        home.init().unwrap();
        home.init().unwrap();
        assert!(home.workspaces_dir().is_dir() && home.database_dir().is_dir());
    }

    #[test]
    fn launches_configured_editor() {
        let dir = tempfile::tempdir().unwrap();
        let ok = EditorLauncher {
            command: Some("true --flag".into()),
        }
        .open(dir.path())
        .unwrap();
        assert!(ok.starts_with("true --flag "));
        let err = EditorLauncher {
            command: Some("definitely-not-an-editor-xyz".into()),
        }
        .open(dir.path());
        assert!(err.is_err());
    }
}
