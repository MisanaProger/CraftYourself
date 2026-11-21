use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use est_application::{DiscoverRequest, Discovery};

use crate::error::ApiResult;
use crate::runtime::Runtime;

/// Plans searches with the model and runs them (201).
pub async fn discover(
    State(rt): State<Runtime>,
    Json(req): Json<DiscoverRequest>,
) -> ApiResult<(StatusCode, Json<Discovery>)> {
    Ok((StatusCode::CREATED, Json(rt.app.discover(req).await?)))
}

pub async fn latest(State(rt): State<Runtime>) -> ApiResult<Json<Option<Discovery>>> {
    Ok(Json(rt.app.latest_discovery().await?))
}
