use axum::{
    extract::ws::{WebSocketUpgrade, WebSocket, Message},
    response::IntoResponse,
};

pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    println!("Client connected");

    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {

            if let Message::Text(text) = msg {
                println!("Received: {}", text);

                let _ = socket.send(Message::Text(text)).await;
            }

        }
    }

    println!("Client disconnected");
}
