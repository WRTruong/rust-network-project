use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use crate::chat::message::ChatMessage;
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::{mpsc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<ChatMessage>>>>,
    pub rooms: Arc<Mutex<HashMap<String, Vec<ChatMessage>>>>,
    pub room_members: Arc<Mutex<HashMap<String, HashSet<String>>>>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    println!("Client connected");

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (client_tx, mut client_rx) = mpsc::unbounded_channel::<ChatMessage>();

    let send_task = tokio::spawn(async move {
        while let Some(msg) = client_rx.recv().await {
            let Ok(json) = serde_json::to_string(&msg) else {
                continue;
            };

            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    let mut username = String::new();
    let mut joined_rooms: HashSet<String> = HashSet::new();

    while let Some(frame) = ws_receiver.next().await {
        match frame {
            Ok(Message::Text(text)) => {
                let Ok(msg) = serde_json::from_str::<ChatMessage>(&text) else {
                    continue;
                };

                if !ensure_registered(&state, &client_tx, &mut username, &msg.username).await {
                    continue;
                }

                match msg.msg_type.as_str() {
                    "join" => {
                        handle_join(&state, &client_tx, &username, &mut joined_rooms, &msg.room).await;
                    }
                    "message" => {
                        handle_message(&state, &client_tx, &username, &joined_rooms, &msg).await;
                    }
                    "leave" => {
                        handle_leave(&state, &username, &mut joined_rooms, &msg.room).await;
                    }
                    _ => {}
                }
            }
            Ok(Message::Close(_)) => break,
            Err(error) => {
                eprintln!("WebSocket receive error: {}", error);
                break;
            }
            _ => {}
        }
    }

    cleanup_connection(&state, &username, &joined_rooms).await;
    send_task.abort();
    println!("Client disconnected");
}

async fn ensure_registered(
    state: &AppState,
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    current_username: &mut String,
    requested_username: &str,
) -> bool {
    if requested_username.trim().is_empty() {
        send_local_error(client_tx, "Username is required");
        return false;
    }

    if current_username.is_empty() {
        let mut clients = state.clients.lock().await;

        if clients.contains_key(requested_username) {
            send_local_error(client_tx, "Username is already in use");
            return false;
        }

        clients.insert(requested_username.to_string(), client_tx.clone());
        *current_username = requested_username.to_string();
        drop(clients);
        
        // Send private chat history for all private rooms involving this user
        send_all_private_chat_history(state, client_tx, requested_username).await;
        return true;
    }

    if current_username != requested_username {
        send_local_error(client_tx, "Username cannot change during a session");
        return false;
    }

    true
}

async fn handle_join(
    state: &AppState,
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    username: &str,
    joined_rooms: &mut HashSet<String>,
    room: &str,
) {
    let room = room.trim();
    if room.is_empty() {
        send_local_error(client_tx, "Room name is required");
        return;
    }

    if is_private_room(room) && !private_room_contains_user(room, username) {
        send_local_error(client_tx, "You cannot join this private chat");
        return;
    }

    if !joined_rooms.insert(room.to_string()) {
        send_history(state, client_tx, room).await;
        return;
    }

    if !is_private_room(room) {
        let mut room_members = state.room_members.lock().await;
        room_members
            .entry(room.to_string())
            .or_default()
            .insert(username.to_string());
    }

    send_history(state, client_tx, room).await;

    if !is_private_room(room) {
        let join_msg = ChatMessage {
            msg_type: "system".into(),
            username: "SYSTEM".into(),
            content: format!("{} joined the room", username),
            room: room.to_string(),
            target: String::new(),
            users: vec![],
        };
        store_room_message(state, join_msg.clone()).await;
        send_to_room_members(state, room, &join_msg).await;
    }
}

async fn handle_message(
    state: &AppState,
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    username: &str,
    joined_rooms: &HashSet<String>,
    msg: &ChatMessage,
) {
    if msg.room.trim().is_empty() {
        send_local_error(client_tx, "Room name is required");
        return;
    }

    if is_private_room(&msg.room) {
        if !private_room_contains_user(&msg.room, username) {
            send_local_error(client_tx, "You cannot send to this private chat");
            return;
        }

        let participants = private_room_participants(&msg.room);
        let target = participants
            .into_iter()
            .find(|participant| participant != username)
            .unwrap_or_default();

        let chat_msg = ChatMessage {
            msg_type: "message".into(),
            username: username.to_string(),
            content: msg.content.clone(),
            room: msg.room.clone(),
            target,
            users: vec![],
        };

        store_room_message(state, chat_msg.clone()).await;
        send_to_private_participants(state, &chat_msg).await;
        return;
    }

    if !joined_rooms.contains(&msg.room) {
        send_local_error(client_tx, "Join the room before sending messages");
        return;
    }

    let chat_msg = ChatMessage {
        msg_type: "message".into(),
        username: username.to_string(),
        content: msg.content.clone(),
        room: msg.room.clone(),
        target: String::new(),
        users: vec![],
    };

    store_room_message(state, chat_msg.clone()).await;
    send_to_room_members(state, &msg.room, &chat_msg).await;
}

async fn handle_leave(
    state: &AppState,
    username: &str,
    joined_rooms: &mut HashSet<String>,
    room: &str,
) {
    if !joined_rooms.remove(room) {
        return;
    }

    if is_private_room(room) {
        return;
    }

    {
        let mut room_members = state.room_members.lock().await;
        if let Some(members) = room_members.get_mut(room) {
            members.remove(username);
            if members.is_empty() {
                room_members.remove(room);
            }
        }
    }

    let leave_msg = ChatMessage {
        msg_type: "system".into(),
        username: "SYSTEM".into(),
        content: format!("{} left the room", username),
        room: room.to_string(),
        target: String::new(),
        users: vec![],
    };

    store_room_message(state, leave_msg.clone()).await;
    send_to_room_members(state, room, &leave_msg).await;
}

async fn cleanup_connection(state: &AppState, username: &str, joined_rooms: &HashSet<String>) {
    if username.is_empty() {
        return;
    }

    {
        let mut clients = state.clients.lock().await;
        clients.remove(username);
    }

    let public_rooms: Vec<String> = joined_rooms
        .iter()
        .filter(|room| !is_private_room(room))
        .cloned()
        .collect();

    for room in public_rooms {
        let should_notify = {
            let mut room_members = state.room_members.lock().await;
            if let Some(members) = room_members.get_mut(&room) {
                members.remove(username);
                let notify = !members.is_empty();
                if members.is_empty() {
                    room_members.remove(&room);
                }
                notify
            } else {
                false
            }
        };

        if should_notify {
            let leave_msg = ChatMessage {
                msg_type: "system".into(),
                username: "SYSTEM".into(),
                content: format!("{} disconnected", username),
                room: room.clone(),
                target: String::new(),
                users: vec![],
            };
            store_room_message(state, leave_msg.clone()).await;
            send_to_room_members(state, &room, &leave_msg).await;
        }
    }
}

async fn send_history(
    state: &AppState,
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    room: &str,
) {
    let history = {
        let rooms = state.rooms.lock().await;
        rooms.get(room).cloned().unwrap_or_default()
    };

    for message in history {
        let _ = client_tx.send(message);
    }
}

async fn send_all_private_chat_history(
    state: &AppState,
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    username: &str,
) {
    let rooms = state.rooms.lock().await;
    
    for (room_name, messages) in rooms.iter() {
        if is_private_room(room_name) && private_room_contains_user(room_name, username) {
            for message in messages {
                let _ = client_tx.send(message.clone());
            }
        }
    }
}

async fn store_room_message(state: &AppState, msg: ChatMessage) {
    let mut rooms = state.rooms.lock().await;
    rooms
        .entry(msg.room.clone())
        .or_default()
        .push(msg);
}

async fn send_to_room_members(state: &AppState, room: &str, msg: &ChatMessage) {
    let recipients = {
        let room_members = state.room_members.lock().await;
        room_members
            .get(room)
            .map(|members| members.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default()
    };

    send_to_users(state, recipients, msg).await;
}

async fn send_to_private_participants(state: &AppState, msg: &ChatMessage) {
    let recipients = private_room_participants(&msg.room);
    send_to_users(state, recipients, msg).await;
}

async fn send_to_users(state: &AppState, recipients: Vec<String>, msg: &ChatMessage) {
    let client_senders = {
        let clients = state.clients.lock().await;
        recipients
            .iter()
            .filter_map(|username| clients.get(username).cloned())
            .collect::<Vec<_>>()
    };

    for sender in client_senders {
        let _ = sender.send(msg.clone());
    }
}

fn send_local_error(client_tx: &mpsc::UnboundedSender<ChatMessage>, content: &str) {
    let _ = client_tx.send(ChatMessage {
        msg_type: "error".into(),
        username: "SYSTEM".into(),
        content: content.into(),
        room: String::new(),
        target: String::new(),
        users: vec![],
    });
}

fn is_private_room(room: &str) -> bool {
    room.starts_with("dm:")
}

fn private_room_contains_user(room: &str, username: &str) -> bool {
    private_room_participants(room)
        .into_iter()
        .any(|participant| participant == username)
}

fn private_room_participants(room: &str) -> Vec<String> {
    room.strip_prefix("dm:")
        .map(|rest| {
            rest.split(':')
                .filter(|segment| !segment.is_empty())
                .map(|segment| segment.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}
