pub mod websocket;

use axum::{routing::get, Router};
use std::net::SocketAddr;
use tokio::sync::broadcast;

pub async fn start_server() {
    let (tx, _) = broadcast::channel(100);
    let app = Router::new()
        .route("/ws", get(websocket::ws_handler))
        .with_state(tx);

    let addr = SocketAddr::from(([127,0,0,1],3000));

    println!("Server running on {}", addr);

    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app
    ).await.unwrap();
}
