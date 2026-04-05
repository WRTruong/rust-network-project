use axum::{
    extract::State,
    extract::ws::{WebSocketUpgrade, WebSocket, Message},
    response::IntoResponse,
};
use crate::chat::message::ChatMessage;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast::Sender;

use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub tx: Sender<ChatMessage>,
    pub history: Arc<Mutex<Vec<ChatMessage>>>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    println!("Client connected");
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.tx.subscribe();

    let mut username: Option<String> = None;

    {
        let history = {
            state.history.lock().unwrap().clone()
        };
        for msg in history {
            let json = serde_json::to_string(&msg).unwrap();
            let _ = sender.send(Message::Text(json.into())).await;
        }
    }

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(mut msg) = serde_json::from_str::<ChatMessage>(&text) {
                            if msg.content == "__join__" {
                                username = Some(msg.username.clone());

                                let join_msg = ChatMessage {
                                    username: "SYSTEM".to_string(),
                                    content: format!("{} đã tham gia", msg.username),
                                };

                                let _ = state.tx.send(join_msg.clone());
                                {
                                    state.history.lock().unwrap().push(join_msg);
                                }
                                
                            }

                            if msg.content != "__join__"{
                                if let Some(ref u) = username {
                                    msg.username = u.clone();
                                    {
                                    state.history.lock().unwrap().push(msg.clone());
                                    }       
                                    let _ = state.tx.send(msg);
                                }
                            }
                        }
                    }

                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }


            outgoing = rx.recv() => {
                if let Ok(msg) = outgoing {
                    

                    let json = serde_json::to_string(&msg).unwrap();

                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    if let Some(u) = username {
        let leave_msg = ChatMessage {
            username: "SYSTEM".to_string(),
            content: format!("{} đã rời chat", u),
        };

        let _ = state.tx.send(leave_msg.clone());
        state.history.lock().unwrap().push(leave_msg);
    }

    println!("Client disconnected");
}