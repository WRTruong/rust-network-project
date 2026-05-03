use crate::chat::message::ChatMessage;
use crate::chat::message_store::MessageStore;
use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::{Mutex, mpsc};

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<ChatMessage>>>>,
    pub rooms: Arc<Mutex<HashMap<String, Vec<ChatMessage>>>>,
    pub room_members: Arc<Mutex<HashMap<String, HashSet<String>>>>,
    pub message_store: MessageStore,
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
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
                        handle_join(&state, &client_tx, &username, &mut joined_rooms, &msg.room)
                            .await;
                    }
                    "message" => {
                        handle_message(&state, &client_tx, &username, &joined_rooms, &msg).await;
                    }
                    "media" => {
                        handle_media(&state, &client_tx, &username, &joined_rooms, &msg).await;
                    }
                    "delete" => {
                        handle_delete(&state, &client_tx, &username, &msg).await;
                    }
                    "edit" => {
                        handle_edit(&state, &client_tx, &username, &msg).await;
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
        let join_msg = ChatMessage::new(
            "system",
            "SYSTEM",
            &format!("{} joined the room", username),
            room,
        );
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

        let mut chat_msg = ChatMessage::new("message", username, &msg.content, &msg.room);
        chat_msg.target = target;

        store_room_message(state, chat_msg.clone()).await;
        send_to_private_participants(state, &chat_msg).await;
        return;
    }

    if !joined_rooms.contains(&msg.room) {
        send_local_error(client_tx, "Join the room before sending messages");
        return;
    }

    let chat_msg = ChatMessage::new("message", username, &msg.content, &msg.room);

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

    let leave_msg = ChatMessage::new(
        "system",
        "SYSTEM",
        &format!("{} left the room", username),
        room,
    );

    store_room_message(state, leave_msg.clone()).await;
    send_to_room_members(state, room, &leave_msg).await;
}

async fn handle_media(
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
    } else if !joined_rooms.contains(&msg.room) {
        send_local_error(client_tx, "Join the room before sending media");
        return;
    }

    let Some(media) = &msg.media else {
        send_local_error(client_tx, "Media metadata is required");
        return;
    };

    if !media.is_supported_type() {
        send_local_error(client_tx, "Media type must be image, video, or file");
        return;
    }

    if !media.has_required_metadata() {
        send_local_error(
            client_tx,
            "Media url, file name, and file size are required",
        );
        return;
    }

    let mut chat_msg =
        ChatMessage::new("media", username, &msg.content, &msg.room).with_media(media.clone());

    if is_private_room(&msg.room) {
        let participants = private_room_participants(&msg.room);
        chat_msg.target = participants
            .into_iter()
            .find(|participant| participant != username)
            .unwrap_or_default();
        store_room_message(state, chat_msg.clone()).await;
        send_to_private_participants(state, &chat_msg).await;
    } else {
        store_room_message(state, chat_msg.clone()).await;
        send_to_room_members(state, &msg.room, &chat_msg).await;
    }
}

async fn handle_delete(
    state: &AppState,
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    username: &str,
    msg: &ChatMessage,
) {
    let message_id = msg.content.trim();
    if message_id.is_empty() {
        send_local_error(client_tx, "Message ID is required");
        return;
    }

    if !state
        .message_store
        .can_modify_message(message_id, username)
        .await
    {
        send_local_error(client_tx, "You can only delete your own messages");
        return;
    }

    let Some(deleted_msg) = state.message_store.delete_message(message_id).await else {
        send_local_error(client_tx, "Message not found or already deleted");
        return;
    };

    replace_room_message(state, &deleted_msg).await;

    if is_private_room(&deleted_msg.room) {
        send_to_private_participants(state, &deleted_msg).await;
    } else {
        send_to_room_members(state, &deleted_msg.room, &deleted_msg).await;
    }
}

async fn handle_edit(
    state: &AppState,
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    username: &str,
    msg: &ChatMessage,
) {
    let message_id = msg.target.trim();
    let new_content = msg.content.trim();

    if message_id.is_empty() {
        send_local_error(client_tx, "Message ID is required");
        return;
    }

    if new_content.is_empty() {
        send_local_error(client_tx, "Message content is required");
        return;
    }

    if !state
        .message_store
        .can_modify_message(message_id, username)
        .await
    {
        send_local_error(client_tx, "You can only edit your own messages");
        return;
    }

    let Some(edited_msg) = state
        .message_store
        .edit_message(message_id, new_content)
        .await
    else {
        send_local_error(client_tx, "Message not found or already deleted");
        return;
    };

    replace_room_message(state, &edited_msg).await;

    if is_private_room(&edited_msg.room) {
        send_to_private_participants(state, &edited_msg).await;
    } else {
        send_to_room_members(state, &edited_msg.room, &edited_msg).await;
    }
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
            let leave_msg = ChatMessage::new(
                "system",
                "SYSTEM",
                &format!("{} disconnected", username),
                &room,
            );
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
    let history = state.message_store.get_room_messages(room).await;

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
    state.message_store.add_message(msg.clone()).await;
    let mut rooms = state.rooms.lock().await;
    rooms.entry(msg.room.clone()).or_default().push(msg);
}

async fn replace_room_message(state: &AppState, msg: &ChatMessage) {
    let mut rooms = state.rooms.lock().await;
    if let Some(messages) = rooms.get_mut(&msg.room) {
        if let Some(existing) = messages
            .iter_mut()
            .find(|message| message.message_id == msg.message_id)
        {
            *existing = msg.clone();
        }
    }
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
    let _ = client_tx.send(ChatMessage::new("error", "SYSTEM", content, ""));
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
