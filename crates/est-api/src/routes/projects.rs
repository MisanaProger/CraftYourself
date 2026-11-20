use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use est_application::AddProject;
use est_core::ProjectId;
use est_domain::{CommandSpec, Project};

use crate::error::ApiResult;
use crate::runtime::Runtime;

pub async fn list(State(rt): State<Runtime>) -> ApiResult<Json<Vec<Project>>> {
    Ok(Json(rt.app.list_projects().await?))
}

/// Body: `{"source": "/path/to/repo"}` or `{"source": "https://…"}`.
pub async fn create(
    State(rt): State<Runtime>,
    Json(req): Json<AddProject>,
) -> ApiResult<(StatusCode, Json<Project>)> {
    Ok((StatusCode::CREATED, Json(rt.app.add_project(req).await?)))
}

pub async fn get(State(rt): State<Runtime>, Path(id): Path<String>) -> ApiResult<Json<Project>> {
    Ok(Json(rt.app.get_project(&ProjectId::from(id)).await?))
}

pub async fn files(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<String>>> {
    Ok(Json(rt.app.project_files(&ProjectId::from(id)).await?))
}

pub async fn commands(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<CommandSpec>>> {
    Ok(Json(rt.app.project_commands(&ProjectId::from(id)).await?))
}
