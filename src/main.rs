mod server;

#[tokio::main]
async fn main() {
    println!("Starting Rust Chat Server...");

    server::start_server().await;
}
