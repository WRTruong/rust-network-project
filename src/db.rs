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