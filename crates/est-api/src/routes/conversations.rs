use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use est_application::{ConversationView, NewConversation, PostMessage};
use est_core::{ConversationId, EscalationId};
use est_domain::{Conversation, EngagementState, Message};

use crate::error::{ApiError, ApiResult};
use crate::runtime::Runtime;

pub async fn list(State(rt): State<Runtime>) -> ApiResult<Json<Vec<Conversation>>> {
    Ok(Json(rt.app.list_conversations().await?))
}

pub async fn create(
    State(rt): State<Runtime>,
    Json(req): Json<NewConversation>,
) -> ApiResult<(StatusCode, Json<Conversation>)> {
    Ok((
        StatusCode::CREATED,
        Json(rt.app.create_conversation(req).await?),
    ))
}

pub async fn get(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<ConversationView>> {
    Ok(Json(
        rt.app.conversation_view(&ConversationId::from(id)).await?,
    ))
}

pub async fn delete(State(rt): State<Runtime>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    rt.app
        .delete_conversation(&ConversationId::from(id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn messages(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Message>>> {
    Ok(Json(
        rt.app
            .conversation_view(&ConversationId::from(id))
            .await?
            .messages,
    ))
}

/// Posts a message; returns the learner's message and every reply.
pub async fn post_message(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
    Json(req): Json<PostMessage>,
) -> ApiResult<(StatusCode, Json<Vec<Message>>)> {
    Ok((
        StatusCode::CREATED,
        Json(rt.app.post_message(&ConversationId::from(id), req).await?),
    ))
}

pub async fn engagement(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<EngagementState>> {
    Ok(Json(rt.app.engagement(&ConversationId::from(id)).await?))
}

pub async fn accept(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Message>>> {
    Ok(Json(
        rt.app.accept_escalation(&EscalationId::from(id)).await?,
    ))
}

pub async fn reject(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Message>>> {
    Ok(Json(
        rt.app.reject_escalation(&EscalationId::from(id)).await?,
    ))
}

/// The learner asks for the next engagement level themselves.
pub async fn raise(
    State(rt): State<Runtime>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Message>>> {
    Ok(Json(rt.app.raise_level(&ConversationId::from(id)).await?))
}

/// The `n`-th HTML page of a mentor answer, as an isolated document.
pub async fn page(
    State(rt): State<Runtime>,
    Path((id, message, n)): Path<(String, String, usize)>,
) -> Response {
    match rt
        .app
        .message_page(
            &ConversationId::from(id),
            &est_core::MessageId::from(message),
            n,
        )
        .await
    {
        Ok(html) => super::isolated::model_page(html),
        Err(e) => ApiError(e).into_response(),
    }
}
