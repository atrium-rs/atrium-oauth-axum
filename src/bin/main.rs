use anyhow::Result;
use atrium_oauth_axum::constant::{CALLBACK_PATH, CLIENT_METADATA_PATH, JWKS_PATH};
use atrium_oauth_axum::oauth::{self, create_oauth_client};
use atrium_oauth_axum::template::{url_for, Home, Login, Page};
use atrium_oauth_client::{AuthorizeOptions, CallbackParams, OAuthClientMetadata};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Redirect;
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use jose_jwk::JwkSet;
use serde::Deserialize;
use std::{env, sync::Arc};

struct AppState {
    oauth_client: oauth::Client,
    redis_client: Arc<redis::Client>,
}

#[derive(Debug, Deserialize)]
struct OAuthLoginParams {
    username: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let redis_client = Arc::new(redis::Client::open(env::var("REDIS_URL")?)?);
    let oauth_client = create_oauth_client(
        env::var("URL").unwrap_or_else(|_| String::from("http://localhost:10000")),
        env::var("PRIVATE_KEY").ok(),
        Arc::clone(&redis_client),
    )?;

    let app = Router::new()
        .route("/", get(home))
        .route(CLIENT_METADATA_PATH, get(client_metadata))
        .route(JWKS_PATH, get(jwks))
        .route(url_for(Page::OAuthLogin), get(get_oauth_login))
        .route(url_for(Page::OAuthLogin), post(post_oauth_login))
        .route(CALLBACK_PATH, get(callback))
        .with_state(Arc::new(AppState {
            oauth_client,
            redis_client,
        }));
    // run our app with hyper, listening globally on port ${PORT}
    let port = env::var("PORT").unwrap_or_else(|_| String::from("10000"));
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    Ok(axum::serve(listener, app).await?)
}

async fn home() -> Home {
    Home {}
}

async fn client_metadata(State(state): State<Arc<AppState>>) -> Json<OAuthClientMetadata> {
    Json(state.oauth_client.client_metadata.clone())
}

async fn jwks(State(state): State<Arc<AppState>>) -> Json<JwkSet> {
    Json(state.oauth_client.jwks())
}

async fn get_oauth_login() -> Login {
    Login {}
}

async fn post_oauth_login(
    State(state): State<Arc<AppState>>,
    Form(params): Form<OAuthLoginParams>,
) -> Result<Redirect, StatusCode> {
    match state
        .oauth_client
        .authorize(params.username, AuthorizeOptions::default())
        .await
    {
        Ok(authorization_url) => Ok(Redirect::to(&authorization_url)),
        Err(err) => {
            eprintln!("failed to authorize: {err}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn callback(State(state): State<Arc<AppState>>, Form(params): Form<CallbackParams>) {
    match state.oauth_client.callback(params).await {
        Ok((_session, state)) => {
            println!("got session, state: {state:?}");
        }
        Err(err) => {
            eprintln!("failed to callback: {err}");
        }
    }
}
