use crate::chat::message::{ChatMessage, PrivateMessage};
use futures_util::{SinkExt, StreamExt};
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};
use tokio::task;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ActivePrivateRoom {
    room_id: String,
    partner: String,
}

#[derive(Debug, Default)]
struct ClientPrivateState {
    pending_sent_invites: Vec<String>,
    pending_received_invites: Vec<String>,
    active_private_room: Option<ActivePrivateRoom>,
}

#[derive(Debug)]
enum ClientCommand {
    Invite(String),
    Accept(String),
    Decline(String),
    Leave,
    Quit,
    Text(String),
}

pub async fn start_client() -> Result<(), Box<dyn std::error::Error>> {
    let username = read_input("Enter username: ")?;

    let server_url = "ws://127.0.0.1:3000/ws";
    let (socket, response) = connect_async(server_url).await?;
    let (mut write, mut read) = socket.split();
    let private_state = Arc::new(Mutex::new(ClientPrivateState::default()));

    println!("Connected to {}", server_url);
    println!("Handshake status: {}", response.status());
    println!("Type messages and press Enter. Type /quit to exit.");
    println!("Private commands: /invite <username>, /accept <username>, /decline <username>, /leave");

    let join_message = ChatMessage {
        username: username.clone(),
        content: "__join__".to_string(),
    };
    let serialized_join = serde_json::to_string(&join_message)?;
    write.send(Message::Text(serialized_join.into())).await?;

    let receive_state = Arc::clone(&private_state);
    let receive_username = username.clone();
    let receive_task = tokio::spawn(async move {
        while let Some(frame) = read.next().await {
            match frame {
                Ok(Message::Text(text)) => {
                    if let Ok(received_message) = serde_json::from_str::<ChatMessage>(&text) {
                        println!("{}: {}", received_message.username, received_message.content);
                    } else if let Ok(private_message) = serde_json::from_str::<PrivateMessage>(&text)
                    {
                        handle_private_event(&receive_state, &receive_username, private_message);
                    } else {
                        eprintln!("Invalid message from server");
                    }
                }
                Ok(Message::Close(close_frame)) => {
                    println!("Server closed connection: {:?}", close_frame);
                    break;
                }
                Ok(_) => {}
                Err(error) => {
                    eprintln!("Receive error: {}", error);
                    break;
                }
            }
        }
    });

    loop {
        render_private_sidebar(&private_state);

        let prompt = build_prompt(&private_state, &username);
        let content = task::spawn_blocking(move || read_input(&prompt)).await??;
        let command = match parse_client_command(&content) {
            Ok(command) => command,
            Err(error) => {
                println!("[SYSTEM] {}", error);
                continue;
            }
        };

        match command {
            ClientCommand::Quit => break,
            ClientCommand::Invite(target) => {
                let message = PrivateMessage::PrivateInvite {
                    from: username.clone(),
                    to: target.clone(),
                };
                let serialized_message = serde_json::to_string(&message)?;

                if write.send(Message::Text(serialized_message.into())).await.is_err() {
                    println!("Disconnected from server");
                    break;
                }

                remember_sent_invite(&private_state, &target);
            }
            ClientCommand::Accept(inviter) => {
                let message = PrivateMessage::PrivateInviteAccepted {
                    from: username.clone(),
                    to: inviter,
                    room_id: String::new(),
                };
                let serialized_message = serde_json::to_string(&message)?;

                if write.send(Message::Text(serialized_message.into())).await.is_err() {
                    println!("Disconnected from server");
                    break;
                }
            }
            ClientCommand::Decline(inviter) => {
                let message = PrivateMessage::PrivateInviteDeclined {
                    from: username.clone(),
                    to: inviter,
                };
                let serialized_message = serde_json::to_string(&message)?;

                if write.send(Message::Text(serialized_message.into())).await.is_err() {
                    println!("Disconnected from server");
                    break;
                }
            }
            ClientCommand::Leave => {
                let active_room = {
                    private_state
                        .lock()
                        .unwrap()
                        .active_private_room
                        .clone()
                };

                if let Some(room) = active_room {
                    let message = PrivateMessage::LeavePrivateRoom {
                        from: username.clone(),
                        room_id: room.room_id,
                    };
                    let serialized_message = serde_json::to_string(&message)?;

                    if write.send(Message::Text(serialized_message.into())).await.is_err() {
                        println!("Disconnected from server");
                        break;
                    }
                } else {
                    println!("[SYSTEM] You are not in a private chat.");
                }
            }
            ClientCommand::Text(text) => {
                if text.is_empty() {
                    continue;
                }

                let active_room = {
                    private_state
                        .lock()
                        .unwrap()
                        .active_private_room
                        .clone()
                };

                if let Some(room) = active_room {
                    let message = PrivateMessage::PrivateChat {
                        from: username.clone(),
                        room_id: room.room_id,
                        content: text,
                    };
                    let serialized_message = serde_json::to_string(&message)?;

                    if write.send(Message::Text(serialized_message.into())).await.is_err() {
                        println!("Disconnected from server");
                        break;
                    }
                } else {
                    let message = ChatMessage {
                        username: username.clone(),
                        content: text,
                    };
                    let serialized_message = serde_json::to_string(&message)?;

                    if write.send(Message::Text(serialized_message.into())).await.is_err() {
                        println!("Disconnected from server");
                        break;
                    }
                }
            }
        }
    }

    let _ = write.close().await;
    let _ = receive_task.await;
    println!("Client disconnected");

    Ok(())
}

fn handle_private_event(
    private_state: &Arc<Mutex<ClientPrivateState>>,
    username: &str,
    message: PrivateMessage,
) {
    match message {
        PrivateMessage::PrivateInvite { from, to } => {
            if to == username {
                let mut state = private_state.lock().unwrap();
                push_unique(&mut state.pending_received_invites, from.clone());
                println!("[PRIVATE] {} invited you to a private chat.", from);
            }
        }
        PrivateMessage::PrivateInviteAccepted { from, to, room_id } => {
            if from == username || to == username {
                let partner = if from == username { to.clone() } else { from.clone() };
                let mut state = private_state.lock().unwrap();
                state.pending_sent_invites.retain(|name| name != &partner);
                state.pending_received_invites.retain(|name| name != &partner);
                state.active_private_room = Some(ActivePrivateRoom {
                    room_id,
                    partner: partner.clone(),
                });
                println!("[PRIVATE] Private chat active with {}.", partner);
            }
        }
        PrivateMessage::PrivateInviteDeclined { from, to } => {
            if to == username {
                let mut state = private_state.lock().unwrap();
                state.pending_sent_invites.retain(|name| name != &from);
                println!("[PRIVATE] {} declined your private invite.", from);
            }
        }
        PrivateMessage::PrivateChat {
            from,
            room_id,
            content,
        } => {
            let active_room = {
                private_state
                    .lock()
                    .unwrap()
                    .active_private_room
                    .clone()
            };

            if let Some(active_room) = active_room {
                if active_room.room_id == room_id {
                    println!("[PM:{}] {}", from, content);
                }
            }
        }
        PrivateMessage::LeavePrivateRoom { from, room_id } => {
            let mut state = private_state.lock().unwrap();
            let should_clear = state
                .active_private_room
                .as_ref()
                .map(|room| room.room_id == room_id)
                .unwrap_or(false);

            if should_clear {
                let partner = state
                    .active_private_room
                    .as_ref()
                    .map(|room| room.partner.clone())
                    .unwrap_or_default();
                state.active_private_room = None;

                if from == username {
                    println!("[PRIVATE] You left the private chat with {}.", partner);
                } else {
                    println!("[PRIVATE] {} left the private chat.", from);
                }
            }
        }
        PrivateMessage::System { content, .. } => {
            println!("[SYSTEM] {}", content);
        }
    }
}

fn remember_sent_invite(private_state: &Arc<Mutex<ClientPrivateState>>, target: &str) {
    let mut state = private_state.lock().unwrap();
    push_unique(&mut state.pending_sent_invites, target.to_string());
}

fn push_unique(items: &mut Vec<String>, value: String) {
    if !items.iter().any(|item| item == &value) {
        items.push(value);
    }
}

fn render_private_sidebar(private_state: &Arc<Mutex<ClientPrivateState>>) {
    let state = private_state.lock().unwrap();

    println!("---------------- Private ----------------");
    if state.pending_sent_invites.is_empty() {
        println!("Invited: -");
    } else {
        println!("Invited: {}", state.pending_sent_invites.join(", "));
    }

    if state.pending_received_invites.is_empty() {
        println!("Incoming: -");
    } else {
        println!("Incoming: {}", state.pending_received_invites.join(", "));
    }

    match &state.active_private_room {
        Some(room) => println!("Active: {}", room.partner),
        None => println!("Active: -"),
    }
    println!("-----------------------------------------");
}

fn build_prompt(private_state: &Arc<Mutex<ClientPrivateState>>, username: &str) -> String {
    let state = private_state.lock().unwrap();
    if let Some(room) = &state.active_private_room {
        format!("[pm:{}]> ", room.partner)
    } else {
        format!("{}> ", username)
    }
}

fn parse_client_command(input: &str) -> Result<ClientCommand, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(ClientCommand::Text(String::new()));
    }

    if trimmed == "/quit" {
        return Ok(ClientCommand::Quit);
    }

    if trimmed == "/leave" {
        return Ok(ClientCommand::Leave);
    }

    for command in ["/invite", "/accept", "/decline"] {
        if let Some(rest) = trimmed.strip_prefix(command) {
            let target = rest.trim();
            if target.is_empty() {
                return Err(format!("Usage: {} <username>", command));
            }

            return match command {
                "/invite" => Ok(ClientCommand::Invite(target.to_string())),
                "/accept" => Ok(ClientCommand::Accept(target.to_string())),
                "/decline" => Ok(ClientCommand::Decline(target.to_string())),
                _ => unreachable!(),
            };
        }
    }

    Ok(ClientCommand::Text(trimmed.to_string()))
}

fn read_input(prompt: &str) -> Result<String, io::Error> {
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_private_commands() {
        assert!(matches!(
            parse_client_command("/invite bob").unwrap(),
            ClientCommand::Invite(user) if user == "bob"
        ));
        assert!(matches!(
            parse_client_command("/accept alice").unwrap(),
            ClientCommand::Accept(user) if user == "alice"
        ));
        assert!(matches!(
            parse_client_command("/decline charlie").unwrap(),
            ClientCommand::Decline(user) if user == "charlie"
        ));
        assert!(matches!(
            parse_client_command("/leave").unwrap(),
            ClientCommand::Leave
        ));
    }

    #[test]
    fn parse_command_requires_username() {
        assert_eq!(
            parse_client_command("/invite").unwrap_err(),
            "Usage: /invite <username>"
        );
        assert_eq!(
            parse_client_command("/accept ").unwrap_err(),
            "Usage: /accept <username>"
        );
        assert_eq!(
            parse_client_command("/decline").unwrap_err(),
            "Usage: /decline <username>"
        );
    }

    #[test]
    fn plain_text_is_preserved() {
        assert!(matches!(
            parse_client_command("hello world").unwrap(),
            ClientCommand::Text(text) if text == "hello world"
        ));
    }
}
