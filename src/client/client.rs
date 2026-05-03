use crate::chat::message::ChatMessage;
use futures_util::{Sink, SinkExt, StreamExt};
use std::{
    collections::HashSet,
    io::{self, Write},
};
use tokio::task;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub async fn start_client() -> Result<(), Box<dyn std::error::Error>> {
    let username = read_input("Enter username: ")?;
    // THÊM: Nhập mật khẩu để gửi lên server xác thực
    let password = read_input("Enter password: ")?;
    
    let general_room = "general".to_string();
    let server_url = "ws://127.0.0.1:3000/ws";
    let (socket, response) = connect_async(server_url).await?;
    let (mut write, mut read) = socket.split();

    println!("Connected to {}", server_url);
    println!("Handshake status: {}", response.status());
    println!("Chat commands:");
    println!("  /dm <username> <message>  Send a private message");
    println!("  /join <room>               Join a public room");
    println!("  /switch <room|@user>       Change the active conversation");
    println!("  /leave                     Leave the active public room");
    println!("  /quit                      Exit");

    let mut open_rooms = HashSet::from([general_room.clone()]);
    let mut active_room = general_room.clone();

    // Gửi tin nhắn type "login" kèm password ngay khi kết nối
    send_login(&mut write, &username, &password).await?;
    // Sau đó join vào room mặc định
    send_join(&mut write, &username, &general_room).await?;

    let receive_username = username.clone();
    let receive_task = tokio::spawn(async move {
        while let Some(frame) = read.next().await {
            match frame {
                Ok(Message::Text(text)) => match serde_json::from_str::<ChatMessage>(&text) {
                    Ok(msg) => print_incoming_message(&receive_username, &msg),
                    Err(error) => eprintln!("Parse error: {}", error),
                },
                Ok(Message::Close(_)) => break,
                Err(error) => {
                    eprintln!("Receive error: {}", error);
                    break;
                }
                _ => {}
            }
        }
    });

    loop {
        let prompt = format!("{} [{}]> ", username, display_room(&username, &active_room));
        let input = task::spawn_blocking(move || read_input(&prompt)).await??;

        if input == "/quit" {
            break;
        }

        if input.is_empty() {
            continue;
        }

        if let Some(dm) = parse_dm_command(&username, &input) {
            if open_rooms.insert(dm.room.clone()) {
                send_join(&mut write, &username, &dm.room).await?;
            }

            send_chat(&mut write, &username, &dm.room, &dm.message).await?;
            active_room = dm.room;
            continue;
        }

        if let Some(room) = input.strip_prefix("/join ") {
            let room = room.trim();
            if room.is_empty() {
                println!("Room name cannot be empty");
                continue;
            }

            if room.starts_with('@') {
                println!("Use /switch @username or /dm <username> <message> for private chat");
                continue;
            }

            if open_rooms.insert(room.to_string()) {
                send_join(&mut write, &username, room).await?;
            }

            active_room = room.to_string();
            println!("Switched to {}", display_room(&username, &active_room));
            continue;
        }

        if let Some(target) = input.strip_prefix("/switch ") {
            let target = target.trim();
            if target.is_empty() {
                println!("Conversation name cannot be empty");
                continue;
            }

            let room = normalize_conversation(&username, target);

            if open_rooms.insert(room.clone()) {
                send_join(&mut write, &username, &room).await?;
            }

            active_room = room;
            println!("Switched to {}", display_room(&username, &active_room));
            continue;
        }

        if input == "/leave" {
            if active_room == general_room {
                println!("The general room stays available. Use /switch to move elsewhere.");
                continue;
            }

            if active_room.starts_with("dm:") {
                open_rooms.remove(&active_room);
                println!("Closed {}", display_room(&username, &active_room));
                active_room = general_room.clone();
                continue;
            }

            send_leave(&mut write, &username, &active_room).await?;
            open_rooms.remove(&active_room);
            println!("Left {}", active_room);
            active_room = general_room.clone();
            continue;
        }

        if !open_rooms.contains(&active_room) {
            send_join(&mut write, &username, &active_room).await?;
            open_rooms.insert(active_room.clone());
        }

        send_chat(&mut write, &username, &active_room, &input).await?;
    }

    for room in open_rooms {
        if !room.starts_with("dm:") {
            let _ = send_leave(&mut write, &username, &room).await;
        }
    }

    let _ = write.close().await;
    let _ = receive_task.await;
    println!("Client disconnected");

    Ok(())
}

// THÊM: Hàm gửi tin nhắn đăng nhập
async fn send_login<S>(write: &mut S, username: &str, password: &str) -> Result<(), Box<dyn std::error::Error>>
where S: Sink<Message> + Unpin, S::Error: std::error::Error + Send + Sync + 'static,
{
    let msg = ChatMessage {
        msg_type: "login".into(),
        username: username.into(),
        password: password.into(), // Gửi mật khẩu thật
        content: String::new(),
        room: String::new(),
        target: String::new(),
        users: vec![],
    };
    write.send(Message::Text(serde_json::to_string(&msg)?.into())).await?;
    Ok(())
}

async fn send_join<S>(write: &mut S, username: &str, room: &str) -> Result<(), Box<dyn std::error::Error>>
where S: Sink<Message> + Unpin, S::Error: std::error::Error + Send + Sync + 'static,
{
    let join_msg = ChatMessage {
        msg_type: "join".into(),
        username: username.into(),
        password: String::new(), // Không cần mật khẩu khi join room
        content: String::new(),
        room: room.into(),
        target: String::new(),
        users: vec![],
    };
    write.send(Message::Text(serde_json::to_string(&join_msg)?.into())).await?;
    Ok(())
}

async fn send_leave<S>(write: &mut S, username: &str, room: &str) -> Result<(), Box<dyn std::error::Error>>
where S: Sink<Message> + Unpin, S::Error: std::error::Error + Send + Sync + 'static,
{
    let leave_msg = ChatMessage {
        msg_type: "leave".into(),
        username: username.into(),
        password: String::new(),
        content: String::new(),
        room: room.into(),
        target: String::new(),
        users: vec![],
    };
    write.send(Message::Text(serde_json::to_string(&leave_msg)?.into())).await?;
    Ok(())
}

async fn send_chat<S>(write: &mut S, username: &str, room: &str, content: &str) -> Result<(), Box<dyn std::error::Error>>
where S: Sink<Message> + Unpin, S::Error: std::error::Error + Send + Sync + 'static,
{
    let target = other_private_participant(username, room);
    let msg = ChatMessage {
        msg_type: "message".into(),
        username: username.into(),
        password: String::new(),
        content: content.into(),
        room: room.into(),
        target,
        users: vec![],
    };
    write.send(Message::Text(serde_json::to_string(&msg)?.into())).await?;
    Ok(())
}

// Các hàm helper giữ nguyên như cũ...
fn print_incoming_message(current_user: &str, msg: &ChatMessage) {
    match msg.msg_type.as_str() {
        "system" => println!("[{}] {}", display_room(current_user, &msg.room), msg.content),
        "error" => println!("[ERROR] {}", msg.content),
        _ => {
            let conversation = display_room(current_user, &msg.room);
            println!("[{}] {}: {}", conversation, msg.username, msg.content);
        }
    }
}

fn parse_dm_command(username: &str, input: &str) -> Option<DirectMessage> {
    let command = input.strip_prefix("/dm ")?;
    let mut parts = command.splitn(2, ' ');
    let target = parts.next()?.trim();
    let message = parts.next()?.trim();

    if target.is_empty() || message.is_empty() || target == username {
        return None;
    }

    Some(DirectMessage {
        room: private_room_id(username, target),
        message: message.to_string(),
    })
}

fn normalize_conversation(username: &str, target: &str) -> String {
    if let Some(private_target) = target.strip_prefix('@') {
        return private_room_id(username, private_target.trim());
    }
    target.to_string()
}

fn private_room_id(left: &str, right: &str) -> String {
    let mut participants = [left.trim().to_string(), right.trim().to_string()];
    participants.sort();
    format!("dm:{}:{}", participants[0], participants[1])
}

fn other_private_participant(username: &str, room: &str) -> String {
    room.strip_prefix("dm:")
        .map(|rest| {
            rest.split(':')
                .find(|participant| *participant != username)
                .unwrap_or_default()
                .to_string()
        })
        .unwrap_or_default()
}

fn display_room(username: &str, room: &str) -> String {
    if let Some(target) = room.strip_prefix("dm:") {
        let other = target
            .split(':')
            .find(|participant| *participant != username)
            .unwrap_or(username);
        return format!("@{}", other);
    }
    format!("#{}", room)
}

fn read_input(prompt: &str) -> Result<String, io::Error> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

struct DirectMessage {
    room: String,
    message: String,
}