use crate::template::{Error, GlobalContext};
use askama::Template;
use axum::{
    extract::{FromRequestParts, Request},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use tower_sessions::Session;

pub const SESSION_USER_KEY: &str = "user";

impl<S> FromRequestParts<S> for GlobalContext
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get()
            .cloned()
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

// This middleware function handles errors by intercepting the request and response.
// It extracts the session and user information, inserts the global context into the request's extensions,
// and renders an error page if the response status indicates a client or server error.
pub async fn handle_error_middleware(request: Request, next: Next) -> Response {
    // extract the session from the request, and insert it into the request's extensions
    let (mut parts, body) = request.into_parts();
    let Some(session) = parts.extensions.get::<Session>() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Ok(g) = session
        .get(SESSION_USER_KEY)
        .await
        .map(|user| GlobalContext { user })
    else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    parts.extensions.insert(g.clone());

    // run the next middleware in the stack and get the response
    let response = next.run(Request::from_parts(parts, body)).await;
    let status = response.status();
    if status.is_client_error() || status.is_server_error() {
        // if the response status indicates an error, render an error page
        let Ok(html) = Error {
            g,
            status_code: status.as_u16(),
            description: status.canonical_reason().map(|s| s.to_string()),
        }
        .render() else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        return (status, Html(html)).into_response();
    }
    response
}
