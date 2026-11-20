use axum::Json;
use axum::extract::{Path, Query, State};
use est_application::KnowledgeProfileView;
use est_core::SessionId;
use est_domain::SessionSummary;

use super::learning::LangQuery;
use crate::error::ApiResult;
use crate::runtime::Runtime;

/// The summary of a finished session (409 while it is active).
pub async fn session_summary(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<SessionSummary>> {
    Ok(Json(rt.app.session_summary(&SessionId::from(id)).await?))
}

/// The Knowledge Profile built from session summaries.
pub async fn profile(
    State(rt): State<Runtime>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<KnowledgeProfileView>> {
    Ok(Json(rt.app.knowledge_profile(q.lang.as_deref()).await?))
}
