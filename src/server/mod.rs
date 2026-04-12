pub mod websocket;

use axum::{routing::get, Router};
use std::{net::SocketAddr, collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, Mutex};

use crate::server::websocket::{ws_handler, AppState};

pub async fn start_server() {
    let (tx, _) = broadcast::channel(100);
    let state: AppState = AppState {
        tx,
        rooms: Arc::new(Mutex::new(HashMap::new())),
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
