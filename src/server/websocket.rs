use axum::{
    extract::State,
    extract::ws::{WebSocketUpgrade, WebSocket, Message},
    response::IntoResponse,
};
use crate::chat::message::ChatMessage;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast::Sender;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(broadcast_tx): State<Sender<ChatMessage>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, broadcast_tx))
}

async fn handle_socket(socket: WebSocket, broadcast_tx: Sender<ChatMessage>) {
    println!("Client connected");
    let (mut sender, mut receiver) = socket.split();
    let mut broadcast_rx = broadcast_tx.subscribe();

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ChatMessage>(&text) {
                            Ok(chat_message) => {
                                println!(
                                    "Received from {}: {}",
                                    chat_message.username, chat_message.content
                                );

                                let _ = broadcast_tx.send(chat_message);
                            }
                            Err(error) => {
                                eprintln!("Invalid chat message: {}", error);
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(error)) => {
                        eprintln!("WebSocket receive error: {}", error);
                        break;
                    }
                }
            }
            outgoing = broadcast_rx.recv() => {
                match outgoing {
                    Ok(chat_message) => {
                        let response = serde_json::to_string(&chat_message)
                            .expect("chat message should serialize");

                        if sender.send(Message::Text(response.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        eprintln!("Broadcast receive error: {}", error);
                        break;
                    }
                }
            }
        }
    }

    println!("Client disconnected");
}
