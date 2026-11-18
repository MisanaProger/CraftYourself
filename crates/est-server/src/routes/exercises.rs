use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use est_exercise::ExerciseManifest;
use est_storage::{ExerciseRecord, NewExercise};
use serde::Serialize;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Serialize)]
pub struct ExerciseResponse {
    #[serde(flatten)]
    pub record: ExerciseRecord,
    pub manifest: ExerciseManifest,
}

fn kebab<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

pub(crate) fn parse_stored(record: &ExerciseRecord) -> ApiResult<ExerciseManifest> {
    ExerciseManifest::from_toml(&record.manifest_toml)
        .map_err(|e| ApiError::Internal(format!("stored manifest {} is invalid: {e}", record.id)))
}

/// Registers an exercise. The body is the manifest as TOML text.
pub async fn create(
    State(state): State<AppState>,
    body: String,
) -> ApiResult<(StatusCode, Json<ExerciseResponse>)> {
    let manifest = ExerciseManifest::from_toml(&body)?;
    let ex = &manifest.exercise;
    let record = state
        .store
        .upsert_exercise(NewExercise {
            id: &ex.id,
            version: &ex.version,
            title: &ex.title,
            category: &kebab(&ex.category),
            difficulty: &kebab(&ex.difficulty),
            repository: &manifest.source.repository,
            revision: &manifest.source.revision,
            manifest_toml: &body,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ExerciseResponse { record, manifest }),
    ))
}

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<ExerciseRecord>>> {
    Ok(Json(state.store.list_exercises().await?))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ExerciseResponse>> {
    let record = state.store.get_exercise(&id).await?;
    let manifest = parse_stored(&record)?;
    Ok(Json(ExerciseResponse { record, manifest }))
}
