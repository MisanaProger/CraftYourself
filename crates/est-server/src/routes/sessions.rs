//! Learning sessions: an isolated working copy of an exercise's project at
//! the pinned revision, on its own branch, plus Git operations on it.

use std::path::{Component, Path as FsPath, PathBuf};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use est_git::{CommitInfo, Identity, Repo, StatusEntry, UrlPolicy};
use est_storage::{new_id, Event, NewSession, Session, SessionStatus};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::exercises::parse_stored;
use super::projects::{clone_project, CreateProject};
use crate::error::{blocking, ApiError, ApiResult};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CreateSession {
    pub exercise_id: String,
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateSession>,
) -> ApiResult<(StatusCode, Json<Session>)> {
    let record = state.store.get_exercise(&req.exercise_id).await?;
    let manifest = parse_stored(&record)?;
    let source = manifest.source.clone();

    let project = match state.store.find_project_by_url(&source.repository).await? {
        Some(p) => p,
        None => {
            clone_project(
                &state,
                CreateProject {
                    url: source.repository.clone(),
                    branch: None,
                    description: None,
                },
            )
            .await?
            .project
        }
    };

    let session_id = new_id();
    let workspace = state.config.workspaces_dir().join(&session_id);
    let branch = format!("est/{}", manifest.exercise.id);
    let cache = PathBuf::from(&project.local_path);
    let revision = source.revision.clone();
    {
        let workspace = workspace.clone();
        let branch = branch.clone();
        blocking(move || {
            let cache = Repo::open(cache);
            if cache.resolve(&revision).is_err() {
                cache.fetch("origin")?;
                cache.resolve(&revision).map_err(|_| {
                    ApiError::Unprocessable(format!(
                        "pinned revision {revision} does not exist in the repository"
                    ))
                })?;
            }
            // The cache is ours, so a local clone is safe here.
            let result = Repo::clone(
                &cache.root().to_string_lossy(),
                &workspace,
                None,
                UrlPolicy { allow_local: true },
            )
            .and_then(|ws| ws.create_branch(&branch, Some(&revision)));
            if result.is_err() {
                let _ = std::fs::remove_dir_all(&workspace);
            }
            Ok(result?)
        })
        .await?;
    }

    let session = state
        .store
        .create_session(NewSession {
            id: &session_id,
            user_id: &state.user.id,
            exercise_id: &manifest.exercise.id,
            project_id: &project.id,
            workspace_path: &workspace.to_string_lossy(),
            branch: &branch,
            base_revision: &source.revision,
        })
        .await?;
    state
        .store
        .record_event(
            &session.id,
            "session.started",
            &json!({ "exercise_version": manifest.exercise.version }),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(session)))
}

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<Session>>> {
    Ok(Json(state.store.list_sessions(&state.user.id).await?))
}

/// Loads a session owned by the current user.
pub(crate) async fn owned_session(state: &AppState, id: &str) -> ApiResult<Session> {
    let s = state.store.get_session(id).await?;
    if s.user_id != state.user.id {
        return Err(ApiError::NotFound("session".into()));
    }
    Ok(s)
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Session>> {
    Ok(Json(owned_session(&state, &id).await?))
}

#[derive(Deserialize)]
pub struct StatusBody {
    pub status: String,
}

pub async fn set_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<StatusBody>,
) -> ApiResult<Json<Session>> {
    owned_session(&state, &id).await?;
    let status = match body.status.as_str() {
        "active" => SessionStatus::Active,
        "completed" => SessionStatus::Completed,
        "abandoned" => SessionStatus::Abandoned,
        other => return Err(ApiError::BadRequest(format!("unknown status `{other}`"))),
    };
    let s = state.store.set_session_status(&id, status).await?;
    state
        .store
        .record_event(&id, "session.status", &json!({ "status": body.status }))
        .await?;
    Ok(Json(s))
}

pub async fn events(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Event>>> {
    owned_session(&state, &id).await?;
    Ok(Json(state.store.list_events(&id).await?))
}

// ---- files ---------------------------------------------------------------

/// Resolves a learner-supplied path inside the workspace, refusing anything
/// that could escape it (absolute paths, `..`, symlinks) or touch `.git`.
fn resolve_in_workspace(root: &FsPath, rel: &str, for_write: bool) -> ApiResult<PathBuf> {
    let p = FsPath::new(rel);
    let ok = !rel.is_empty()
        && p.components().all(|c| match c {
            Component::Normal(s) => s != ".git",
            Component::CurDir => true,
            _ => false,
        });
    if !ok {
        return Err(ApiError::BadRequest(format!("path `{rel}` is not allowed")));
    }
    let root = root.canonicalize()?;
    let full = root.join(p);
    let check = |path: &FsPath| -> ApiResult<()> {
        let real = path.canonicalize()?;
        if real.starts_with(&root) {
            Ok(())
        } else {
            Err(ApiError::Forbidden(format!(
                "path `{rel}` leaves the workspace"
            )))
        }
    };
    if for_write {
        if let Ok(meta) = std::fs::symlink_metadata(&full) {
            if meta.file_type().is_symlink() {
                return Err(ApiError::Forbidden(format!("`{rel}` is a symlink")));
            }
        }
        let parent = full.parent().expect("joined path has a parent");
        std::fs::create_dir_all(parent)?;
        check(parent)?;
    } else {
        if !full.exists() {
            return Err(ApiError::NotFound(format!("file `{rel}`")));
        }
        check(&full)?;
    }
    Ok(full)
}

pub async fn list_files(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<String>>> {
    let s = owned_session(&state, &id).await?;
    Ok(Json(
        blocking(move || Ok(Repo::open(s.workspace_path).ls_files()?)).await?,
    ))
}

const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

pub async fn read_file(
    State(state): State<AppState>,
    Path((id, path)): Path<(String, String)>,
) -> ApiResult<String> {
    let s = owned_session(&state, &id).await?;
    blocking(move || {
        let full = resolve_in_workspace(FsPath::new(&s.workspace_path), &path, false)?;
        if !full.is_file() {
            return Err(ApiError::BadRequest(format!("`{path}` is not a file")));
        }
        if full.metadata()?.len() > MAX_FILE_BYTES {
            return Err(ApiError::Unprocessable(format!(
                "`{path}` is too large to open"
            )));
        }
        String::from_utf8(std::fs::read(full)?)
            .map_err(|_| ApiError::Unprocessable(format!("`{path}` is not UTF-8 text")))
    })
    .await
}

pub async fn write_file(
    State(state): State<AppState>,
    Path((id, path)): Path<(String, String)>,
    body: String,
) -> ApiResult<StatusCode> {
    let s = owned_session(&state, &id).await?;
    if body.len() as u64 > MAX_FILE_BYTES {
        return Err(ApiError::Unprocessable("file is too large".into()));
    }
    blocking(move || {
        let full = resolve_in_workspace(FsPath::new(&s.workspace_path), &path, true)?;
        std::fs::write(full, body)?;
        Ok(())
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- git -----------------------------------------------------------------

async fn with_repo<T, F>(state: &AppState, id: &str, f: F) -> ApiResult<T>
where
    F: FnOnce(Repo, Session) -> ApiResult<T> + Send + 'static,
    T: Send + 'static,
{
    let s = owned_session(state, id).await?;
    blocking(move || f(Repo::open(&s.workspace_path), s)).await
}

pub async fn git_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<StatusEntry>>> {
    Ok(Json(with_repo(&state, &id, |r, _| Ok(r.status()?)).await?))
}

#[derive(Deserialize)]
pub struct DiffQuery {
    /// `worktree` (default), `staged`, or `base` (everything since the
    /// exercise's pinned revision).
    #[serde(default)]
    pub against: Option<String>,
}

pub async fn git_diff(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<DiffQuery>,
) -> ApiResult<String> {
    with_repo(&state, &id, move |r, s| match q.against.as_deref() {
        None | Some("worktree") => Ok(r.diff(false)?),
        Some("staged") => Ok(r.diff(true)?),
        Some("base") => Ok(r.diff_range(&s.base_revision, "HEAD")?),
        Some(other) => Err(ApiError::BadRequest(format!(
            "unknown diff target `{other}`"
        ))),
    })
    .await
}

#[derive(Deserialize)]
pub struct LogQuery {
    #[serde(default)]
    pub limit: Option<usize>,
}

pub async fn git_log(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<LogQuery>,
) -> ApiResult<Json<Vec<CommitInfo>>> {
    let limit = q.limit.unwrap_or(50).min(500);
    Ok(Json(
        with_repo(&state, &id, move |r, _| Ok(r.log(limit)?)).await?,
    ))
}

#[derive(Serialize)]
pub struct Branches {
    pub current: Option<String>,
    pub all: Vec<String>,
}

pub async fn git_branches(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Branches>> {
    Ok(Json(
        with_repo(&state, &id, |r, _| {
            Ok(Branches {
                current: r.current_branch()?,
                all: r.branches()?,
            })
        })
        .await?,
    ))
}

#[derive(Deserialize)]
pub struct AddBody {
    pub paths: Vec<String>,
}

pub async fn git_add(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AddBody>,
) -> ApiResult<Json<Vec<StatusEntry>>> {
    Ok(Json(
        with_repo(&state, &id, move |r, _| {
            r.add(&body.paths)?;
            Ok(r.status()?)
        })
        .await?,
    ))
}

#[derive(Deserialize)]
pub struct CommitBody {
    pub message: String,
}

#[derive(Serialize)]
pub struct CommitResponse {
    pub hash: String,
}

pub async fn git_commit(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CommitBody>,
) -> ApiResult<(StatusCode, Json<CommitResponse>)> {
    let author = Identity {
        name: state.user.display_name.clone(),
        email: format!("{}@local.est", state.user.id),
    };
    let message = body.message.clone();
    let hash = with_repo(&state, &id, move |r, _| Ok(r.commit(&message, &author)?)).await?;
    let subject = body.message.lines().next().unwrap_or_default();
    state
        .store
        .record_event(
            &id,
            "git.commit",
            &json!({ "hash": hash, "subject": subject, "message_len": body.message.len() }),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(CommitResponse { hash })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_paths() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "").unwrap();
        assert!(resolve_in_workspace(dir.path(), "a.rs", false).is_ok());
        assert!(resolve_in_workspace(dir.path(), "new/dir/b.rs", true).is_ok());
        for bad in ["../x", "/etc/passwd", ".git/config", "src/../../x", ""] {
            assert!(
                resolve_in_workspace(dir.path(), bad, true).is_err(),
                "{bad}"
            );
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc", dir.path().join("link")).unwrap();
            assert!(resolve_in_workspace(dir.path(), "link/passwd", false).is_err());
            assert!(resolve_in_workspace(dir.path(), "link/x", true).is_err());
            std::os::unix::fs::symlink("/etc/hostname", dir.path().join("f")).unwrap();
            assert!(resolve_in_workspace(dir.path(), "f", true).is_err());
        }
    }
}
