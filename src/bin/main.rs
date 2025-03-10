use anyhow::Result;
use atrium_api::agent::SessionManager;
use atrium_oauth_axum::{
    axum::SESSION_USER_KEY,
    constant::{CALLBACK_PATH, CLIENT_METADATA_PATH, JWKS_PATH},
    oauth::{self, create_oauth_client},
    template::{url_for, GlobalContext, Home, Login, Page},
    types::User,
    utils::resolve_identity,
};
use atrium_oauth_client::{
    AuthorizeOptions, CallbackParams, KnownScope, OAuthClientMetadata, Scope,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::Redirect,
    routing::{get, post},
    Form, Json, Router,
};
use jose_jwk::JwkSet;
use serde::Deserialize;
use std::{env, sync::Arc};
use tower_sessions::{Expiry, Session, SessionManagerLayer};
use tower_sessions_redis_store::{
    fred::prelude::{ClientLike, Config, Pool},
    RedisStore,
};

struct AppState {
    oauth_client: oauth::Client,
}

#[derive(Debug, Deserialize)]
struct OAuthLoginParams {
    username: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // create a redis connection pool
    let config = Config::from_url_centralized(&env::var("REDIS_URL")?)?;
    let pool = Pool::new(config, None, None, None, 6)?;
    let redis_conn = pool.connect();
    pool.wait_for_connect().await?;

    // create a session layer with a redis store
    let session_layer =
        SessionManagerLayer::new(RedisStore::new(pool.clone())).with_expiry(Expiry::OnSessionEnd);

    let oauth_client = create_oauth_client(
        env::var("URL").unwrap_or_else(|_| String::from("http://localhost:10000")),
        env::var("PRIVATE_KEY").ok(),
        pool,
    )?;

    let app = Router::new()
        .route("/", get(home))
        .route(CLIENT_METADATA_PATH, get(client_metadata))
        .route(JWKS_PATH, get(jwks))
        .route(url_for(Page::OAuthLogin), get(get_oauth_login))
        .route(url_for(Page::OAuthLogin), post(post_oauth_login))
        .route(url_for(Page::OAuthLogout), get(get_oauth_logout))
        .route(CALLBACK_PATH, get(callback))
        .layer(session_layer)
        .with_state(Arc::new(AppState { oauth_client }));
    // run our app with hyper, listening globally on port ${PORT}
    let port = env::var("PORT").unwrap_or_else(|_| String::from("10000"));
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    axum::serve(listener, app).await?;

    redis_conn.await??;

    Ok(())
}

async fn home(g: GlobalContext) -> Result<Home, StatusCode> {
    Ok(Home { g })
}

async fn client_metadata(State(state): State<Arc<AppState>>) -> Json<OAuthClientMetadata> {
    Json(state.oauth_client.client_metadata.clone())
}

async fn jwks(State(state): State<Arc<AppState>>) -> Json<JwkSet> {
    Json(state.oauth_client.jwks())
}

async fn get_oauth_login(g: GlobalContext) -> Result<Login, StatusCode> {
    Ok(Login { g })
}

async fn post_oauth_login(
    State(state): State<Arc<AppState>>,
    Form(params): Form<OAuthLoginParams>,
) -> Result<Redirect, StatusCode> {
    match state
        .oauth_client
        .authorize(
            params.username,
            AuthorizeOptions {
                scopes: vec![
                    Scope::Known(KnownScope::Atproto),
                    Scope::Known(KnownScope::TransitionGeneric),
                ],
                ..Default::default()
            },
        )
        .await
    {
        Ok(authorization_url) => Ok(Redirect::to(&authorization_url)),
        Err(err) => {
            eprintln!("failed to authorize: {err}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_oauth_logout(session: Session) -> Redirect {
    session.clear().await;
    Redirect::to("/")
}

async fn callback(
    session: Session,
    State(state): State<Arc<AppState>>,
    Form(params): Form<CallbackParams>,
) -> Result<Redirect, StatusCode> {
    match state.oauth_client.callback(params).await {
        Ok((oauth_session, _)) => {
            let did = oauth_session.did().await.unwrap();
            if let Ok(Some(handle)) = resolve_identity(&did).await {
                match session.insert(SESSION_USER_KEY, User { did, handle }).await {
                    Ok(_) => return Ok(Redirect::to("/")),
                    Err(e) => eprintln!("failed to insert DID into session: {e}"),
                }
            }
        }
        Err(err) => {
            eprintln!("failed to callback: {err}");
        }
    }
    Err(StatusCode::INTERNAL_SERVER_ERROR)
}
