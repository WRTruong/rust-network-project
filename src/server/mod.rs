pub mod websocket;

use axum::{Router, routing::get};
use std::{collections::HashMap, collections::HashSet, io, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::chat::message_store::MessageStore;
use crate::server::websocket::{AppState, ws_handler};
use crate::db;

pub async fn start_server() {
    // Initialize database
    if let Err(e) = db::init_db().await {
        eprintln!("Failed to initialize database: {}", e);
        return;
    }

    let state: AppState = AppState {
        clients: Arc::new(Mutex::new(HashMap::new())),
        rooms: Arc::new(Mutex::new(HashMap::new())),
        room_members: Arc::new(Mutex::new(HashMap::<String, HashSet<String>>::new())),
        message_store: MessageStore::new(),
    };
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state);

    let Ok((addr, listener)) = bind_first_available_port(3000, 3010).await else {
        eprintln!("Could not bind server to any port from 3000 to 3010");
        return;
    };

    println!("Server running at ws://{}", addr);

    if let Err(error) = axum::serve(listener, app).await {
        eprintln!("Server stopped with error: {}", error);
    }
}

async fn bind_first_available_port(
    start_port: u16,
    end_port: u16,
) -> io::Result<(SocketAddr, TcpListener)> {
    for port in start_port..=end_port {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));

        match TcpListener::bind(addr).await {
            Ok(listener) => return Ok((addr, listener)),
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                eprintln!("Port {} is already in use, trying next port", port);
            }
            Err(error) => return Err(error),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AddrInUse,
        "all configured ports are in use",
    ))
}
