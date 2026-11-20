//! [`est_application::ports::GitPort`] implemented with the git CLI.

use std::path::Path;

use est_application::ports::{CommitSummary, DiffTarget, FileStatus, GitPort, SourceMatch};
use est_application::{AppError, AppResult};

use crate::{GitError, Repo, UrlPolicy};

/// Git adapter used by the application.
#[derive(Debug, Clone, Default)]
pub struct CliGit {
    /// Applies to remote clones only; workspaces are always cloned from
    /// repositories EST already knows.
    pub remote_policy: UrlPolicy,
}

fn map(e: GitError) -> AppError {
    match e {
        GitError::UrlNotAllowed(_) | GitError::InvalidArgument(_) => {
            AppError::Invalid(e.to_string())
        }
        other => AppError::Infrastructure(other.to_string()),
    }
}

impl GitPort for CliGit {
    fn is_repository(&self, path: &Path) -> bool {
        Repo::is_repository(path)
    }

    fn clone_remote(&self, url: &str, dest: &Path) -> AppResult<()> {
        Repo::clone(url, dest, None, self.remote_policy)
            .map(|_| ())
            .map_err(map)
    }

    fn clone_local(&self, source: &Path, dest: &Path) -> AppResult<()> {
        let src = source
            .to_str()
            .ok_or_else(|| AppError::Invalid("repository path is not UTF-8".into()))?;
        Repo::clone(src, dest, None, UrlPolicy { allow_local: true })
            .map(|_| ())
            .map_err(map)
    }

    fn remote_url(&self, repo: &Path) -> AppResult<Option<String>> {
        Repo::open(repo).remote_url().map_err(map)
    }

    fn resolve(&self, repo: &Path, rev: &str) -> AppResult<Option<String>> {
        match Repo::open(repo).resolve(rev) {
            Ok(hash) if !hash.is_empty() => Ok(Some(hash)),
            Ok(_) | Err(GitError::Failed { .. }) => Ok(None),
            Err(e) => Err(map(e)),
        }
    }

    fn fetch(&self, repo: &Path) -> AppResult<()> {
        Repo::open(repo).fetch("origin").map_err(map)
    }

    fn create_branch(&self, repo: &Path, name: &str, start: &str) -> AppResult<()> {
        Repo::open(repo)
            .create_branch(name, Some(start))
            .map_err(map)
    }

    fn current_branch(&self, repo: &Path) -> AppResult<Option<String>> {
        Repo::open(repo).current_branch().map_err(map)
    }

    fn status(&self, repo: &Path) -> AppResult<Vec<FileStatus>> {
        Ok(Repo::open(repo)
            .status()
            .map_err(map)?
            .into_iter()
            .map(|e| FileStatus {
                code: e.code,
                path: e.path,
            })
            .collect())
    }

    fn diff(&self, repo: &Path, target: DiffTarget<'_>) -> AppResult<String> {
        let r = Repo::open(repo);
        match target {
            DiffTarget::Worktree => r.diff(false),
            DiffTarget::Staged => r.diff(true),
            DiffTarget::Since(rev) => r.diff_against(rev),
        }
        .map_err(map)
    }

    fn log(&self, repo: &Path, since: Option<&str>, limit: usize) -> AppResult<Vec<CommitSummary>> {
        Ok(Repo::open(repo)
            .log(since, limit)
            .map_err(map)?
            .into_iter()
            .map(|c| CommitSummary {
                hash: c.hash,
                author: c.author_name,
                date: c.date,
                subject: c.subject,
            })
            .collect())
    }

    fn tracked_files(&self, repo: &Path) -> AppResult<Vec<String>> {
        Repo::open(repo).ls_files().map_err(map)
    }

    fn grep(&self, repo: &Path, needle: &str, max: usize) -> AppResult<Vec<SourceMatch>> {
        Ok(Repo::open(repo)
            .grep(needle, max)
            .map_err(map)?
            .into_iter()
            .map(|m| SourceMatch {
                path: m.path,
                line: m.line,
                text: m.text,
            })
            .collect())
    }
}
