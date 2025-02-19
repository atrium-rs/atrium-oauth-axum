use atrium_oauth_axum::constant::{CALLBACK_PATH, CLIENT_METADATA_PATH, JWKS_PATH};
use atrium_oauth_axum::oauth::{create_oauth_client, Client};
use atrium_oauth_axum::template::{url_for, Home, Login, Page};
use atrium_oauth_client::{AuthorizeOptions, CallbackParams, OAuthClientMetadata};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Redirect;
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use jose_jwk::JwkSet;
use serde::Deserialize;
use std::{env, io, sync::Arc};

struct AppState {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct OAuthLoginParams {
    username: String,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let client = create_oauth_client(
        env::var("URL").unwrap_or_else(|_| String::from("http://localhost:10000")),
        env::var("PRIVATE_KEY").ok(),
    )
    .expect("failed to create oauth client");
    // build our application with a single route
    let app = Router::new()
        .route("/", get(home))
        .route(CLIENT_METADATA_PATH, get(client_metadata))
        .route(JWKS_PATH, get(jwks))
        .route(url_for(Page::OAuthLogin), get(get_oauth_login))
        .route(url_for(Page::OAuthLogin), post(post_oauth_login))
        .route(CALLBACK_PATH, get(callback))
        .with_state(Arc::new(AppState { client }));

    // run our app with hyper, listening globally on port ${PORT}
    let port = env::var("PORT").unwrap_or_else(|_| String::from("10000"));
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await
}

async fn home() -> Home {
    Home {}
}

async fn client_metadata(State(state): State<Arc<AppState>>) -> Json<OAuthClientMetadata> {
    Json(state.client.client_metadata.clone())
}

async fn jwks(State(state): State<Arc<AppState>>) -> Json<JwkSet> {
    Json(state.client.jwks())
}

async fn get_oauth_login() -> Login {
    Login {}
}

async fn post_oauth_login(
    State(state): State<Arc<AppState>>,
    Form(params): Form<OAuthLoginParams>,
) -> Result<Redirect, StatusCode> {
    match state
        .client
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
    match state.client.callback(params).await {
        Ok((_session, state)) => {
            println!("got session, state: {state:?}");
        }
        Err(err) => {
            eprintln!("failed to callback: {err}");
        }
    }
}
