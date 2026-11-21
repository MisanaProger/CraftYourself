//! Stage 9: commit review, pull requests, Issue writing.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use est_application::{DraftPullRequest, IssueText, PublishPreview, PullRequestView};
use est_core::{ProjectId, SessionId};
use est_review::commits::Review;
use est_review::issue_text::IssueTextReview;

use crate::error::ApiResult;
use crate::runtime::Runtime;

pub async fn review_commits(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<Review>> {
    Ok(Json(rt.app.review_commits(&SessionId::from(id)).await?))
}

pub async fn pull_request(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<PullRequestView>> {
    Ok(Json(rt.app.pull_request(&SessionId::from(id)).await?))
}

pub async fn draft_pull_request(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
    Json(req): Json<DraftPullRequest>,
) -> ApiResult<(StatusCode, Json<PublishPreview>)> {
    let draft = rt.app.draft_pull_request(&SessionId::from(id), req).await?;
    Ok((StatusCode::CREATED, Json(draft)))
}

pub async fn review_issue(
    State(rt): State<Runtime>,
    Json(req): Json<IssueText>,
) -> Json<IssueTextReview> {
    Json(rt.app.review_issue_text(&req))
}

pub async fn draft_issue(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
    Json(req): Json<IssueText>,
) -> ApiResult<(StatusCode, Json<PublishPreview>)> {
    let draft = rt.app.draft_new_issue(&ProjectId::from(id), req).await?;
    Ok((StatusCode::CREATED, Json(draft)))
}
