use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use est_application::AppError;
use serde_json::json;

pub struct ApiError(pub AppError);

pub type ApiResult<T> = Result<T, ApiError>;

impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self {
        Self(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Invalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::PermissionRequired(m) => {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({ "error": m, "permission": "network" })),
                )
                    .into_response();
            }
            AppError::Infrastructure(m) => {
                tracing::error!("{m}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        (status, Json(json!({ "error": self.0.to_string() }))).into_response()
    }
}
