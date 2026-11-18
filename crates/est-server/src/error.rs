use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use est_exercise::ManifestError;
use est_git::GitError;
use est_storage::StorageError;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0} not found")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Unprocessable(String),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type ApiResult<T> = Result<T, ApiError>;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::Unprocessable(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::Internal(msg) => {
                tracing::error!("{msg}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}

impl From<StorageError> for ApiError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::NotFound(what) => ApiError::NotFound(what.into()),
            StorageError::Conflict(msg) => ApiError::Conflict(msg),
            other => ApiError::Internal(other.to_string()),
        }
    }
}

impl From<GitError> for ApiError {
    fn from(e: GitError) -> Self {
        match e {
            GitError::UrlNotAllowed(_) | GitError::InvalidArgument(_) => {
                ApiError::BadRequest(e.to_string())
            }
            GitError::Failed { .. } => ApiError::Unprocessable(e.to_string()),
            other => ApiError::Internal(other.to_string()),
        }
    }
}

impl From<ManifestError> for ApiError {
    fn from(e: ManifestError) -> Self {
        ApiError::Unprocessable(e.to_string())
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

/// Runs blocking work (git, filesystem) off the async runtime.
pub async fn blocking<T, F>(f: F) -> ApiResult<T>
where
    F: FnOnce() -> ApiResult<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ApiError::Internal(format!("worker panicked: {e}")))?
}
