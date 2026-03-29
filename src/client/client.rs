use futures_util::{SinkExt, StreamExt};
use crate::chat::message::ChatMessage;
use std::io::{self, Write};
use tokio::task;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub async fn start_client() -> Result<(), Box<dyn std::error::Error>> {
    let username = read_input("Enter username: ")?;

    let server_url = "ws://127.0.0.1:3000/ws";
    let (socket, response) = connect_async(server_url).await?;
    let (mut write, mut read) = socket.split();

    println!("Connected to {}", server_url);
    println!("Handshake status: {}", response.status());
    println!("Type messages and press Enter. Type /quit to exit.");

    let receive_task = tokio::spawn(async move {
        while let Some(frame) = read.next().await {
            match frame {
                Ok(Message::Text(text)) => {
                    match serde_json::from_str::<ChatMessage>(&text) {
                        Ok(received_message) => {
                            println!(
                                "{}: {}",
                                received_message.username, received_message.content
                            );
                        }
                        Err(error) => eprintln!("Invalid message from server: {}", error),
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
        let prompt = format!("{}> ", username);
        let content = task::spawn_blocking(move || read_input(&prompt)).await??;

        if content == "/quit" {
            break;
        }

        if content.is_empty() {
            continue;
        }

        let message = ChatMessage {
            username: username.clone(),
            content,
        };
        let serialized_message = serde_json::to_string(&message)?;

        if write.send(Message::Text(serialized_message.into())).await.is_err() {
            println!("Disconnected from server");
            break;
        }
    }

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