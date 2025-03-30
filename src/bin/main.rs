use anyhow::Result;
use atrium_api::{
    agent::{Agent, SessionManager},
    types::{string::Datetime, Collection, TryIntoUnknown},
};
use atrium_oauth_axum::{
    axum::SESSION_USER_KEY,
    constant::{CALLBACK_PATH, CLIENT_METADATA_PATH, JWKS_PATH},
    oauth::{self, create_oauth_client},
    template::{url_for, BskyPost, GlobalContext, Home, Login, Page},
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
use tokio::try_join;
use tower_sessions::{cookie::SameSite, Expiry, Session, SessionManagerLayer};
use tower_sessions_redis_store::{
    fred::{
        prelude::{ClientLike, Config},
        types::Builder,
    },
    RedisStore,
};

struct AppState {
    oauth_client: oauth::Client,
}

#[derive(Debug, Deserialize)]
struct OAuthLoginParams {
    username: String,
}

#[derive(Debug, Deserialize)]
struct PostBskyParams {
    post_text: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // initialize the logger
    env_logger::init();

    // create a redis connection pool
    let pool0 = Builder::from_config(Config::from_url_centralized(&format!(
        "{}/0",
        env::var("REDIS_URL")?
    ))?)
    .build_pool(6)?;
    let pool1 = Builder::from_config(Config::from_url_centralized(&format!(
        "{}/1",
        env::var("REDIS_URL")?
    ))?)
    .build_pool(4)?;
    pool0.connect();
    pool1.connect();
    try_join!(pool0.wait_for_connect(), pool1.wait_for_connect())?;

    // create a session layer with a redis store
    let session_layer = SessionManagerLayer::new(RedisStore::new(pool0))
        .with_expiry(Expiry::OnSessionEnd)
        .with_same_site(SameSite::Lax);
    // create an OAuth client
    let oauth_client = create_oauth_client(
        env::var("URL").unwrap_or_else(|_| String::from("http://localhost:10000")),
        env::var("PRIVATE_KEY").ok(),
        pool1,
    )?;
    // create an axum app
    let app = Router::new()
        .route("/", get(home))
        .route(CLIENT_METADATA_PATH, get(client_metadata))
        .route(JWKS_PATH, get(jwks))
        .route(url_for(Page::OAuthLogin), get(get_oauth_login))
        .route(url_for(Page::OAuthLogin), post(post_oauth_login))
        .route(url_for(Page::OAuthLogout), get(get_oauth_logout))
        .route(CALLBACK_PATH, get(callback))
        .route(url_for(Page::BskyPost), get(get_bsky_post))
        .route(url_for(Page::BskyPost), post(post_bsky_post))
        .layer(session_layer)
        .with_state(Arc::new(AppState { oauth_client }));
    // run our app with hyper, listening globally on port ${PORT}
    let port = env::var("PORT").unwrap_or_else(|_| String::from("10000"));
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    log::info!("Starting server on port {port}");
    axum::serve(listener, app).await?;

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
        Err(e) => {
            log::error!("failed to authorize: {e}");
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
                    Err(e) => log::error!("failed to insert DID into session: {e}"),
                }
            }
        }
        Err(e) => {
            log::error!("failed to callback: {e}");
        }
    }
    Err(StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_bsky_post(g: GlobalContext) -> Result<BskyPost, StatusCode> {
    Ok(BskyPost { g })
}

async fn post_bsky_post(
    g: GlobalContext,
    State(state): State<Arc<AppState>>,
    Form(params): Form<PostBskyParams>,
) -> Result<BskyPost, StatusCode> {
    let Some(user) = &g.user else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    let oauth_session = match state.oauth_client.restore(&user.did).await {
        Ok(oauth_session) => oauth_session,
        Err(e) => {
            log::error!("failed to restore session: {e}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let agent = Agent::new(oauth_session);
    let Ok(record) = atrium_api::record::KnownRecord::AppBskyFeedPost(Box::new(
        atrium_api::app::bsky::feed::post::RecordData {
            created_at: Datetime::now(),
            embed: None,
            entities: None,
            facets: None,
            labels: None,
            langs: None,
            reply: None,
            tags: None,
            text: params.post_text,
        }
        .into(),
    ))
    .try_into_unknown() else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    if let Err(e) = agent
        .api
        .com
        .atproto
        .repo
        .create_record(
            atrium_api::com::atproto::repo::create_record::InputData {
                collection: atrium_api::app::bsky::feed::Post::nsid(),
                record,
                repo: user.did.clone().into(),
                rkey: None,
                swap_commit: None,
                validate: None,
            }
            .into(),
        )
        .await
    {
        log::error!("failed to create record: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    Ok(BskyPost { g })
}
