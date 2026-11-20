use axum::Json;
use axum::extract::{Path, Query, State};
use est_application::SessionOverview;
use est_application::ports::StoredEvent;
use est_core::SessionId;
use est_domain::{LearningSession, SessionOutcome};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::ApiResult;
use crate::runtime::Runtime;

pub async fn list(State(rt): State<Runtime>) -> ApiResult<Json<Vec<LearningSession>>> {
    Ok(Json(rt.app.list_sessions().await?))
}

pub async fn get(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<SessionOverview>> {
    Ok(Json(rt.app.session_overview(&SessionId::from(id)).await?))
}

#[derive(Deserialize)]
pub struct FinishBody {
    pub outcome: SessionOutcome,
}

pub async fn finish(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
    Json(body): Json<FinishBody>,
) -> ApiResult<Json<LearningSession>> {
    Ok(Json(
        rt.app
            .finish_session(&SessionId::from(id), body.outcome)
            .await?,
    ))
}

#[derive(Deserialize)]
pub struct DiffQuery {
    /// `worktree` (default), `staged` or `base`.
    pub against: Option<String>,
}

pub async fn diff(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
    Query(q): Query<DiffQuery>,
) -> ApiResult<String> {
    let target = q.against.as_deref().unwrap_or("worktree");
    Ok(rt.app.session_diff(&SessionId::from(id), target).await?)
}

pub async fn events(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<StoredEvent>>> {
    Ok(Json(rt.app.session_events(&SessionId::from(id)).await?))
}

pub async fn open_workspace(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let command = rt.app.open_workspace(&SessionId::from(id)).await?;
    Ok(Json(json!({ "command": command })))
}

pub async fn sandbox_info(State(rt): State<Runtime>) -> Json<est_application::ports::SandboxInfo> {
    Json(rt.app.sandbox_info())
}

/// Starts a sandbox run; returns at once with status `running`.
pub async fn start_run(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
    Json(req): Json<est_application::RunRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<est_domain::SandboxRun>)> {
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(rt.app.start_run(&SessionId::from(id), req).await?),
    ))
}

pub async fn runs(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<est_domain::SandboxRun>>> {
    Ok(Json(rt.app.runs(&SessionId::from(id)).await?))
}

pub async fn run(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<est_domain::SandboxRun>> {
    Ok(Json(rt.app.get_run(&id).await?))
}
