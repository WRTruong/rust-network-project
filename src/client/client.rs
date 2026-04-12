use futures_util::{SinkExt, StreamExt};
use crate::chat::message::ChatMessage;
use std::io::{self, Write};
use tokio::task;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub async fn start_client() -> Result<(), Box<dyn std::error::Error>> {
    let username = read_input("Enter username: ")?;
    let room = "general".to_string();
    let server_url = "ws://127.0.0.1:3000/ws";
    let (socket, response) = connect_async(server_url).await?;
    let (mut write, mut read) = socket.split();

    println!("Connected to {}", server_url);
    println!("Handshake status: {}", response.status());
    println!("Type messages and press Enter. Type /quit to exit.");

    let join_msg = ChatMessage {
        msg_type: "join".into(),
        username: username.clone(),
        content: "".into(),
        room: room.clone(),
    };

    write
        .send(Message::Text(serde_json::to_string(&join_msg)?.into()))
        .await?;

    let receive_task = tokio::spawn(async move {
        while let Some(frame) = read.next().await {
            match frame {
                Ok(Message::Text(text)) => {
                    match serde_json::from_str::<ChatMessage>(&text) {
                        Ok(msg) => {
                            if msg.msg_type == "system" {
                                println!("[SYSTEM] {}", msg.content);
                            } else {
                                println!("{}: {}", msg.username, msg.content);
                            }
                        }
                        Err(e) => eprintln!("Parse error: {}", e),
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(e) => {
                    eprintln!("Receive error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    loop {
        let prompt = format!("{}> ", username);
        let content = task::spawn_blocking(move || read_input(&prompt)).await??;

        if content == "/quit" {
            break;
        }

        if content.is_empty() {
            continue;
        }

        let msg = ChatMessage {
            msg_type: "message".into(),
            username: username.clone(),
            content,
            room: room.clone(),
        };

        let serialized = serde_json::to_string(&msg)?;
        
        if write.send(Message::Text(serialized.into())).await.is_err() {
            println!("Disconnected from server");
            break;
        }
    }

    let leave_msg: ChatMessage = ChatMessage {
        msg_type: "leave".into(),
        username: username.clone(),
        content: "".into(),
        room: room.clone(),
    };

    let _ = write
        .send(Message::Text(serde_json::to_string(&leave_msg)?.into()))
        .await;

    let _ = write.close().await;
    let _ = receive_task.await;
    println!("Client disconnected");

    Ok(())
}

fn read_input(prompt: &str) -> Result<String, io::Error> {
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_string())
}