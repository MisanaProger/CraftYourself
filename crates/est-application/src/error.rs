use est_domain::DomainError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0} not found")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Conflict(String),
    /// A port (database, git, filesystem, editor) failed.
    #[error("{0}")]
    Infrastructure(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl From<DomainError> for AppError {
    fn from(e: DomainError) -> Self {
        match e {
            DomainError::Invalid(m) => AppError::Invalid(m),
            DomainError::InvalidTransition(m) => AppError::Conflict(m),
        }
    }
}

impl From<est_exercises::ManifestError> for AppError {
    fn from(e: est_exercises::ManifestError) -> Self {
        AppError::Invalid(e.to_string())
    }
}
