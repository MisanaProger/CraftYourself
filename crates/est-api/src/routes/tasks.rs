use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use est_application::StartedSession;
use est_core::{ProjectId, TaskId};
use est_domain::{NewTask, Task};
use serde::Deserialize;

use crate::error::ApiResult;
use crate::runtime::Runtime;

#[derive(Deserialize)]
pub struct ListQuery {
    pub project_id: Option<String>,
}

pub async fn list(
    State(rt): State<Runtime>,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Vec<Task>>> {
    let project = q.project_id.map(ProjectId::from);
    Ok(Json(rt.app.list_tasks(project.as_ref()).await?))
}

pub async fn create(
    State(rt): State<Runtime>,
    Json(task): Json<NewTask>,
) -> ApiResult<(StatusCode, Json<Task>)> {
    Ok((StatusCode::CREATED, Json(rt.app.create_task(task).await?)))
}

pub async fn get(State(rt): State<Runtime>, Path(id): Path<String>) -> ApiResult<Json<Task>> {
    Ok(Json(rt.app.get_task(&TaskId::from(id)).await?))
}

pub async fn start(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<StartedSession>)> {
    let started = rt.app.start_task(&TaskId::from(id)).await?;
    let status = if started.resumed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(started)))
}

#[derive(Deserialize)]
pub struct ImportBody {
    /// Exercise Manifest as TOML text.
    pub manifest: String,
}

pub async fn import_exercise(
    State(rt): State<Runtime>,
    Json(body): Json<ImportBody>,
) -> ApiResult<(StatusCode, Json<Task>)> {
    Ok((
        StatusCode::CREATED,
        Json(rt.app.import_exercise(&body.manifest).await?),
    ))
}
