use tiberius::{Client, Config, AuthMethod};
use tokio::net::TcpStream;
use tokio_util::compat::{TokioAsyncWriteCompatExt, Compat};

// Đặt pub để websocket.rs gọi được
pub async fn get_db_client() -> Result<Client<Compat<TcpStream>>, Box<dyn std::error::Error + Send + Sync>> {
    let mut config = Config::new();
    config.host("127.0.0.1"); 
    config.port(1433);
    config.database("ChatDB");
    config.authentication(AuthMethod::sql_server("sa", "123456"));
    config.trust_cert(); 

    let tcp = TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;

    let client = Client::connect(config, tcp.compat_write()).await?;
    Ok(client)
}

// Lấy 50 tin nhắn gần nhất của một room (theo thứ tự thời gian tăng dần)
pub async fn get_room_history(room: &str) -> Result<Vec<(String, String, String)>, Box<dyn std::error::Error + Send + Sync>> {
    let mut client = get_db_client().await?;
    let stream = client.query(
        "SELECT sender, message, room FROM (SELECT TOP 50 sender, message, room, created_at FROM ChatHistory WHERE room = @P1 ORDER BY created_at DESC) AS sub ORDER BY created_at ASC",
        &[&room],
    ).await?;

    let rows = stream.into_first_result().await?;
    let mut messages = Vec::new();
    for row in rows {
        let sender: &str = row.get(0).unwrap_or("");
        let message: &str = row.get(1).unwrap_or("");
        let room_val: &str = row.get(2).unwrap_or("");
        messages.push((sender.to_string(), message.to_string(), room_val.to_string()));
    }
    Ok(messages)
}

// Lấy tất cả tin nhắn DM liên quan đến một user
pub async fn get_private_history(username: &str) -> Result<Vec<(String, String, String)>, Box<dyn std::error::Error + Send + Sync>> {
    let mut client = get_db_client().await?;
    let stream = client.query(
        "SELECT sender, message, room FROM ChatHistory WHERE (room LIKE 'dm:' + @P1 + ':%' OR room LIKE 'dm:%:' + @P2) ORDER BY created_at ASC",
        &[&username, &username],
    ).await?;

    let rows = stream.into_first_result().await?;
    let mut messages = Vec::new();
    for row in rows {
        let sender: &str = row.get(0).unwrap_or("");
        let message: &str = row.get(1).unwrap_or("");
        let room_val: &str = row.get(2).unwrap_or("");
        messages.push((sender.to_string(), message.to_string(), room_val.to_string()));
    }
    Ok(messages)
}