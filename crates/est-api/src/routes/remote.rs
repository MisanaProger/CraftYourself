//! Forges and judges: Issue import, statement loading, confirmed publishing.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use est_application::ports::FetchedProblem;
use est_application::{
    ConfirmPublish, DraftComment, FetchProblem, ImportIssue, IssuePreview, IssueRequest,
    PublishPreview, Published,
};
use est_core::TaskId;
use est_domain::Task;

use crate::error::ApiResult;
use crate::runtime::Runtime;

pub async fn preview_issue(
    State(rt): State<Runtime>,
    Json(req): Json<IssueRequest>,
) -> ApiResult<Json<IssuePreview>> {
    Ok(Json(rt.app.preview_issue(req).await?))
}

pub async fn import_issue(
    State(rt): State<Runtime>,
    Json(req): Json<ImportIssue>,
) -> ApiResult<(StatusCode, Json<Task>)> {
    Ok((StatusCode::CREATED, Json(rt.app.import_issue(req).await?)))
}

pub async fn fetch_problem(
    State(rt): State<Runtime>,
    Json(req): Json<FetchProblem>,
) -> ApiResult<Json<FetchedProblem>> {
    Ok(Json(rt.app.fetch_problem(req).await?))
}

pub async fn draft_comment(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
    Json(req): Json<DraftComment>,
) -> ApiResult<(StatusCode, Json<PublishPreview>)> {
    let draft = rt.app.draft_issue_comment(&TaskId::from(id), req).await?;
    Ok((StatusCode::CREATED, Json(draft)))
}

pub async fn confirm(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
    Json(req): Json<ConfirmPublish>,
) -> ApiResult<Json<Published>> {
    Ok(Json(rt.app.confirm_publish(&id, req).await?))
}

pub async fn discard(State(rt): State<Runtime>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    rt.app.discard_draft(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}
