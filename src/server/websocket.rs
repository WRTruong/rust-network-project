use axum::{
    extract::{State, ws::{WebSocketUpgrade, WebSocket, Message}},
    response::IntoResponse,
};
use crate::chat::message::ChatMessage;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{broadcast, Mutex};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone)]
pub struct AppState {
    pub tx: broadcast::Sender<ChatMessage>,
    pub rooms: Arc<Mutex<HashMap<String, Vec<ChatMessage>>>>,
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

    let mut username = String::new();
    let mut joined_rooms: Vec<String> = vec![];

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(msg) = serde_json::from_str::<ChatMessage>(&text) {
                            match msg.msg_type.as_str() {

                                "join" => {
                                    username = msg.username.clone();
                                    let room = msg.room.clone();

                                    if !joined_rooms.contains(&room) {
                                        joined_rooms.push(room.clone());
                                    }

                                    let rooms = state.rooms.lock().await;
                                    if let Some(history) = rooms.get(&room) {
                                        for old_msg in history {
                                            if let Ok(json) = serde_json::to_string(old_msg) {
                                                let _ = sender.send(Message::Text(json)).await;
                                            }
                                        }
                                    }
                                    drop(rooms);

                                    let join_msg = ChatMessage {
                                        msg_type: "system".into(),
                                        username: "SYSTEM".into(),
                                        content: format!("{} joined the room", username),
                                        room,
                                    };

                                    let _ = state.tx.send(join_msg);
                                }

                                "message" => {
                                    if joined_rooms.contains(&msg.room) {
                                        let chat_msg = ChatMessage {
                                            msg_type: "message".into(),
                                            username: username.clone(),
                                            content: msg.content.clone(),
                                            room: msg.room.clone(),
                                        };

                                        let mut rooms = state.rooms.lock().await;
                                        rooms.entry(msg.room.clone())
                                            .or_insert_with(Vec::new)
                                            .push(chat_msg.clone());

                                        let _ = state.tx.send(chat_msg);
                                    }
                                }

                                "leave" => {
                                    joined_rooms.retain(|r| r != &msg.room);

                                    let leave_msg = ChatMessage {
                                        msg_type: "system".into(),
                                        username: "SYSTEM".into(),
                                        content: format!("{} left the room", username),
                                        room: msg.room,
                                    };

                                    let _ = state.tx.send(leave_msg);
                                }

                                "typing" => {
                                    // Broadcast typing indicator to all users in the room
                                    if joined_rooms.contains(&msg.room) {
                                        let typing_msg = ChatMessage {
                                            msg_type: "typing".into(),
                                            username: username.clone(),
                                            content: msg.content.clone(),
                                            room: msg.room.clone(),
                                        };
                                        let _ = state.tx.send(typing_msg);
                                    }
                                }

                                _ => {}
                            }
                        }
                    }

                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }

            outgoing = rx.recv() => {
                if let Ok(msg) = outgoing {
                    if !joined_rooms.contains(&msg.room) {
                        continue;
                    }

                    if let Ok(json) = serde_json::to_string(&msg) {
                        if sender.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }

    println!("Client disconnected");
}
