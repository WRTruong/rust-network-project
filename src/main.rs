mod chat;
mod client;
mod server;

#[tokio::main]
async fn main() {
    let mode = std::env::args().nth(1);

    match mode.as_deref() {
        Some("client") => {
            println!("Starting Rust Chat Client...");

            if let Err(error) = client::client::start_client().await {
                eprintln!("Client error: {}", error);
            }
        }
        _ => {
            println!("Starting Rust Chat Server...");
            server::start_server().await;
        }
    }
}
