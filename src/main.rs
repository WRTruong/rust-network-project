mod chat;
mod client;
mod server;
mod db;
mod auth;
#[tokio::main]
async fn main() {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    let db_host = std::env::var("DB_HOST").unwrap_or_else(|_| "(not set)".to_string());
    let db_port = std::env::var("DB_PORT").unwrap_or_else(|_| "1433".to_string());
    println!("DB config: host={} port={}", db_host, db_port);

    // Graceful Ctrl+C: exit with code 0 so Windows doesn't report 0xc000013a
    tokio::spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            std::process::exit(0);
        }
    });

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
