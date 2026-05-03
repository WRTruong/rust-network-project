use bcrypt::{hash, verify, DEFAULT_COST};
use tiberius::Client;
use tokio::net::TcpStream;
use tokio_util::compat::Compat;

// Hàm đăng ký: Mã hóa mật khẩu và lưu vào SQL Server
pub async fn register(db: &mut Client<Compat<TcpStream>>, user: &str, pass: &str) -> bool {
    // Mã hóa mật khẩu bằng bcrypt
    let hashed = match hash(pass, DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => return false,
    };

    // Thực hiện lệnh INSERT
    let res = db.execute(
        "INSERT INTO Users (username, password) VALUES (@P1, @P2)",
        &[&user, &hashed],
    ).await;

    res.is_ok()
}

// Hàm đăng nhập: Kiểm tra username và so khớp hash mật khẩu
pub async fn login(db: &mut Client<Compat<TcpStream>>, user: &str, pass: &str) -> bool {
    // 1. Lấy password_hash từ database dựa trên username
    let res = db.query(
        "SELECT password FROM Users WHERE username = @P1",
        &[&user]
    ).await;

    // Sửa lỗi cảnh báo 'mut' không cần thiết ở dòng 31 trong ảnh
    if let Ok(stream) = res {
        let stream = stream; // Chỉ gán mut khi cần thiết để gọi into_row
        
        // 2. Lấy dòng dữ liệu đầu tiên (nếu có)
        if let Ok(Some(row)) = stream.into_row().await {
            // Lấy giá trị hash từ cột đầu tiên (index 0)
            let db_hash: &str = row.get(0).unwrap_or("");
            
            // 3. So sánh mật khẩu bằng bcrypt::verify
            return verify(pass, db_hash).unwrap_or(false);
        }
    }

    false
}