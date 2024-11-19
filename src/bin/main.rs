use axum::{routing::get, Router};
use std::env;

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = Router::new().route("/", get(|| async { "Hello, World, from axum!" }));

    // run our app with hyper, listening globally on port ${PORT}
    let port = env::var("PORT").unwrap_or_else(|_| String::from("10000"));
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
