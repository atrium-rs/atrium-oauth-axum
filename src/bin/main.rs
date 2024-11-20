use atrium_oauth_axum::constant::JWKS_PATH;
use atrium_oauth_axum::oauth::create_oauth_client;
use atrium_oauth_axum::template::Home;
use atrium_oauth_axum::{constant::CLIENT_METADATA_PATH, oauth::Client};
use atrium_oauth_client::OAuthClientMetadata;
use axum::extract::State;
use axum::Json;
use axum::{routing::get, Router};
use jose_jwk::JwkSet;
use std::sync::Arc;
use std::{env, io};

struct AppState {
    client: Client,
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
