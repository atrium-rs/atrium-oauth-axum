use crate::template::GlobalContext;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use tower_sessions::Session;

pub const SESSION_USER_KEY: &str = "user";

impl<S> FromRequestParts<S> for GlobalContext
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let session = parts
            .extensions
            .get::<Session>()
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

        log::warn!("session: {:?}", session);

        Ok(GlobalContext {
            user: session
                .get(SESSION_USER_KEY)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        })
    }
}
