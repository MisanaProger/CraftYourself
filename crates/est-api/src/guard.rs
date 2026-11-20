//! The daemon listens on localhost, where any web page the learner visits
//! could try to call it. Requests are therefore accepted only when:
//!
//! * the `Host` header names this daemon (defeats DNS rebinding), and
//! * an `Origin` header, if present, has the same host (defeats CSRF and
//!   cross-site WebSocket hijacking).
//!
//! Mutating endpoints additionally take JSON bodies, which browsers cannot
//! send cross-origin without a preflight.

use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

pub async fn local_only(State(allowed): State<Vec<String>>, req: Request, next: Next) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if !allowed.iter().any(|a| a.eq_ignore_ascii_case(host)) {
        return (StatusCode::FORBIDDEN, "unexpected Host header").into_response();
    }
    if let Some(origin) = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|o| o.to_str().ok())
    {
        let origin_host = origin.split_once("://").map(|(_, h)| h).unwrap_or(origin);
        if !origin_host.eq_ignore_ascii_case(host) {
            return (StatusCode::FORBIDDEN, "cross-origin request rejected").into_response();
        }
    }
    next.run(req).await
}
