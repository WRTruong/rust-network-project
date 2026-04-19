# Rust Network Project

Ứng dụng chat đơn giản bằng Rust sử dụng WebSocket, hỗ trợ:

- chat room công khai
- private chat giữa 2 user
- terminal client
- giao diện web trong `index.html`

## Tính năng hiện tại

- Server WebSocket chạy tại `127.0.0.1:3000`
- Chat room công khai, mặc định vào room `general`
- Private chat giữa 2 user theo username
- Lưu lịch sử theo từng hội thoại
- Hiển thị join/leave với public room
- Username phải là duy nhất trong mỗi phiên kết nối

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

## Chạy server

Mở terminal trong thư mục project và chạy:

```powershell
cargo run
```

Khi chạy thành công, màn hình hiện:

```text
Starting Rust Chat Server...
Server running at ws://127.0.0.1:3000
```

## Dùng terminal client

Mở terminal khác và chạy:

```powershell
cargo run -- client
```

Sau đó nhập username. Client sẽ tự động vào room `general`.

Khi kết nối xong, terminal hiển thị các lệnh hỗ trợ:

```text
Chat commands:
  /dm <username> <message>  Send a private message
  /join <room>              Join a public room
  /switch <room|@user>      Change the active conversation
  /leave                    Leave the active public room
  /quit                     Exit
```

### Chat trong room công khai

Ví dụ:

```text
Enter username: alice
Connected to ws://127.0.0.1:3000/ws
Handshake status: 101 Switching Protocols
alice [#general]> hello everyone
[#general] alice: hello everyone
```

Tạo hoặc vào room mới:

```text
alice [#general]> /join rust
Switched to #rust
alice [#rust]> xin chào room rust
```

Chuyển lại room khác:

```text
/switch general
/switch rust
```

Rời room hiện tại:

```text
/leave
```

Lưu ý:

- `general` là room mặc định, không cần tạo thủ công
- `/leave` chỉ áp dụng với public room

### Chat private giữa 2 user

Gửi tin nhắn riêng:

```text
/dm bob chào Bob
```

Sau lệnh này, client sẽ tạo hội thoại private với `bob` và chuyển sang hội thoại đó.

Nếu muốn mở lại hội thoại private mà chưa gửi tin ngay:

```text
/switch @bob
```

Ví dụ:

```text
alice [#general]> /dm bob hello Bob
alice [@bob]> bạn đang ở đó không?
[@bob] alice: hello Bob
[@bob] alice: bạn đang ở đó không?
```

## Dùng giao diện web

Project có file giao diện tại `index.html`.

### Cách mở

Có 2 cách:

1. Mở trực tiếp `index.html` trong browser
2. dùng live server

Sau khi mở trang:

- nhập username
- bấm `Kết nối`
- ứng dụng sẽ tự động vào room `general`

### Chat room trên web

- Sidebar bên trái là danh sách hội thoại
- Bấm `New` và nhập tên room để tạo/vào room mới
- Chọn room trong sidebar để chuyển hội thoại
- Bấm `Leave` để rời room hiện tại

Ví dụ tạo room:

```text
rust
```

### Private chat trên web

Từ giao diện web, bấm `New` và nhập:

```text
@bob
```

Hệ thống sẽ tạo private chat giữa bạn và `bob`, sau đó hiện trong sidebar dưới dạng một hội thoại riêng.

Lưu ý:

- private chat được nhận diện bằng `@username`
- không cần tạo room thủ công cho private chat
- lịch sử private chat được lưu riêng với public room

## Test nhanh room và private

1. Chạy server:

```powershell
cargo run
```

2. Mở 2 client, có thể là:

- 2 terminal client
- 1 terminal client và 1 browser
- 2 browser với 2 username khác nhau

3. Test public room:

- `alice` vào `general` hoặc `/join rust`
- `bob` vào cùng room đó
- gửi tin và kiểm tra cả 2 bên đều nhận được

4. Test private chat:

- từ `alice`, gửi `/dm bob hello`
- hoặc trên web bấm `New` và nhập `@bob`
- kiểm tra chỉ `alice` và `bob` nhận được tin nhắn đó

## Cấu trúc message

Server và client đang trao đổi JSON theo cấu trúc:

```rust
ChatMessage {
    msg_type: String,
    username: String,
    content: String,
    room: String,
    target: String,
    users: Vec<String>,
}
```

Ý nghĩa cơ bản:

- `msg_type`: `join`, `leave`, `message`, `system`, `error`
- `room`: tên room công khai hoặc id private chat
- `target`: user đích khi cần xử lý private chat

Private chat được lưu theo dạng:

```text
dm:alice:bob
```

Trong đó 2 username được sắp xếp ổn định để cả 2 phía cùng dùng chung một hội thoại.

## Cấu trúc thư mục

```text
src/
|-- chat/
|   |-- mod.rs
|   `-- message.rs
|-- client/
|   |-- mod.rs
|   `-- client.rs
|-- server/
|   |-- mod.rs
|   `-- websocket.rs
`-- main.rs
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
