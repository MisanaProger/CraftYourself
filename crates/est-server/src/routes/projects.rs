use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use est_git::Repo;
use est_storage::{new_id, NewProject, Project};
use serde::{Deserialize, Serialize};

use crate::error::{blocking, ApiError, ApiResult};
use crate::inspect;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CreateProject {
    pub url: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct ProjectResponse {
    #[serde(flatten)]
    pub project: Project,
    /// Licensing caveats the learner should be aware of.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_warning: Option<String>,
}

fn license_warning(license: Option<&str>) -> Option<String> {
    match license {
        None => Some(
            "No license detected. Without a license the code is all rights reserved: \
             study it locally, but do not redistribute it or exercises derived from it."
                .into(),
        ),
        Some(_) => None,
    }
}

/// Clones a repository into the project cache and registers it.
pub(crate) async fn clone_project(
    state: &AppState,
    req: CreateProject,
) -> ApiResult<ProjectResponse> {
    if let Some(existing) = state.store.find_project_by_url(&req.url).await? {
        return Err(ApiError::Conflict(format!(
            "repository already connected as project {}",
            existing.id
        )));
    }
    let id = new_id();
    let dest = state.config.projects_dir().join(&id);
    let policy = state.url_policy();
    let url = req.url.clone();
    let branch = req.branch.clone();

    let (head, default_branch, license, language) = blocking(move || {
        let result = (|| {
            let repo = Repo::clone(&url, &dest, branch.as_deref(), policy)?;
            let files = repo.ls_files()?;
            Ok::<_, ApiError>((
                repo.head_commit()?,
                repo.current_branch()?,
                inspect::detect_license(repo.root(), &files),
                inspect::detect_language(&files),
            ))
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&dest);
        }
        result
    })
    .await?;

    let project = state
        .store
        .insert_project(NewProject {
            name: inspect::name_from_url(&req.url),
            description: req.description,
            repository_url: req.url,
            license,
            language,
            default_branch,
            head_revision: head,
            local_path: state
                .config
                .projects_dir()
                .join(&id)
                .to_string_lossy()
                .into_owned(),
        })
        .await?;
    let license_warning = license_warning(project.license.as_deref());
    Ok(ProjectResponse {
        project,
        license_warning,
    })
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateProject>,
) -> ApiResult<(StatusCode, Json<ProjectResponse>)> {
    Ok((StatusCode::CREATED, Json(clone_project(&state, req).await?)))
}

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<Project>>> {
    Ok(Json(state.store.list_projects().await?))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ProjectResponse>> {
    let project = state.store.get_project(&id).await?;
    let license_warning = license_warning(project.license.as_deref());
    Ok(Json(ProjectResponse {
        project,
        license_warning,
    }))
}

/// Tracked files of the project: the structure overview.
pub async fn files(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<String>>> {
    let project = state.store.get_project(&id).await?;
    let files = blocking(move || Ok(Repo::open(project.local_path).ls_files()?)).await?;
    Ok(Json(files))
}
