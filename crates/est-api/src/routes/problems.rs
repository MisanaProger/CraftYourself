use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use est_application::{ImportProblem, ProblemSummary, StartedSession};

use crate::error::ApiResult;
use crate::runtime::Runtime;

/// Imports a pasted problem as an Issue and starts it (201).
pub async fn import(
    State(rt): State<Runtime>,
    Json(req): Json<ImportProblem>,
) -> ApiResult<(StatusCode, Json<StartedSession>)> {
    Ok((StatusCode::CREATED, Json(rt.app.import_problem(req).await?)))
}

pub async fn list(State(rt): State<Runtime>) -> ApiResult<Json<Vec<ProblemSummary>>> {
    Ok(Json(rt.app.problems().await?))
}
