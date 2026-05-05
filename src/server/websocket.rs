use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use crate::chat::message::ChatMessage;
use crate::db; 
use crate::auth; 
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

    // Task gửi tin nhắn từ server xuống client qua websocket
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

    // Vòng lặp nhận tin nhắn từ client
    while let Some(frame) = ws_receiver.next().await {
        match frame {
            Ok(Message::Text(text)) => {
                let Ok(msg) = serde_json::from_str::<ChatMessage>(&text) else {
                    continue;
                };

                // Kiểm tra Đăng nhập / Đăng ký / Xác thực session
                if !ensure_registered(&state, &client_tx, &mut username, &msg).await {
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
    msg: &ChatMessage,
) -> bool {
    let requested_username = msg.username.trim();

    // 1. Trường hợp chưa đăng nhập
    if current_username.is_empty() {
        let mut db_client = match db::get_db_client().await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("DB Connection Error: {}", e);
                send_local_error(client_tx, "Kết nối cơ sở dữ liệu thất bại");
                return false;
            }
        };

        match msg.msg_type.as_str() {
            "register" => {
                let mut user_exists = false;
                {
                    let check_query = db_client.query(
                        "SELECT username FROM Users WHERE username = @P1",
                        &[&requested_username],
                    ).await;

                    match check_query {
                        Ok(stream) => {
                            if let Ok(rows) = stream.into_first_result().await {
                                user_exists = !rows.is_empty();
                            }
                        }
                        Err(e) => {
                            eprintln!("Query Error: {}", e);
                            send_local_error(client_tx, "Lỗi truy vấn dữ liệu");
                            return false;
                        }
                    }
                } 

                if user_exists {
                    send_local_error(client_tx, "Tên đăng nhập đã tồn tại");
                    return false;
                }

                if auth::register(&mut db_client, requested_username, &msg.password).await {
                    let success_msg = ChatMessage {
                        msg_type: "system".to_string(),
                        username: "System".to_string(),
                        content: "Đăng ký thành công! Vui lòng đăng nhập.".to_string(),
                        room: "general".to_string(),
                        ..Default::default()
                    };
                    let _ = client_tx.send(success_msg);
                    return false; 
                } else {
                    send_local_error(client_tx, "Không thể tạo tài khoản");
                    return false;
                }
            }

            "login" => {
                if auth::login(&mut db_client, requested_username, &msg.password).await {
                    let mut clients = state.clients.lock().await;

                    if clients.contains_key(requested_username) {
                        send_local_error(client_tx, "Tài khoản này đang đăng nhập ở nơi khác");
                        return false;
                    }

                    clients.insert(requested_username.to_string(), client_tx.clone());
                    *current_username = requested_username.to_string();
                    drop(clients);
                    
                    send_all_private_chat_history(state, client_tx, requested_username).await;
                    
                    // Gửi tin nhắn chào mừng
                    let welcome = ChatMessage {
                        msg_type: "system".to_string(),
                        username: "System".to_string(),
                        content: format!("Đăng nhập thành công! Chào mừng {}", requested_username),
                        room: "general".to_string(),
                        ..Default::default()
                    };
                    let _ = client_tx.send(welcome);
                    
                    return true;
                } else {
                    send_local_error(client_tx, "Sai tên đăng nhập hoặc mật khẩu");
                    return false;
                }
            }
            _ => {
                send_local_error(client_tx, "Vui lòng đăng nhập trước");
                return false;
            }
        }
    }

    // 2. Nếu đã có session, kiểm tra khớp username
    if current_username != requested_username {
        send_local_error(client_tx, "Phiên làm việc không hợp lệ (Session mismatch)");
        return false;
    }

    true
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

    // Kiểm tra quyền vào room DM
    if is_private_room(&msg.room) && !private_room_contains_user(&msg.room, username) {
        send_local_error(client_tx, "Access denied to private room");
        return;
    }

    if !is_private_room(&msg.room) && !joined_rooms.contains(&msg.room) {
        send_local_error(client_tx, "Join the room before sending messages");
        return;
    }

    let chat_msg = ChatMessage {
        msg_type: "message".into(),
        username: username.to_string(),
        content: msg.content.clone(),
        room: msg.room.clone(),
        target: if is_private_room(&msg.room) {
             private_room_participants(&msg.room).into_iter().find(|p| p != username).unwrap_or_default()
        } else { String::new() },
        password: String::new(), // Không gửi ngược password về
        users: vec![],
    };

    // --- LƯU VÀO SQL SERVER (Async Task) ---
    let db_msg = chat_msg.clone();
    tokio::spawn(async move {
        // Gọi hàm kết nối từ db.rs
        match db::get_db_client().await {
            Ok(mut client) => {
                // Thực thi câu lệnh SQL
                let result = client.execute(
                    "INSERT INTO ChatHistory (sender, message, room, created_at) VALUES (@P1, @P2, @P3, GETDATE())",
                    &[&db_msg.username, &db_msg.content, &db_msg.room],
                ).await;

                if let Err(e) = result {
                    eprintln!("SQL Execute Error: {:?}", e);
                }
            },
            // Chuyển lỗi thành String để thỏa mãn trait Send
            Err(e) => {
                eprintln!("SQL Connection Error: {}", e.to_string());
            }
        }
    });
    // Lưu vào cache bộ nhớ và broadcast
    store_room_message(state, chat_msg.clone()).await;
    
    if is_private_room(&msg.room) {
        send_to_private_participants(state, &chat_msg).await;
    } else {
        send_to_room_members(state, &msg.room, &chat_msg).await;
    }
}

// --- GIỮ NGUYÊN CÁC HÀM HELPER CỦA BẠN (Join, Leave, Cleanup...) ---

async fn handle_join(state: &AppState, client_tx: &mpsc::UnboundedSender<ChatMessage>, username: &str, joined_rooms: &mut HashSet<String>, room: &str) {
    let room = room.trim();
    if room.is_empty() { return; }
    if is_private_room(room) && !private_room_contains_user(room, username) { return; }

    if joined_rooms.insert(room.to_string()) {
        if !is_private_room(room) {
            let mut room_members = state.room_members.lock().await;
            room_members.entry(room.to_string()).or_default().insert(username.to_string());
        }
    }
    send_history(state, client_tx, room).await;
}

async fn handle_leave(state: &AppState, username: &str, joined_rooms: &mut HashSet<String>, room: &str) {
    if !joined_rooms.remove(room) { return; }
    if !is_private_room(room) {
        let mut room_members = state.room_members.lock().await;
        if let Some(members) = room_members.get_mut(room) {
            members.remove(username);
        }
    }
}

async fn cleanup_connection(state: &AppState, username: &str, _joined_rooms: &HashSet<String>) {
    if username.is_empty() { return; }
    let mut clients = state.clients.lock().await;
    clients.remove(username);
    // ... logic notify system leave (giữ nguyên của bạn) ...
}

async fn send_history(_state: &AppState, client_tx: &mpsc::UnboundedSender<ChatMessage>, room: &str) {
    match db::get_room_history(room).await {
        Ok(messages) => {
            for (sender, content, room) in messages {
                let msg = ChatMessage {
                    msg_type: "message".into(),
                    username: sender,
                    content,
                    room,
                    ..Default::default()
                };
                let _ = client_tx.send(msg);
            }
        }
        Err(e) => eprintln!("Failed to load room history: {}", e),
    }
}

async fn send_all_private_chat_history(_state: &AppState, client_tx: &mpsc::UnboundedSender<ChatMessage>, username: &str) {
    match db::get_private_history(username).await {
        Ok(messages) => {
            for (sender, content, room) in messages {
                let msg = ChatMessage {
                    msg_type: "message".into(),
                    username: sender,
                    content,
                    room,
                    ..Default::default()
                };
                let _ = client_tx.send(msg);
            }
        }
        Err(e) => eprintln!("Failed to load private chat history: {}", e),
    }
}

async fn store_room_message(state: &AppState, msg: ChatMessage) {
    let mut rooms = state.rooms.lock().await;
    rooms.entry(msg.room.clone()).or_default().push(msg);
}

async fn send_to_room_members(state: &AppState, room: &str, msg: &ChatMessage) {
    let members = {
        let rm = state.room_members.lock().await;
        rm.get(room).map(|m| m.iter().cloned().collect::<Vec<_>>()).unwrap_or_default()
    };
    send_to_users(state, members, msg).await;
}

async fn send_to_private_participants(state: &AppState, msg: &ChatMessage) {
    send_to_users(state, private_room_participants(&msg.room), msg).await;
}

async fn send_to_users(state: &AppState, recipients: Vec<String>, msg: &ChatMessage) {
    let clients = state.clients.lock().await;
    for r in recipients {
        if let Some(tx) = clients.get(&r) { let _ = tx.send(msg.clone()); }
    }
}

fn send_local_error(client_tx: &mpsc::UnboundedSender<ChatMessage>, content: &str) {
    let _ = client_tx.send(ChatMessage {
        msg_type: "error".into(),
        username: "SYSTEM".into(),
        content: content.into(),
        room: "".into(),
        target: "".into(),
        password: "".into(),
        users: vec![],
    });
}

fn is_private_room(room: &str) -> bool { room.starts_with("dm:") }
fn private_room_contains_user(room: &str, user: &str) -> bool { private_room_participants(room).contains(&user.to_string()) }
fn private_room_participants(room: &str) -> Vec<String> {
    room.strip_prefix("dm:").map(|r| r.split(':').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()).unwrap_or_default()
}