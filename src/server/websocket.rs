use crate::auth::{self, UserSession};
use crate::chat::message::ChatMessage;
use crate::chat::message_store::MessageStore;
use crate::db;
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

#[derive(serde::Deserialize)]
struct ProfilePayload {
    display_name: String,
    bio: String,
    #[serde(default)]
    avatar_url: Option<String>,
}

#[derive(serde::Deserialize)]
struct PasswordPayload {
    old_password: String,
    new_password: String,
}

#[derive(serde::Deserialize)]
struct AdminUserUpdatePayload {
    user_id: i32,
    role: String,
    is_active: bool,
}

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
    let mut user_session: Option<UserSession> = None;
    let mut joined_rooms: HashSet<String> = HashSet::new();

    // Vòng lặp nhận tin nhắn từ client
    while let Some(frame) = ws_receiver.next().await {
        match frame {
            Ok(Message::Text(text)) => {
                let Ok(msg) = serde_json::from_str::<ChatMessage>(&text) else {
                    continue;
                };

                // Kiểm tra Đăng nhập / Đăng ký / Xác thực session
                if !ensure_registered(&state, &client_tx, &mut username, &mut user_session, &msg)
                    .await
                {
                    continue;
                }

                let Some(session) = user_session.as_mut() else {
                    continue;
                };

                match msg.msg_type.as_str() {
                    "join" => {
                        handle_join(&state, &client_tx, session, &mut joined_rooms, &msg.room)
                            .await;
                    }
                    "message" => {
                        handle_message(&state, &client_tx, session, &joined_rooms, &msg).await;
                    }
                    "media" => {
                        handle_media(&state, &client_tx, session, &joined_rooms, &msg).await;
                    }
                    "delete" => {
                        handle_delete(&state, &client_tx, session, &msg).await;
                    }
                    "edit" => {
                        handle_edit(&state, &client_tx, session, &msg).await;
                    }
                    "leave" => {
                        handle_leave(&state, &username, &mut joined_rooms, &msg.room).await;
                    }
                    "profile_get" => handle_profile_get(&client_tx, session).await,
                    "profile_update" => handle_profile_update(&client_tx, session, &msg).await,
                    "friend_search" => handle_friend_search(&client_tx, session, &msg).await,
                    "friend_request" => handle_friend_request(&client_tx, session, &msg).await,
                    "friend_accept" => handle_friend_respond(&client_tx, session, &msg, true).await,
                    "friend_decline" => {
                        handle_friend_respond(&client_tx, session, &msg, false).await
                    }
                    "friends_list" => send_friends_list(&client_tx, session).await,
                    "group_create" => handle_group_create(&client_tx, session, &msg).await,
                    "group_invite" => handle_group_invite(&client_tx, session, &msg).await,
                    "group_invite_accept" => {
                        handle_group_invite_accept(&client_tx, session, &msg).await
                    }
                    "group_join_request" => {
                        handle_group_join_request(&client_tx, session, &msg).await
                    }
                    "group_join_accept" => {
                        handle_group_join_respond(&client_tx, session, &msg, true).await
                    }
                    "group_join_decline" => {
                        handle_group_join_respond(&client_tx, session, &msg, false).await
                    }
                    "groups_list" => send_groups_list(&client_tx, session).await,
                    "settings_change_password" => {
                        handle_change_password(&client_tx, session, &msg).await
                    }
                    "admin_users_list" => handle_admin_users_list(&client_tx, session, &msg).await,
                    "admin_user_update" => {
                        handle_admin_user_update(&client_tx, session, &msg).await
                    }
                    "logout" => break,
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
    current_session: &mut Option<UserSession>,
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
                // Kiểm tra email/phone hợp lệ
                let email = msg.email.trim();
                let phone = msg.phone.trim();
                
                if !auth::validate_email(email) {
                    send_local_error(client_tx, "Email không hợp lệ");
                    return false;
                }
                
                if !auth::validate_phone(phone) {
                    send_local_error(client_tx, "Số điện thoại không hợp lệ");
                    return false;
                }

                // Kiểm tra tên đăng nhập đã tồn tại
                let mut user_exists = false;
                {
                    let check_query = db_client
                        .query(
                            "SELECT username FROM Users WHERE username = @P1 OR email = @P2 OR phone = @P3",
                            &[&requested_username, &email, &phone],
                        )
                        .await;

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
                    send_local_error(client_tx, "Tên đăng nhập, email hoặc số điện thoại đã tồn tại");
                    return false;
                }

                if auth::register(&mut db_client, requested_username, email, phone, &msg.password).await {
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
                if let Some(session) =
                    auth::login(&mut db_client, requested_username, &msg.password).await
                {
                    let mut clients = state.clients.lock().await;

                    if clients.contains_key(&session.username) {
                        send_local_error(client_tx, "Tài khoản này đang đăng nhập ở nơi khác");
                        return false;
                    }

                    clients.insert(session.username.clone(), client_tx.clone());
                    *current_username = session.username.clone();
                    drop(clients);

                    send_all_private_chat_history(state, client_tx, &session.username).await;

                    // Gửi tin nhắn chào mừng
                    let welcome = ChatMessage {
                        msg_type: "system".to_string(),
                        username: "System".to_string(),
                        content: format!(
                            "Đăng nhập thành công! Chào mừng {} ({})",
                            session.username, session.role
                        ),
                        room: "general".to_string(),
                        ..Default::default()
                    };
                    let _ = client_tx.send(welcome);
                    *current_session = Some(session);

                    return true;
                } else {
                    send_local_error(client_tx, "Sai tên đăng nhập/email/số điện thoại hoặc mật khẩu");
                    return false;
                }
            }
            _ => {
                send_local_error(client_tx, "Vui lòng đăng nhập trước");
                return false;
            }
        }
    }

    // 2. Nếu đã có session hợp lệ, không cần kiểm tra username
    // Session chứa thông tin xác thực đã được chứng minh
    // Username trong message có thể là email/phone hoặc bất kỳ giá trị nào
    if current_session.is_some() {
        // Đã xác thực, có thể thực hiện hành động
        return true;
    }
    
    // Nếu chưa có session, phải có username match
    if current_username != requested_username {
        send_local_error(client_tx, "Phiên làm việc không hợp lệ (Session mismatch)");
        return false;
    }

    true
}

async fn handle_message(
    state: &AppState,
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    joined_rooms: &HashSet<String>,
    msg: &ChatMessage,
) {
    let username = &session.username;
    if !session.has_permission("chat.send") {
        send_local_error(client_tx, "You do not have permission to send messages");
        return;
    }

    if msg.room.trim().is_empty() {
        send_local_error(client_tx, "Room name is required");
        return;
    }

    // Kiểm tra quyền vào room DM
    if is_private_room(&msg.room) && !private_room_contains_user(&msg.room, username) {
        send_local_error(client_tx, "Access denied to private room");
        return;
    }

    if is_private_room(&msg.room) {
        let target = private_room_participants(&msg.room)
            .into_iter()
            .find(|p| p != username)
            .unwrap_or_default();
        if target.is_empty() || !db::are_friends(username, &target).await.unwrap_or(false) {
            send_local_error(client_tx, "Only friends can use private chat");
            return;
        }
    }

    if let Some(group_name) = group_room_name(&msg.room) {
        if !db::is_group_member(group_name, session.user_id)
            .await
            .unwrap_or(false)
        {
            send_local_error(client_tx, "Join the group before sending messages");
            return;
        }
    }

    if !is_private_room(&msg.room) && !joined_rooms.contains(&msg.room) {
        send_local_error(client_tx, "Join the room before sending messages");
        return;
    }

    let mut chat_msg = ChatMessage::new("message", username, &msg.content, &msg.room);
    chat_msg.target = if is_private_room(&msg.room) {
        private_room_participants(&msg.room)
            .into_iter()
            .find(|p| p != username)
            .unwrap_or_default()
    } else {
        String::new()
    };
    chat_msg.password = String::new();
    chat_msg.sender_avatar = session.avatar_url.clone();

    // --- LƯU VÀO SQL SERVER (Async Task) ---
    if let Err(e) = db::save_chat_message(&chat_msg, session.user_id).await {
        eprintln!("SQL Execute Error: {:?}", e);
        send_local_error(client_tx, "Could not save message");
        return;
    }
    // Lưu vào cache bộ nhớ và broadcast
    store_room_message(state, chat_msg.clone()).await;

    if is_private_room(&msg.room) {
        send_to_private_participants(state, &chat_msg).await;
    } else {
        send_to_room_members(state, &msg.room, &chat_msg).await;
    }
}

// --- GIỮ NGUYÊN CÁC HÀM HELPER CỦA BẠN (Join, Leave, Cleanup...) ---

async fn handle_join(
    state: &AppState,
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    joined_rooms: &mut HashSet<String>,
    room: &str,
) {
    let username = &session.username;
    let room = room.trim();
    if room.is_empty() {
        return;
    }
    if is_private_room(room) && !private_room_contains_user(room, username) {
        return;
    }
    if is_private_room(room) {
        let target = private_room_participants(room)
            .into_iter()
            .find(|p| p != username)
            .unwrap_or_default();
        if target.is_empty() || !db::are_friends(username, &target).await.unwrap_or(false) {
            send_local_error(client_tx, "Only friends can use private chat");
            return;
        }
    }
    if let Some(group_name) = group_room_name(room) {
        if !db::is_group_member(group_name, session.user_id)
            .await
            .unwrap_or(false)
        {
            send_local_error(client_tx, "You are not a member of this group");
            return;
        }
    }

    if joined_rooms.insert(room.to_string()) {
        if !is_private_room(room) {
            let mut room_members = state.room_members.lock().await;
            room_members
                .entry(room.to_string())
                .or_default()
                .insert(username.to_string());
        }
    }
    send_history(state, client_tx, room).await;
}

async fn handle_media(
    state: &AppState,
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    joined_rooms: &HashSet<String>,
    msg: &ChatMessage,
) {
    let username = &session.username;
    if !session.has_permission("chat.media") {
        send_local_error(client_tx, "You do not have permission to send media");
        return;
    }

    if msg.room.trim().is_empty() {
        send_local_error(client_tx, "Room name is required");
        return;
    }
    if msg.media.is_none() {
        send_local_error(client_tx, "Media metadata is required");
        return;
    }

    // Kiểm tra kích thước file URL (base64 string)
    if let Some(media) = &msg.media {
        let max_size = 10 * 1024 * 1024; // 10MB cho base64 string
        if media.file_url.len() > max_size {
            send_local_error(
                client_tx,
                &format!("File quá lớn! Tối đa 10MB (hiện tại: {})", media.file_size),
            );
            return;
        }
    }

    if is_private_room(&msg.room) && !private_room_contains_user(&msg.room, username) {
        send_local_error(client_tx, "Access denied to private room");
        return;
    }
    if is_private_room(&msg.room) {
        let target = private_room_participants(&msg.room)
            .into_iter()
            .find(|p| p != username)
            .unwrap_or_default();
        if target.is_empty() || !db::are_friends(username, &target).await.unwrap_or(false) {
            send_local_error(client_tx, "Only friends can use private chat");
            return;
        }
    }
    if let Some(group_name) = group_room_name(&msg.room) {
        if !db::is_group_member(group_name, session.user_id)
            .await
            .unwrap_or(false)
        {
            send_local_error(client_tx, "Join the group before sending media");
            return;
        }
    }
    if !is_private_room(&msg.room) && !joined_rooms.contains(&msg.room) {
        send_local_error(client_tx, "Join the room before sending media");
        return;
    }

    let mut chat_msg = ChatMessage::new("media", username, &msg.content, &msg.room);
    chat_msg.target = if is_private_room(&msg.room) {
        private_room_participants(&msg.room)
            .into_iter()
            .find(|p| p != username)
            .unwrap_or_default()
    } else {
        String::new()
    };
    chat_msg.password = String::new();
    chat_msg.media = msg.media.clone();
    chat_msg.sender_avatar = session.avatar_url.clone();

    // --- LƯU VÀO SQL SERVER (Async Task) ---
    if let Err(e) = db::save_chat_message(&chat_msg, session.user_id).await {
        eprintln!("SQL Execute Error (Media): {:?}", e);
        send_local_error(client_tx, "Could not save media message");
        return;
    }

    store_room_message(state, chat_msg.clone()).await;
    if is_private_room(&msg.room) {
        send_to_private_participants(state, &chat_msg).await;
    } else {
        send_to_room_members(state, &msg.room, &chat_msg).await;
    }
}

async fn handle_delete(
    state: &AppState,
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    msg: &ChatMessage,
) {
    let username = &session.username;
    if !session.has_permission("message.delete.own") {
        send_local_error(client_tx, "You do not have permission to delete messages");
        return;
    }

    if msg.message_id.trim().is_empty() {
        send_local_error(client_tx, "Message ID is required for deletion");
        return;
    }
    match db::delete_message(&msg.message_id, username).await {
        Ok(Some(edited)) => {
            let _ = state.message_store.delete_message(&msg.message_id).await;
            if is_private_room(&edited.room) {
                send_to_private_participants(state, &edited).await;
            } else {
                send_to_room_members(state, &edited.room, &edited).await;
            }
        }
        Ok(None) => send_local_error(client_tx, "Message not found or already deleted"),
        Err(e) => {
            eprintln!("SQL Delete Error: {:?}", e);
            send_local_error(client_tx, "Could not delete message");
        }
    }
}

async fn handle_edit(
    state: &AppState,
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    msg: &ChatMessage,
) {
    let username = &session.username;
    if !session.has_permission("message.edit.own") {
        send_local_error(client_tx, "You do not have permission to edit messages");
        return;
    }

    if msg.message_id.trim().is_empty() {
        send_local_error(client_tx, "Message ID is required for edit");
        return;
    }
    if msg.content.trim().is_empty() {
        send_local_error(client_tx, "New content is required for edit");
        return;
    }
    match db::edit_message(&msg.message_id, username, &msg.content).await {
        Ok(Some(edited)) => {
            let _ = state
                .message_store
                .edit_message(&msg.message_id, &msg.content)
                .await;
            if is_private_room(&edited.room) {
                send_to_private_participants(state, &edited).await;
            } else {
                send_to_room_members(state, &edited.room, &edited).await;
            }
        }
        Ok(None) => send_local_error(client_tx, "Message not found or cannot be edited"),
        Err(e) => {
            eprintln!("SQL Edit Error: {:?}", e);
            send_local_error(client_tx, "Could not edit message");
        }
    }
}

async fn handle_profile_get(client_tx: &mpsc::UnboundedSender<ChatMessage>, session: &UserSession) {
    match db::get_profile(session.user_id).await {
        Ok(profile) => send_json(client_tx, "profile_data", &profile),
        Err(e) => {
            eprintln!("Profile Error: {:?}", e);
            send_local_error(client_tx, "Could not load profile");
        }
    }
}

async fn handle_profile_update(
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &mut UserSession,
    msg: &ChatMessage,
) {
    if !session.has_permission("profile.update") {
        send_local_error(client_tx, "You do not have permission to update profile");
        return;
    }
    let Ok(payload) = serde_json::from_str::<ProfilePayload>(&msg.content) else {
        send_local_error(client_tx, "Invalid profile data");
        return;
    };
    match db::update_profile(
        session.user_id, 
        &payload.display_name, 
        &payload.bio,
        payload.avatar_url.as_deref()
    ).await {
        Ok(profile) => {
            send_json(client_tx, "profile_data", &profile);
            session.display_name = Some(profile.display_name);
            session.avatar_url = profile.avatar_url;
        }
        Err(e) => {
            eprintln!("Profile Update Error: {:?}", e);
            send_local_error(client_tx, "Could not update profile");
        }
    }
}

async fn handle_friend_search(
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    msg: &ChatMessage,
) {
    match db::search_user(msg.target.trim(), session.user_id).await {
        Ok(result) => send_json(client_tx, "friend_search_result", &result),
        Err(e) => {
            eprintln!("Friend Search Error: {:?}", e);
            send_local_error(client_tx, "Could not search user");
        }
    }
}

async fn handle_friend_request(
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    msg: &ChatMessage,
) {
    if !session.has_permission("friend.manage") {
        send_local_error(client_tx, "You do not have permission to manage friends");
        return;
    }
    match db::send_friend_request(session.user_id, msg.target.trim()).await {
        Ok(()) => {
            send_system(client_tx, "Da gui loi moi ket ban");
            send_friends_list(client_tx, session).await;
        }
        Err(e) => {
            eprintln!("Friend Request Error: {:?}", e);
            send_local_error(client_tx, "Could not send friend request");
        }
    }
}

async fn handle_friend_respond(
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    msg: &ChatMessage,
    accept: bool,
) {
    match db::respond_friend_request(session.user_id, msg.target.trim(), accept).await {
        Ok(()) => send_friends_list(client_tx, session).await,
        Err(e) => {
            eprintln!("Friend Respond Error: {:?}", e);
            send_local_error(client_tx, "Could not update friend request");
        }
    }
}

async fn send_friends_list(client_tx: &mpsc::UnboundedSender<ChatMessage>, session: &UserSession) {
    match db::friend_lists(session.user_id).await {
        Ok(list) => send_json(client_tx, "friends_data", &list),
        Err(e) => {
            eprintln!("Friends List Error: {:?}", e);
            send_local_error(client_tx, "Could not load friends");
        }
    }
}

async fn handle_group_create(
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    msg: &ChatMessage,
) {
    if !session.has_permission("group.create") {
        send_local_error(client_tx, "You do not have permission to create groups");
        return;
    }
    match db::create_group(session.user_id, msg.target.trim()).await {
        Ok(()) => send_groups_list(client_tx, session).await,
        Err(e) => {
            eprintln!("Group Create Error: {:?}", e);
            send_local_error(client_tx, "Could not create group");
        }
    }
}

async fn handle_group_invite(
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    msg: &ChatMessage,
) {
    let group_name = group_room_name(&msg.room).unwrap_or(msg.room.trim());
    match db::group_invite(session.user_id, group_name, msg.target.trim()).await {
        Ok(()) => send_groups_list(client_tx, session).await,
        Err(e) => {
            eprintln!("Group Invite Error: {:?}", e);
            send_local_error(client_tx, "Could not invite user");
        }
    }
}

async fn handle_group_invite_accept(
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    msg: &ChatMessage,
) {
    match db::group_invite_accept(session.user_id, msg.target.trim()).await {
        Ok(()) => send_groups_list(client_tx, session).await,
        Err(e) => {
            eprintln!("Group Invite Accept Error: {:?}", e);
            send_local_error(client_tx, "Could not accept group invite");
        }
    }
}

async fn handle_group_join_request(
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    msg: &ChatMessage,
) {
    match db::group_join_request(session.user_id, msg.target.trim()).await {
        Ok(()) => {
            send_system(client_tx, "Da gui yeu cau vao nhom");
            send_groups_list(client_tx, session).await;
        }
        Err(e) => {
            eprintln!("Group Join Request Error: {:?}", e);
            send_local_error(client_tx, "Could not request group access");
        }
    }
}

async fn handle_group_join_respond(
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    msg: &ChatMessage,
    accept: bool,
) {
    let group_name = group_room_name(&msg.room).unwrap_or(msg.room.trim());
    match db::group_join_respond(session.user_id, group_name, msg.target.trim(), accept).await {
        Ok(()) => send_groups_list(client_tx, session).await,
        Err(e) => {
            eprintln!("Group Join Respond Error: {:?}", e);
            send_local_error(client_tx, "Could not update join request");
        }
    }
}

async fn send_groups_list(client_tx: &mpsc::UnboundedSender<ChatMessage>, session: &UserSession) {
    match db::group_lists(session.user_id).await {
        Ok(list) => send_json(client_tx, "groups_data", &list),
        Err(e) => {
            eprintln!("Groups List Error: {:?}", e);
            send_local_error(client_tx, "Could not load groups");
        }
    }
}

async fn handle_change_password(
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    msg: &ChatMessage,
) {
    if !session.has_permission("settings.update_password") {
        send_local_error(client_tx, "You do not have permission to update password");
        return;
    }
    let Ok(payload) = serde_json::from_str::<PasswordPayload>(&msg.content) else {
        send_local_error(client_tx, "Invalid password data");
        return;
    };
    let mut client = match db::get_db_client().await {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Password DB Error: {:?}", e);
            send_local_error(client_tx, "Could not connect to database");
            return;
        }
    };
    if auth::change_password(
        &mut client,
        session.user_id,
        &payload.old_password,
        &payload.new_password,
    )
    .await
    {
        send_system(client_tx, "Doi mat khau thanh cong");
    } else {
        send_local_error(client_tx, "Old password is incorrect");
    }
}

async fn handle_admin_users_list(
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    msg: &ChatMessage,
) {
    if !session.has_permission("admin.manage_users") {
        send_local_error(client_tx, "You do not have permission to manage users");
        return;
    }

    match db::admin_list_users(msg.target.trim()).await {
        Ok(list) => send_json(client_tx, "admin_users_data", &list),
        Err(e) => {
            eprintln!("Admin Users List Error: {:?}", e);
            send_local_error(client_tx, "Could not load users");
        }
    }
}

async fn handle_admin_user_update(
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    session: &UserSession,
    msg: &ChatMessage,
) {
    if !session.has_permission("admin.manage_users") {
        send_local_error(client_tx, "You do not have permission to manage users");
        return;
    }

    let Ok(payload) = serde_json::from_str::<AdminUserUpdatePayload>(&msg.content) else {
        send_local_error(client_tx, "Invalid user update data");
        return;
    };

    match db::admin_update_user(payload.user_id, &payload.role, payload.is_active).await {
        Ok(list) => send_json(client_tx, "admin_users_data", &list),
        Err(e) => {
            eprintln!("Admin User Update Error: {:?}", e);
            send_local_error(client_tx, "Could not update user");
        }
    }
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
    if !is_private_room(room) {
        let mut room_members = state.room_members.lock().await;
        if let Some(members) = room_members.get_mut(room) {
            members.remove(username);
        }
    }
}

async fn cleanup_connection(state: &AppState, username: &str, _joined_rooms: &HashSet<String>) {
    if username.is_empty() {
        return;
    }
    let mut clients = state.clients.lock().await;
    clients.remove(username);
    // ... logic notify system leave (giữ nguyên của bạn) ...
}

async fn send_history(
    state: &AppState,
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    room: &str,
) {
    match db::get_room_history(room).await {
        Ok(messages) => {
            for msg in messages {
                state.message_store.add_message(msg.clone()).await;
                let _ = client_tx.send(msg);
            }
        }
        Err(e) => eprintln!("Failed to load room history: {}", e),
    }
}

async fn send_all_private_chat_history(
    state: &AppState,
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    username: &str,
) {
    match db::get_private_history(username).await {
        Ok(messages) => {
            for msg in messages {
                state.message_store.add_message(msg.clone()).await;
                let _ = client_tx.send(msg);
            }
        }
        Err(e) => eprintln!("Failed to load private chat history: {}", e),
    }
}

async fn store_room_message(state: &AppState, msg: ChatMessage) {
    state.message_store.add_message(msg.clone()).await;
    let mut rooms = state.rooms.lock().await;
    rooms.entry(msg.room.clone()).or_default().push(msg);
}

async fn send_to_room_members(state: &AppState, room: &str, msg: &ChatMessage) {
    let members = {
        let rm = state.room_members.lock().await;
        rm.get(room)
            .map(|m| m.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default()
    };
    send_to_users(state, members, msg).await;
}

async fn send_to_private_participants(state: &AppState, msg: &ChatMessage) {
    send_to_users(state, private_room_participants(&msg.room), msg).await;
}

async fn send_to_users(state: &AppState, recipients: Vec<String>, msg: &ChatMessage) {
    let clients = state.clients.lock().await;
    for r in recipients {
        if let Some(tx) = clients.get(&r) {
            let _ = tx.send(msg.clone());
        }
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
        ..Default::default()
    });
}

fn send_system(client_tx: &mpsc::UnboundedSender<ChatMessage>, content: &str) {
    let _ = client_tx.send(ChatMessage {
        msg_type: "system".into(),
        username: "SYSTEM".into(),
        content: content.into(),
        room: "general".into(),
        ..Default::default()
    });
}

fn send_json<T: serde::Serialize>(
    client_tx: &mpsc::UnboundedSender<ChatMessage>,
    msg_type: &str,
    payload: &T,
) {
    match serde_json::to_string(payload) {
        Ok(content) => {
            let _ = client_tx.send(ChatMessage {
                msg_type: msg_type.into(),
                username: "SYSTEM".into(),
                content,
                room: "general".into(),
                ..Default::default()
            });
        }
        Err(_) => send_local_error(client_tx, "Could not serialize response"),
    }
}

fn is_private_room(room: &str) -> bool {
    room.starts_with("dm:")
}
fn group_room_name(room: &str) -> Option<&str> {
    room.strip_prefix("group:")
        .filter(|name| !name.trim().is_empty())
}
fn private_room_contains_user(room: &str, user: &str) -> bool {
    private_room_participants(room).contains(&user.to_string())
}
fn private_room_participants(room: &str) -> Vec<String> {
    room.strip_prefix("dm:")
        .map(|r| {
            r.split(':')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}
