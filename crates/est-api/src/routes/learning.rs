use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use est_application::learning::{LessonView, ProgramTile, ProgramView, normalize_lang};
use est_application::ports::LessonProgress;
use est_application::{GenerateProgram, GeneratedProgramSummary, StartedSession};
use serde::Deserialize;

use crate::error::{ApiError, ApiResult};
use crate::runtime::Runtime;

#[derive(Deserialize)]
pub struct LangQuery {
    pub lang: Option<String>,
}

pub async fn programs(
    State(rt): State<Runtime>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ProgramTile>>> {
    Ok(Json(
        rt.app
            .learning_programs(normalize_lang(q.lang.as_deref()))
            .await?,
    ))
}

pub async fn program(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<ProgramView>> {
    Ok(Json(
        rt.app
            .learning_program(&id, normalize_lang(q.lang.as_deref()))
            .await?,
    ))
}

pub async fn lesson(
    State(rt): State<Runtime>,
    Path((program, lesson)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<LessonView>> {
    Ok(Json(
        rt.app
            .learning_lesson(&program, &lesson, normalize_lang(q.lang.as_deref()))
            .await?,
    ))
}

pub async fn complete(
    State(rt): State<Runtime>,
    Path((program, lesson)): Path<(String, String)>,
) -> ApiResult<Json<LessonProgress>> {
    Ok(Json(rt.app.complete_reading(&program, &lesson).await?))
}

#[derive(Deserialize)]
pub struct QuizBody {
    pub answers: BTreeMap<String, usize>,
}

/// Grades a quiz on the server.
pub async fn quiz(
    State(rt): State<Runtime>,
    Path((program, lesson)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
    Json(body): Json<QuizBody>,
) -> ApiResult<Json<est_learning::QuizResult>> {
    Ok(Json(
        rt.app
            .submit_quiz(
                &program,
                &lesson,
                body.answers,
                normalize_lang(q.lang.as_deref()),
            )
            .await?,
    ))
}

#[derive(Deserialize)]
pub struct ArtifactProgress {
    pub score: u32,
    pub completed: bool,
}

/// Progress relayed by the platform page from the artifact's `postMessage`.
pub async fn artifact_progress(
    State(rt): State<Runtime>,
    Path((program, lesson)): Path<(String, String)>,
    Json(body): Json<ArtifactProgress>,
) -> ApiResult<Json<LessonProgress>> {
    Ok(Json(
        rt.app
            .artifact_progress(&program, &lesson, body.score, body.completed)
            .await?,
    ))
}

pub async fn practice(
    State(rt): State<Runtime>,
    Path((program, lesson)): Path<(String, String)>,
) -> ApiResult<(StatusCode, Json<StartedSession>)> {
    Ok((
        StatusCode::CREATED,
        Json(rt.app.start_practice(&program, &lesson).await?),
    ))
}

/// Serves an artifact file as an isolated document (see [`super::isolated`]).
pub async fn artifact_file(Path((id, path)): Path<(String, String)>) -> Response {
    match est_learning::artifact_file(&id, &path) {
        Ok((bytes, mime)) => super::isolated::document(bytes, mime),
        Err(e) => ApiError(e.into()).into_response(),
    }
}

/// The HTML page of a generated lesson, as an isolated document.
pub async fn page(
    State(rt): State<Runtime>,
    Path((program, lesson)): Path<(String, String)>,
) -> Response {
    match rt.app.lesson_page(&program, &lesson).await {
        Ok(html) => super::isolated::model_page(html),
        Err(e) => ApiError(e).into_response(),
    }
}

pub async fn generated(State(rt): State<Runtime>) -> ApiResult<Json<Vec<GeneratedProgramSummary>>> {
    Ok(Json(rt.app.generated_programs().await?))
}

pub async fn generated_one(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<GeneratedProgramSummary>> {
    Ok(Json(rt.app.generated_program(&id).await?))
}

/// Starts generating a program in the background (202 Accepted).
pub async fn generate(
    State(rt): State<Runtime>,
    Json(req): Json<GenerateProgram>,
) -> ApiResult<(StatusCode, Json<GeneratedProgramSummary>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(rt.app.generate_program(req).await?),
    ))
}

pub async fn delete_generated(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    rt.app.delete_generated_program(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}
