pub mod websocket;

use axum::{routing::get, Router};
use std::{collections::HashMap, collections::HashSet, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;

use crate::server::websocket::{ws_handler, AppState};

pub async fn start_server() {
    let state: AppState = AppState {
        clients: Arc::new(Mutex::new(HashMap::new())),
        rooms: Arc::new(Mutex::new(HashMap::new())),
        room_members: Arc::new(Mutex::new(HashMap::<String, HashSet<String>>::new())),
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
