mod app_state;
mod config;
mod heartbeat;
mod peers;
mod register;

use axum::{Router, routing::post};
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let config = config::Config::from_env();
    let state = app_state::AppState::new(&config).await;

    let listener = TcpListener::bind(config.bind_address)
        .await
        .expect("failed to bind coordinator listener");

    let router = Router::new()
        .route("/v1/nodes/register", post(register::register))
        .with_state(state);

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("coordinator server stopped");
}
