use axum::Json;
use axum::extract::{Path, Query, State};
use est_application::ports::StoredEvent;
use est_application::{MentorAnswer, MentorAsk, SessionOverview};
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

pub async fn mentor_ask(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
    Json(ask): Json<MentorAsk>,
) -> ApiResult<Json<MentorAnswer>> {
    Ok(Json(rt.app.mentor_ask(&SessionId::from(id), ask).await?))
}

pub async fn mentor_history(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<StoredEvent>>> {
    Ok(Json(rt.app.mentor_history(&SessionId::from(id)).await?))
}
