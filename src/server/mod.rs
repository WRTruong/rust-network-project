pub mod websocket;

use axum::{routing::get, Router};
use std::net::SocketAddr;
use tokio::sync::broadcast;
use std::sync::{Arc, Mutex};

use websocket::{ws_handler, AppState};

pub async fn start_server() {
    let (tx, _) = broadcast::channel(100);
    let state: AppState = AppState {
        tx,
        history: Arc::new(Mutex::new(Vec::new())),
    };
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state);

    let addr = SocketAddr::from(([127,0,0,1],3000));

    println!("Server running at ws://{}", addr);

    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app
    ).await.unwrap();
}
