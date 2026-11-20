//! AI providers: presets, the learner's providers, the active one.

use axum::Json;
use axum::extract::{Path, State};
use est_ai::catalog::{PRESETS, Preset};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::ApiResult;
use crate::llm::{AddProvider, ModelsQuery, ProviderView, ProvidersView, UpdateProvider};
use crate::runtime::Runtime;

pub async fn presets() -> Json<&'static [Preset]> {
    Json(PRESETS)
}

pub async fn list(State(rt): State<Runtime>) -> ApiResult<Json<ProvidersView>> {
    Ok(Json(rt.providers.list()?))
}

pub async fn add(
    State(rt): State<Runtime>,
    Json(body): Json<AddProvider>,
) -> ApiResult<Json<ProviderView>> {
    let activate_now = body.activate;
    let mut view = rt.providers.add(body)?;
    if activate_now {
        activate(&rt, Some(&view.provider.id)).await?;
        view.active = true;
    }
    Ok(Json(view))
}

pub async fn update(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
    Json(body): Json<UpdateProvider>,
) -> ApiResult<Json<ProviderView>> {
    let view = rt.providers.update(&id, body)?;
    // Apply a new model or key to the running mentor at once.
    if view.active {
        activate(&rt, Some(&id)).await?;
    }
    Ok(Json(view))
}

pub async fn remove(State(rt): State<Runtime>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    if rt.providers.remove(&id)? {
        rt.app.set_llm(None).await?;
    }
    Ok(Json(json!({ "deleted": id })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveBody {
    /// `null` disconnects the model.
    id: Option<String>,
}

pub async fn set_active(
    State(rt): State<Runtime>,
    Json(body): Json<ActiveBody>,
) -> ApiResult<Json<ProvidersView>> {
    activate(&rt, body.id.as_deref()).await?;
    Ok(Json(rt.providers.list()?))
}

/// Builds the provider first, so a broken one never replaces a working one.
async fn activate(rt: &Runtime, id: Option<&str>) -> ApiResult<()> {
    let provider = id.map(|id| rt.providers.connect(id)).transpose()?;
    rt.providers.set_active(id)?;
    rt.app.set_llm(provider).await?;
    Ok(())
}

pub async fn models(
    State(rt): State<Runtime>,
    Json(q): Json<ModelsQuery>,
) -> ApiResult<Json<Vec<String>>> {
    Ok(Json(rt.providers.models(q).await?))
}
