# Rust Network Project

Project này xây dựng ứng dụng chat đơn giản bằng Rust sử dụng WebSocket.

## Chức năng hiện có

- WebSocket server chạy tại `127.0.0.1:3000`
- WebSocket client kết nối tới server qua terminal
- Gửi và nhận dữ liệu theo dạng `ChatMessage { username, content }`
- Broadcast tin nhắn tới nhiều client đang kết nối

## Yêu cầu

Cần cài sẵn:

- Rust
- Cargo

Kiểm tra phiên bản:

```powershell
rustc --version
cargo --version
```

## Cài đặt

Clone project và di chuyển vào thư mục làm việc:

```powershell
git clone <repo-url>
cd rust-network-project
```

## Cách chạy server

Mở terminal trong thư mục project và chạy:

```powershell
cargo run
```

Khi chạy thành công, màn hình sẽ hiển thị tương tự:

```text
Starting Rust Chat Server...
Server running on 127.0.0.1:3000
```

## Cách chạy client

Mở một terminal khác và chạy:

```powershell
cargo run -- client
```

Sau đó nhập `username`, rồi nhập tin nhắn trực tiếp từ terminal.

Ví dụ:

```text
Enter username: alice
Connected to ws://127.0.0.1:3000/ws
Handshake status: 101 Switching Protocols
Type messages and press Enter. Type /quit to exit.
alice> hello
alice: hello
```

Để thoát client, nhập:

```text
/quit
```

## Test nhiều client

Để kiểm tra tính năng broadcast:

1. Mở terminal thứ nhất và chạy server:

```powershell
cargo run
```

2. Mở 2 hoặc nhiều terminal khác và chạy client:

```powershell
cargo run -- client
```

3. Nhập các `username` khác nhau
5. Quan sát các client còn lại sẽ nhận được cùng tin nhắn

## Message format

Project đang sử dụng cấu trúc dữ liệu:

```rust
ChatMessage {
    username: String,
    content: String,
}
```

Client và server trao đổi dữ liệu bằng JSON.

Ví dụ:

```json
{
  "username": "alice",
  "content": "hello"
}
```

## Cấu trúc thư mục

```text
src/
├── chat/
│   ├── mod.rs
│   └── message.rs
├── client/
│   ├── mod.rs
│   └── client.rs
├── server/
│   ├── mod.rs
│   └── websocket.rs
└── main.rs
```

## Lệnh hữu ích

Kiểm tra project:

```powershell
cargo check
```

Build project:

```powershell
cargo build
```

## Hướng phát triển tiếp

- thêm thông báo user join hoặc leave
- không broadcast lại tin nhắn cho chính người gửi
- lưu lịch sử chat
- thêm giao diện người dùng
