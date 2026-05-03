# API Summary - Media & Message Management Features

## Files Được Sửa Đổi

### 1. `src/chat/message.rs`
**Thay đổi:** Mở rộng `ChatMessage` struct với các field mới
- Thêm `message_id: String` - Unique identifier cho mỗi tin nhắn
- Thêm `timestamp: u64` - UNIX epoch time
- Thêm `media: Option<MediaInfo>` - Thông tin media (hình ảnh, video, file)
- Thêm `MediaInfo` struct với các field: media_type, file_url, file_name, file_size, mime_type
- Thêm `ChatMessage::new()` constructor
- Thêm `with_media()` builder method

### 2. `src/chat/message_store.rs` (Mới tạo)
**Mục đích:** Quản lý lưu trữ tin nhắn và message IDs
**Các phương thức chính:**
- `new()` - Tạo MessageStore mới
- `add_message()` - Thêm tin nhắn
- `delete_message()` - Xóa/thu hồi tin nhắn
- `edit_message()` - Sửa nội dung tin nhắn
- `get_message()` - Lấy tin nhắn bằng message_id
- `can_delete_message()` - Kiểm tra quyền xóa
- `get_room_messages()` - Lấy tất cả tin nhắn của room
- `clear_room()` - Xóa toàn bộ tin nhắn của room

### 3. `src/chat/mod.rs`
**Thay đổi:** Thêm export cho module mới
```rust
pub mod message_store;
```

### 4. `src/server/websocket.rs`
**Thay đổi chính:**
- Thêm `MessageStore` vào `AppState`
- Thêm 3 handler mới:
  - `handle_media()` - Xử lý gửi media
  - `handle_delete()` - Xử lý xóa tin nhắn
  - `handle_edit()` - Xử lý sửa tin nhắn
- Cập nhật match statement để dispatch đến handlers mới
- Cập nhật handlers hiện có để dùng `ChatMessage::new()`
- Cập nhật để gọi `message_store.add_message()` cho tất cả tin nhắn

### 5. `src/server/mod.rs`
**Thay đổi:** Khởi tạo `MessageStore` khi tạo `AppState`
```rust
message_store: MessageStore::new(),
```

### 6. `src/client/client.rs`
**Thay đổi:** Cập nhật ChatMessage initialization để dùng `ChatMessage::new()`

---

## Message Types Được Hỗ Trợ

| msg_type | Mục Đích | Field Cần Thiết |
|----------|---------|-----------------|
| "join" | Người dùng join room | username, room |
| "leave" | Người dùng rời room | username, room |
| "message" | Tin nhắn text | username, content, room |
| "media" | Gửi media (ảnh/video/file) | username, content, room, media |
| "delete" | Xóa tin nhắn | username, content (message_id), room |
| "edit" | Sửa tin nhắn | username, content (new content), target (message_id), room |
| "system" | Thông báo hệ thống | content, room |
| "error" | Thông báo lỗi | content |

---

## Message ID Format

```
Format: "{username}-{unix_timestamp}"
Ví dụ: "alice-1715000000"
```

Message ID được **tự động tạo bởi server** khi nhận tin nhắn từ client.

---

## Lưu Trữ Tin Nhắn (MessageStore Architecture)

### Cấu Trúc Lưu Trữ

```
MessageStore
├── room_messages: HashMap
│   ├── "general" -> [msg1, msg2, msg3, ...]
│   ├── "photos" -> [msg4, msg5, ...]
│   └── "work" -> [msg6, ...]
│
└── message_index: HashMap
    ├── "alice-1715000000" -> ("general", 0)
    ├── "bob-1715000001" -> ("general", 1)
    └── "charlie-1715000010" -> ("work", 0)
```

### Lợi Ích

✓ O(1) lookup tin nhắn bằng message_id
✓ O(1) xác định room của tin nhắn
✓ Hỗ trợ quick delete/edit operations
✓ Lưu trữ lịch sử đầy đủ (marked as "deleted")

---

## Quy Trình Xóa Tin Nhắn

```
Client sends delete request
    ↓
Server receives delete message
    ↓
Server checks: is requester the message author?
    ↓
    ├─ NO → send error to client
    │
    └─ YES → mark message as "deleted"
           ↓
           update content to "[Tin nhắn đã bị xóa]"
           ↓
           change msg_type to "deleted"
           ↓
           clear media field
           ↓
           broadcast message_deleted notification
           ↓
           send delete_ack to client
```

---

## Quy Trình Sửa Tin Nhắn

```
Client sends edit request
    ↓
Server receives edit message
    ↓
Server checks: is requester the message author?
    ↓
    ├─ NO → send error to client
    │
    └─ YES → update message content
           ↓
           change msg_type to "edited"
           ↓
           broadcast message_edited notification
           ↓
           send edit_ack to client
```

---

## Quy Trình Gửi Media

```
Client prepares media
    ↓
Client uploads file to server (via HTTP)
    ↓
Server returns file URL
    ↓
Client sends media message via WebSocket
    {
      "msg_type": "media",
      "username": "alice",
      "media": {
        "media_type": "image",
        "file_url": "https://example.com/file.jpg",
        ...
      }
    }
    ↓
Server creates message_id & timestamp
    ↓
Server stores in MessageStore
    ↓
Server broadcasts to room members
    ↓
Server sends media_ack to client
```

---

## Error Handling

### Lỗi Phổ Biến & Xử Lý

```rust
// Lỗi: Không phải tác giả
{
  "msg_type": "error",
  "username": "SYSTEM",
  "content": "You can only delete your own messages"
}

// Lỗi: Tin nhắn không tìm thấy
{
  "msg_type": "error",
  "username": "SYSTEM",
  "content": "Message not found or already deleted"
}

// Lỗi: Chưa join room
{
  "msg_type": "error",
  "username": "SYSTEM",
  "content": "Join the room before sending media"
}

// Lỗi: Room trống
{
  "msg_type": "error",
  "username": "SYSTEM",
  "content": "Room name is required"
}
```

---

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| Add message | O(1) | Direct push to vec + index entry |
| Delete message | O(1) | Direct index lookup |
| Edit message | O(1) | Direct index lookup |
| Get message | O(1) | Direct index lookup |
| Get room messages | O(n) | n = số tin nhắn trong room |
| Clear room | O(n) | n = số tin nhắn trong room |

---

## Giới Hạn Hiện Tại & Tương Lai

### Giới Hạn Hiện Tại
- Tin nhắn lưu trữ trong bộ nhớ (không persistent)
- Không có database backend
- Không có full-text search
- Không có message retention policy

### Cải Tiến Tương Lai
1. Thêm persistent storage (Database - PostgreSQL, MongoDB)
2. Thêm message search functionality
3. Thêm auto-delete policy (xóa tin nhắn cũ)
4. Thêm message reactions (👍, ❤️, etc.)
5. Thêm typing indicators
6. Thêm read receipts
7. Thêm message threading
8. Thêm forwarding capability
9. Thêm pinned messages
10. Thêm bulk operations

---

## Ví Dụ JSON Requests

### 1. Send Text Message
```json
{
  "msg_type": "message",
  "username": "alice",
  "content": "Hello everyone!",
  "room": "general"
}
```

### 2. Send Image
```json
{
  "msg_type": "media",
  "username": "bob",
  "content": "Check out this photo!",
  "room": "photos",
  "media": {
    "media_type": "image",
    "file_url": "https://cdn.example.com/photo.jpg",
    "file_name": "photo.jpg",
    "file_size": 512000,
    "mime_type": "image/jpeg"
  }
}
```

### 3. Send Video
```json
{
  "msg_type": "media",
  "username": "charlie",
  "content": "Tutorial video",
  "room": "education",
  "media": {
    "media_type": "video",
    "file_url": "https://cdn.example.com/tutorial.mp4",
    "file_name": "tutorial.mp4",
    "file_size": 52428800,
    "mime_type": "video/mp4"
  }
}
```

### 4. Delete Message
```json
{
  "msg_type": "delete",
  "username": "alice",
  "content": "alice-1715000000",
  "room": "general"
}
```

### 5. Edit Message
```json
{
  "msg_type": "edit",
  "username": "alice",
  "content": "Updated content",
  "target": "alice-1715000000",
  "room": "general"
}
```

---

## Testing

Để chạy ví dụ:
```bash
cargo run --example media_examples
```

Để chạy tests:
```bash
cargo test --example media_examples
```

---

## Documentation Files

1. **MEDIA_FEATURES.md** - Hướng dẫn chi tiết sử dụng các tính năng
2. **examples/media_examples.rs** - Ví dụ code Rust
3. **API_SUMMARY.md** (file này) - Tóm tắt API changes

---

## Contact & Support

Để có câu hỏi hoặc báo cáo vấn đề, vui lòng mở issue hoặc contact team development.
