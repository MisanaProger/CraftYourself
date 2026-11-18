//! Git Engine (spec 3.5).
//!
//! A thin, synchronous wrapper over the `git` command line. Every invocation:
//!
//! * runs with an explicit working directory and never through a shell,
//! * disables repository hooks (`core.hooksPath=/dev/null`) because cloned
//!   open-source repositories are untrusted,
//! * disables interactive credential prompts,
//! * ignores the user's global/system git configuration so that behaviour is
//!   reproducible, while still allowing an explicit author identity.
//!
//! Callers in async code should run these methods inside
//! `tokio::task::spawn_blocking`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::Serialize;
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("failed to start git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("git {command} failed (exit code {code:?}): {stderr}")]
    Failed {
        command: String,
        code: Option<i32>,
        stderr: String,
    },
    #[error("repository URL not allowed: {0}")]
    UrlNotAllowed(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("git produced unexpected output: {0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, GitError>;

/// Which repository URLs may be cloned.
#[derive(Debug, Clone, Copy, Default)]
pub struct UrlPolicy {
    /// `file://` URLs and plain local paths. Off by default for the server,
    /// on for tests and for explicitly local workflows.
    pub allow_local: bool,
}

impl UrlPolicy {
    pub fn check(&self, url: &str) -> Result<()> {
        if url.starts_with('-') {
            return Err(GitError::UrlNotAllowed(url.into()));
        }
        // scp-like ssh syntax: git@github.com:owner/repo.git
        if !url.contains("://") {
            if let Some((user_host, _path)) = url.split_once(':') {
                if user_host.contains('@') && !user_host.contains('/') {
                    return Ok(());
                }
            }
            let local_path =
                url.starts_with('/') || url.starts_with("./") || url.starts_with("../");
            return if self.allow_local && local_path && !url.contains("::") {
                Ok(())
            } else {
                Err(GitError::UrlNotAllowed(url.into()))
            };
        }
        let parsed = Url::parse(url).map_err(|_| GitError::UrlNotAllowed(url.into()))?;
        match parsed.scheme() {
            "https" | "ssh" => Ok(()),
            "file" if self.allow_local => Ok(()),
            _ => Err(GitError::UrlNotAllowed(url.into())),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitInfo {
    pub hash: String,
    pub author_name: String,
    pub author_email: String,
    /// ISO-8601 author date.
    pub date: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StatusEntry {
    /// Two-letter porcelain status code, e.g. ` M`, `A `, `??`.
    pub code: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GrepMatch {
    pub path: String,
    pub line: u32,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub name: String,
    pub email: String,
}

/// A handle to a local working tree.
#[derive(Debug, Clone)]
pub struct Repo {
    root: PathBuf,
}

fn base_command() -> Command {
    let mut cmd = Command::new("git");
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("LC_ALL", "C")
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(["-c", "core.fsmonitor=false"])
        .args(["-c", "protocol.ext.allow=never"])
        .args(["-c", "init.defaultBranch=main"]);
    cmd
}

fn run(mut cmd: Command, label: &str) -> Result<Output> {
    let output = cmd.output()?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(GitError::Failed {
            command: label.to_owned(),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}

fn validate_ref(name: &str) -> Result<()> {
    let bad = name.is_empty()
        || name.starts_with('-')
        || name.contains("..")
        || name
            .chars()
            .any(|c| c.is_control() || " ~^:?*[\\".contains(c));
    if bad {
        Err(GitError::InvalidArgument(format!(
            "invalid ref name `{name}`"
        )))
    } else {
        Ok(())
    }
}

fn validate_rel_path(path: &str) -> Result<()> {
    let p = Path::new(path);
    if path.is_empty()
        || p.is_absolute()
        || p.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        Err(GitError::InvalidArgument(format!(
            "path `{path}` must be relative to the repository root"
        )))
    } else {
        Ok(())
    }
}

impl Repo {
    /// Opens an existing working tree (does not verify it is a repository).
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Initialises a new empty repository. Mostly useful for tests and for
    /// learners creating projects from scratch.
    pub fn init(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        let mut cmd = base_command();
        cmd.arg("init").arg("--quiet").arg(&root);
        run(cmd, "init")?;
        Ok(Self { root })
    }

    /// Clones `url` into `dest`. `dest` must not exist or be empty.
    pub fn clone(url: &str, dest: &Path, branch: Option<&str>, policy: UrlPolicy) -> Result<Self> {
        policy.check(url)?;
        let mut cmd = base_command();
        if policy.allow_local {
            cmd.args(["-c", "protocol.file.allow=always"]);
        } else {
            cmd.args(["-c", "protocol.file.allow=never"]);
        }
        cmd.args(["clone", "--quiet", "--no-recurse-submodules"]);
        if let Some(b) = branch {
            validate_ref(b)?;
            cmd.args(["--branch", b]);
        }
        cmd.arg("--").arg(url).arg(dest);
        run(cmd, "clone")?;
        Ok(Self {
            root: dest.to_owned(),
        })
    }

    fn git<I, S>(&self, args: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = base_command();
        cmd.arg("-C").arg(&self.root).args(args);
        cmd
    }

    fn stdout(&self, args: &[&str], label: &str) -> Result<String> {
        let out = run(self.git(args), label)?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    pub fn fetch(&self, remote: &str) -> Result<()> {
        validate_ref(remote)?;
        run(
            self.git(["fetch", "--quiet", "--no-recurse-submodules", remote]),
            "fetch",
        )?;
        Ok(())
    }

    /// Checks out a branch, tag or commit.
    pub fn checkout(&self, rev: &str) -> Result<()> {
        validate_ref(rev)?;
        run(self.git(["checkout", "--quiet", rev, "--"]), "checkout")?;
        Ok(())
    }

    /// Creates `name` at `start` (or HEAD) and switches to it.
    pub fn create_branch(&self, name: &str, start: Option<&str>) -> Result<()> {
        validate_ref(name)?;
        let mut args = vec!["switch", "--quiet", "-c", name];
        if let Some(s) = start {
            validate_ref(s)?;
            args.push(s);
        }
        run(self.git(args), "switch -c")?;
        Ok(())
    }

    pub fn current_branch(&self) -> Result<Option<String>> {
        let out = self.stdout(&["branch", "--show-current"], "branch --show-current")?;
        let name = out.trim();
        Ok((!name.is_empty()).then(|| name.to_owned()))
    }

    pub fn branches(&self) -> Result<Vec<String>> {
        let out = self.stdout(&["branch", "--format=%(refname:short)"], "branch")?;
        Ok(out.lines().map(str::to_owned).collect())
    }

    pub fn head_commit(&self) -> Result<String> {
        Ok(self
            .stdout(&["rev-parse", "HEAD"], "rev-parse")?
            .trim()
            .to_owned())
    }

    /// Resolves any revision to a full commit hash.
    pub fn resolve(&self, rev: &str) -> Result<String> {
        validate_ref(rev)?;
        let spec = format!("{rev}^{{commit}}");
        Ok(self
            .stdout(&["rev-parse", "--verify", "--quiet", &spec], "rev-parse")?
            .trim()
            .to_owned())
    }

    pub fn add(&self, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Err(GitError::InvalidArgument("no paths to add".into()));
        }
        for p in paths {
            validate_rel_path(p)?;
        }
        let mut cmd = self.git(["add", "--"]);
        cmd.args(paths);
        run(cmd, "add")?;
        Ok(())
    }

    /// Creates a commit from the index. Returns the new commit hash.
    pub fn commit(&self, message: &str, author: &Identity) -> Result<String> {
        if message.trim().is_empty() {
            return Err(GitError::InvalidArgument(
                "commit message must not be empty".into(),
            ));
        }
        let mut cmd = self.git(["commit", "--quiet", "--no-verify", "--file=-"]);
        cmd.env("GIT_AUTHOR_NAME", &author.name)
            .env("GIT_AUTHOR_EMAIL", &author.email)
            .env("GIT_COMMITTER_NAME", &author.name)
            .env("GIT_COMMITTER_EMAIL", &author.email)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = cmd.spawn()?;
        {
            use std::io::Write;
            let mut stdin = child.stdin.take().expect("stdin is piped");
            stdin.write_all(message.as_bytes())?;
        }
        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(GitError::Failed {
                command: "commit".into(),
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        self.head_commit()
    }

    /// Unified diff of the working tree against the index, or of the index
    /// against HEAD when `staged` is set.
    pub fn diff(&self, staged: bool) -> Result<String> {
        let mut args = vec!["diff", "--no-color", "--no-ext-diff"];
        if staged {
            args.push("--cached");
        }
        self.stdout(&args, "diff")
    }

    /// Diff between two revisions (e.g. pinned exercise base and HEAD).
    pub fn diff_range(&self, from: &str, to: &str) -> Result<String> {
        validate_ref(from)?;
        validate_ref(to)?;
        let range = format!("{from}..{to}");
        self.stdout(&["diff", "--no-color", "--no-ext-diff", &range], "diff")
    }

    pub fn log(&self, limit: usize) -> Result<Vec<CommitInfo>> {
        let n = format!("-n{}", limit.max(1));
        let out = self.stdout(
            &["log", &n, "--format=%H%x1f%an%x1f%ae%x1f%aI%x1f%s%x1e"],
            "log",
        )?;
        out.split('\x1e')
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .map(|record| {
                let f: Vec<&str> = record.split('\x1f').collect();
                if f.len() != 5 {
                    return Err(GitError::Parse(record.to_owned()));
                }
                Ok(CommitInfo {
                    hash: f[0].into(),
                    author_name: f[1].into(),
                    author_email: f[2].into(),
                    date: f[3].into(),
                    subject: f[4].into(),
                })
            })
            .collect()
    }

    pub fn status(&self) -> Result<Vec<StatusEntry>> {
        let out = run(
            self.git(["status", "--porcelain=v1", "-z", "--untracked-files=all"]),
            "status",
        )?;
        let text = String::from_utf8_lossy(&out.stdout);
        let mut entries = Vec::new();
        let mut parts = text.split('\0').filter(|s| !s.is_empty());
        while let Some(rec) = parts.next() {
            if rec.len() < 4 {
                return Err(GitError::Parse(rec.to_owned()));
            }
            let code = rec[..2].to_owned();
            let path = rec[3..].to_owned();
            // Renames/copies are followed by the original path.
            if code.starts_with('R') || code.starts_with('C') {
                parts.next();
            }
            entries.push(StatusEntry { code, path });
        }
        Ok(entries)
    }

    /// Lists tracked files (project structure overview).
    pub fn ls_files(&self) -> Result<Vec<String>> {
        let out = run(self.git(["ls-files", "-z"]), "ls-files")?;
        Ok(String::from_utf8_lossy(&out.stdout)
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect())
    }

    /// Fixed-string search over tracked files. Used by the Documentation
    /// Router to point learners at the definition of a symbol.
    pub fn grep(&self, needle: &str, max_results: usize) -> Result<Vec<GrepMatch>> {
        if needle.is_empty() {
            return Err(GitError::InvalidArgument("empty search string".into()));
        }
        let output = self
            .git(["grep", "-n", "-I", "--no-color", "-F", "-e", needle])
            .output()?;
        // Exit code 1 means "no matches".
        match output.status.code() {
            Some(0) => {}
            Some(1) => return Ok(Vec::new()),
            code => {
                return Err(GitError::Failed {
                    command: "grep".into(),
                    code,
                    stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                })
            }
        }
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text
            .lines()
            .filter_map(|l| {
                let (path, rest) = l.split_once(':')?;
                let (line, text) = rest.split_once(':')?;
                Some(GrepMatch {
                    path: path.to_owned(),
                    line: line.parse().ok()?,
                    text: text.trim().to_owned(),
                })
            })
            .take(max_results)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn me() -> Identity {
        Identity {
            name: "Learner".into(),
            email: "learner@example.invalid".into(),
        }
    }

    fn repo_with_commit() -> (tempfile::TempDir, Repo) {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repo::init(dir.path().join("origin")).unwrap();
        std::fs::write(repo.root().join("lib.rs"), "pub fn parse_range() {}\n").unwrap();
        repo.add(&["lib.rs".into()]).unwrap();
        repo.commit("Initial commit", &me()).unwrap();
        (dir, repo)
    }

    #[test]
    fn url_policy() {
        let strict = UrlPolicy::default();
        assert!(strict.check("https://github.com/a/b").is_ok());
        assert!(strict.check("git@github.com:a/b.git").is_ok());
        assert!(strict.check("ssh://git@github.com/a/b").is_ok());
        assert!(strict.check("file:///tmp/x").is_err());
        assert!(strict.check("/tmp/x").is_err());
        assert!(strict.check("ext::sh -c touch% /tmp/pwned").is_err());
        assert!(strict.check("--upload-pack=evil").is_err());
        assert!(strict.check("http://insecure.example/a").is_err());
        let local = UrlPolicy { allow_local: true };
        assert!(local.check("/tmp/x").is_ok());
        assert!(local.check("file:///tmp/x").is_ok());
        assert!(local.check("ext::sh -c id").is_err());
        assert!(local.check("relative-without-prefix").is_err());
    }

    #[test]
    fn rejects_option_like_refs_and_escaping_paths() {
        let (_d, repo) = repo_with_commit();
        assert!(matches!(
            repo.checkout("--orphan"),
            Err(GitError::InvalidArgument(_))
        ));
        assert!(matches!(
            repo.add(&["../x".into()]),
            Err(GitError::InvalidArgument(_))
        ));
        assert!(matches!(
            repo.add(&["/etc/passwd".into()]),
            Err(GitError::InvalidArgument(_))
        ));
    }

    #[test]
    fn full_workflow() {
        let (dir, origin) = repo_with_commit();
        let base = origin.head_commit().unwrap();
        assert_eq!(base.len(), 40);

        let work = Repo::clone(
            origin.root().to_str().unwrap(),
            &dir.path().join("work"),
            None,
            UrlPolicy { allow_local: true },
        )
        .unwrap();
        work.checkout(&base).unwrap();
        work.create_branch("est/fix-range", None).unwrap();
        assert_eq!(
            work.current_branch().unwrap().as_deref(),
            Some("est/fix-range")
        );

        std::fs::write(
            work.root().join("lib.rs"),
            "pub fn parse_range() { todo!() }\n",
        )
        .unwrap();
        std::fs::write(work.root().join("new.rs"), "// new\n").unwrap();
        let status = work.status().unwrap();
        assert!(status.contains(&StatusEntry {
            code: " M".into(),
            path: "lib.rs".into()
        }));
        assert!(status.contains(&StatusEntry {
            code: "??".into(),
            path: "new.rs".into()
        }));
        assert!(work
            .diff(false)
            .unwrap()
            .contains("+pub fn parse_range() { todo!() }"));

        work.add(&["lib.rs".into(), "new.rs".into()]).unwrap();
        assert!(work.diff(true).unwrap().contains("new.rs"));
        let head = work
            .commit("Make parse_range explicit\n\nBody explaining why.", &me())
            .unwrap();
        assert_eq!(work.resolve("HEAD").unwrap(), head);
        assert!(work.status().unwrap().is_empty());

        let log = work.log(10).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].subject, "Make parse_range explicit");
        assert_eq!(log[1].hash, base);

        assert!(work.diff_range(&base, "HEAD").unwrap().contains("new.rs"));
        assert_eq!(work.ls_files().unwrap(), vec!["lib.rs", "new.rs"]);

        let hits = work.grep("fn parse_range", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "lib.rs");
        assert_eq!(hits[0].line, 1);
        assert!(work.grep("does_not_exist", 10).unwrap().is_empty());
    }

    #[test]
    fn empty_commit_message_is_rejected() {
        let (_d, repo) = repo_with_commit();
        assert!(matches!(
            repo.commit("   ", &me()),
            Err(GitError::InvalidArgument(_))
        ));
    }
}
