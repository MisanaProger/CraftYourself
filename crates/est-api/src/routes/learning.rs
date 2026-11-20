use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use est_application::StartedSession;
use est_application::learning::{LessonView, ProgramTile, ProgramView, normalize_lang};
use est_application::ports::LessonProgress;
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

/// Serves an artifact file as an isolated document: the CSP `sandbox`
/// directive gives it an opaque origin (no access to the platform's API,
/// cookies or storage, even when opened directly), and it may not load
/// anything from the network.
pub async fn artifact_file(Path((id, path)): Path<(String, String)>) -> Response {
    match est_learning::artifact_file(&id, &path) {
        Ok((bytes, mime)) => (
            [
                (header::CONTENT_TYPE, mime),
                (
                    header::CONTENT_SECURITY_POLICY,
                    "sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline'; \
                     style-src 'unsafe-inline'; img-src data:; font-src 'none'; connect-src 'none'; \
                     form-action 'none'; base-uri 'none'; frame-ancestors 'self'",
                ),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                (header::REFERRER_POLICY, "no-referrer"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            bytes,
        )
            .into_response(),
        Err(e) => ApiError(e.into()).into_response(),
    }
}
