use axum::Json;
use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use est_application::{AdiHistoryItem, AdiOverview, PrivacySettings, SkillMatrix};
use serde_json::{Value, json};

use crate::error::ApiResult;
use crate::runtime::Runtime;

pub async fn adi(State(rt): State<Runtime>) -> ApiResult<Json<AdiOverview>> {
    Ok(Json(rt.app.adi_overview().await?))
}

pub async fn adi_history(State(rt): State<Runtime>) -> ApiResult<Json<Vec<AdiHistoryItem>>> {
    Ok(Json(rt.app.adi_overview().await?.history))
}

pub async fn disciplines(State(rt): State<Runtime>) -> ApiResult<Json<SkillMatrix>> {
    Ok(Json(rt.app.skill_matrix().await?))
}

/// Flat list of skills with their discipline.
pub async fn skills(State(rt): State<Runtime>) -> ApiResult<Json<Vec<Value>>> {
    let m = rt.app.skill_matrix().await?;
    Ok(Json(
        m.disciplines
            .iter()
            .flat_map(|d| {
                d.skills
                    .iter()
                    .map(move |s| json!({ "discipline": d.id, "skill": s }))
            })
            .collect(),
    ))
}

pub async fn privacy(State(rt): State<Runtime>) -> Json<PrivacySettings> {
    Json(rt.app.privacy_settings())
}

/// Full export of the learner's data as a downloadable JSON file.
pub async fn export(State(rt): State<Runtime>) -> ApiResult<impl IntoResponse> {
    let data = rt.app.export_data().await?;
    let name = format!(
        "craft-yourself-export-{}.json",
        est_core::now().format("%Y%m%d-%H%M%S")
    );
    Ok((
        [
            (header::CONTENT_TYPE, "application/json".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
        ],
        serde_json::to_string_pretty(&data).unwrap_or_default(),
    ))
}
