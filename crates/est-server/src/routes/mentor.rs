//! Mentor routing endpoint.
//!
//! This is the programmatic front door of the AI Mentor (spec 4): every
//! question is classified, checked against the Documentation Boundary and
//! assigned a help level *before* any model is involved. Requests that hit
//! the boundary are answered with links to primary sources and never reach a
//! model. The AI Gateway that answers the remaining requests arrives in
//! Stage 2; until then `forward_to_model` tells the client what would happen.

use axum::extract::{Path, State};
use axum::Json;
use est_core::HelpLevel;
use est_git::Repo;
use est_policy::docs::definition_patterns;
use est_policy::{
    route as route_request, BoundaryDecision, DocLink, DocLinkKind, DocRouter, DocSource,
    HelpContext, MentorRequest, PolicyConfig, PolicyEngine, RouteDecision, Tool, ToolRequest,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::exercises::parse_stored;
use super::sessions::owned_session;
use crate::error::{blocking, ApiError, ApiResult};
use crate::state::AppState;

/// Event kinds that count as the learner's own attempts.
pub const ATTEMPT_EVENTS: &[&str] = &["git.commit", "tests.run"];

#[derive(Deserialize)]
pub struct MentorBody {
    pub question: String,
    #[serde(default)]
    pub excerpt: Option<String>,
    /// Requested help level; defaults to 1 (leading questions).
    #[serde(default)]
    pub level: Option<HelpLevel>,
}

#[derive(Serialize)]
pub struct MentorResponse {
    #[serde(flatten)]
    pub route: RouteDecision,
    /// Primary sources to study (filled for redirects).
    pub links: Vec<DocLink>,
    /// Whether this request may be forwarded to the mentor model.
    pub forward_to_model: bool,
}

const MAX_SOURCE_HITS: usize = 5;

pub async fn route(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<MentorBody>,
) -> ApiResult<Json<MentorResponse>> {
    if body.question.trim().is_empty() {
        return Err(ApiError::BadRequest("question must not be empty".into()));
    }
    let session = owned_session(&state, &id).await?;
    let manifest = parse_stored(&state.store.get_exercise(&session.exercise_id).await?)?;
    let attempts = state.store.count_events(&id, ATTEMPT_EVENTS).await?;

    let request = MentorRequest {
        question: body.question,
        excerpt: body.excerpt,
    };
    let help = HelpContext {
        requested: body.level.unwrap_or(HelpLevel::Direction),
        max_allowed: manifest.mentor.max_help_level,
        own_attempts: attempts,
        min_attempts_for_demonstration: manifest.mentor.min_attempts_before_demonstration,
    };
    let decision = route_request(&request, &help);

    let mut links = Vec::new();
    if let BoundaryDecision::Redirect { symbols } = &decision.boundary {
        let sources: Vec<DocSource> = manifest
            .docs
            .iter()
            .map(|d| DocSource {
                title: d.title.clone(),
                url: d.url.clone(),
                topics: d.topics.clone(),
            })
            .collect();
        let language = manifest.source.language.clone();
        let router = DocRouter {
            language: &language,
            sources: &sources,
        };
        links = router.links(symbols);

        // Point at definitions in the project itself, through the same
        // policy the mentor is bound by (hidden tests stay hidden).
        let policy =
            PolicyEngine::with_extra_denied(PolicyConfig::default(), &manifest.mentor.denied_paths)
                .map_err(|e| ApiError::Internal(e.to_string()))?;
        let symbols = symbols.clone();
        let workspace = session.workspace_path.clone();
        let source_links = blocking(move || {
            let mut out = Vec::new();
            if !policy
                .check(&ToolRequest {
                    tool: Tool::SearchRepo,
                    path: None,
                })
                .is_allowed()
            {
                return Ok(out);
            }
            let repo = Repo::open(workspace);
            for sym in &symbols {
                // `globset::escape` → prefer matches under a `globset` path.
                let segments: Vec<&str> = sym.split([':', '.']).filter(|s| !s.is_empty()).collect();
                let qualifiers = &segments[..segments.len().saturating_sub(1)];
                let mut hits = Vec::new();
                for pattern in definition_patterns(sym, &language) {
                    for hit in repo.grep(&pattern, 4 * MAX_SOURCE_HITS)? {
                        let allowed = policy
                            .check(&ToolRequest {
                                tool: Tool::ReadFile,
                                path: Some(&hit.path),
                            })
                            .is_allowed();
                        if allowed {
                            hits.push(hit);
                        }
                    }
                }
                hits.sort_by_key(|h| !qualifiers.iter().any(|q| h.path.contains(q)));
                for hit in hits {
                    let target = format!("{}:{}", hit.path, hit.line);
                    if out.len() < MAX_SOURCE_HITS
                        && !out.iter().any(|l: &DocLink| l.target == target)
                    {
                        out.push(DocLink {
                            kind: DocLinkKind::Source,
                            title: hit.text,
                            target,
                            symbol: Some(sym.clone()),
                        });
                    }
                }
            }
            Ok(out)
        })
        .await?;
        links.splice(0..0, source_links);

        if links.is_empty() {
            // Nothing specific found: offer the exercise's curated docs.
            links = sources
                .into_iter()
                .map(|s| DocLink {
                    kind: DocLinkKind::Curated,
                    title: s.title,
                    target: s.url,
                    symbol: None,
                })
                .collect();
        }
    }

    let forward_to_model = matches!(
        decision.boundary,
        BoundaryDecision::Answer | BoundaryDecision::ExplainExcerpt { .. }
    );
    let action = serde_json::to_value(&decision.boundary)
        .ok()
        .and_then(|v| v.get("action").cloned())
        .unwrap_or_default();
    state
        .store
        .record_event(
            &id,
            "mentor.request",
            &json!({
                "query_kind": decision.kind,
                "requested_level": decision.help.requested,
                "granted_level": decision.help.granted,
                "action": action,
                "links": links.len(),
            }),
        )
        .await?;

    Ok(Json(MentorResponse {
        route: decision,
        links,
        forward_to_model,
    }))
}
