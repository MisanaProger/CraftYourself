use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use est_application::RecommendRequest;
use est_application::ports::RecommendationSet;
use serde_json::Value;

use super::learning::LangQuery;
use crate::error::ApiResult;
use crate::runtime::Runtime;

/// The latest recommendations, or `null`.
pub async fn latest(State(rt): State<Runtime>) -> ApiResult<Json<Option<RecommendationSet>>> {
    Ok(Json(rt.app.latest_recommendations().await?))
}

/// Asks the model for new recommendations (synchronous, one request).
pub async fn generate(
    State(rt): State<Runtime>,
    Json(req): Json<RecommendRequest>,
) -> ApiResult<(StatusCode, Json<RecommendationSet>)> {
    Ok((StatusCode::CREATED, Json(rt.app.recommend(req).await?)))
}

/// Exactly what the model is given.
pub async fn activity(
    State(rt): State<Runtime>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(rt.app.learning_activity(q.lang.as_deref()).await?))
}
