# Hướng Dẫn Sử Dụng Tính Năng Media & Delete Messages

## Tổng Quan

Dự án chat này hiện hỗ trợ ba tính năng chính:
1. **Gửi Media** - Hình ảnh, video, file
2. **Xóa/Thu hồi tin nhắn** - Delete messages
3. **Sửa tin nhắn** - Edit messages

---

## 1. Gửi Hình Ảnh, Video, File (Media)

### Cấu trúc Tin Nhắn Media

```json
{
  "msg_type": "media",
  "username": "user1",
  "content": "Check out this image!",
  "room": "general",
  "target": "",
  "users": [],
  "message_id": "user1-1715000000",
  "timestamp": 1715000000,
  "media": {
    "media_type": "image",
    "file_url": "https://example.com/image.jpg",
    "file_name": "image.jpg",
    "file_size": 204800,
    "mime_type": "image/jpeg"
  }
}
```

### Các loại Media Được Hỗ Trợ

- **image** - Hình ảnh (JPEG, PNG, GIF, WebP, etc.)
- **video** - Video (MP4, WebM, MOV, etc.)
- **file** - Tập tin (PDF, DOC, ZIP, etc.)

### Ví Dụ Gửi Media

#### Gửi Hình Ảnh
```json
{
  "msg_type": "media",
  "username": "alice",
  "content": "My vacation photo",
  "room": "photos",
  "media": {
    "media_type": "image",
    "file_url": "https://cdn.example.com/vacation.jpg",
    "file_name": "vacation.jpg",
    "file_size": 512000,
    "mime_type": "image/jpeg"
  }
}
```

#### Gửi Video
```json
{
  "msg_type": "media",
  "username": "bob",
  "content": "Check out my new video!",
  "room": "entertainment",
  "media": {
    "media_type": "video",
    "file_url": "https://cdn.example.com/video.mp4",
    "file_name": "video.mp4",
    "file_size": 5242880,
    "mime_type": "video/mp4"
  }
}
```

#### Gửi File
```json
{
  "msg_type": "media",
  "username": "charlie",
  "content": "Project documentation",
  "room": "work",
  "media": {
    "media_type": "file",
    "file_url": "https://cdn.example.com/doc.pdf",
    "file_name": "document.pdf",
    "file_size": 1048576,
    "mime_type": "application/pdf"
  }
}
```

### Phản Hồi Từ Server

Khi gửi media thành công, client sẽ nhận được:
```json
{
  "msg_type": "media_ack",
  "username": "SYSTEM",
  "content": "Media sent: image.jpg",
  "room": "photos",
  "timestamp": 1715000001
}
```

---

## 2. Xóa/Thu Hồi Tin Nhắn (Delete)

### Yêu Cầu
- Chỉ có thể xóa tin nhắn **của chính mình**
- Server sẽ kiểm tra xem người yêu cầu có phải là tác giả tin nhắn không
- Tin nhắn sẽ được đánh dấu là "deleted" thay vì xóa hoàn toàn (giữ lịch sử)

### Cấu Trúc Yêu Cầu Delete

```json
{
  "msg_type": "delete",
  "username": "alice",
  "content": "alice-1715000000",
  "room": "general"
}
```

Trong đó:
- `msg_type`: "delete"
- `content`: **message_id** của tin nhắn cần xóa
- `room`: room chứa tin nhắn
- `username`: tên người dùng

### Ví Dụ Xóa Tin Nhắn

```json
{
  "msg_type": "delete",
  "username": "user1",
  "content": "user1-1715000000",
  "room": "general"
}
```

### Phản Hồi Từ Server

Khi xóa thành công:
```json
{
  "msg_type": "delete_ack",
  "username": "SYSTEM",
  "content": "Message deleted successfully",
  "room": "general"
}
```

Khi xóa thất bại (không phải tác giả):
```json
{
  "msg_type": "error",
  "username": "SYSTEM",
  "content": "You can only delete your own messages"
}
```

### Tin Nhắn Đã Xóa (Trong Lịch Sử)

Một khi tin nhắn được xóa, nó sẽ hiển thị như sau:
```json
{
  "msg_type": "deleted",
  "username": "alice",
  "content": "[Tin nhắn đã bị xóa]",
  "room": "general",
  "message_id": "alice-1715000000",
  "timestamp": 1715000000
}
```

---

## 3. Sửa Tin Nhắn (Edit)

### Yêu Cầu
- Chỉ có thể sửa tin nhắn **của chính mình**
- Nội dung mới sẽ thay thế nội dung cũ
- Tin nhắn sẽ được đánh dấu là "edited"

### Cấu Trúc Yêu Cầu Edit

```json
{
  "msg_type": "edit",
  "username": "alice",
  "content": "Updated message content",
  "target": "alice-1715000000",
  "room": "general"
}
```

Trong đó:
- `msg_type`: "edit"
- `content`: **nội dung mới**
- `target`: **message_id** của tin nhắn cần sửa
- `room`: room chứa tin nhắn
- `username`: tên người dùng

### Ví Dụ Sửa Tin Nhắn

```json
{
  "msg_type": "edit",
  "username": "bob",
  "content": "I meant to say something different",
  "target": "bob-1715000000",
  "room": "general"
}
```

### Phản Hồi Từ Server

Khi sửa thành công:
```json
{
  "msg_type": "edit_ack",
  "username": "SYSTEM",
  "content": "Message edited successfully",
  "room": "general"
}
```

### Tin Nhắn Đã Sửa (Trong Lịch Sử)

Tin nhắn đã sửa sẽ có `msg_type` = "edited":
```json
{
  "msg_type": "edited",
  "username": "bob",
  "content": "I meant to say something different",
  "room": "general",
  "message_id": "bob-1715000000",
  "timestamp": 1715000000
}
```

---

## 4. Message ID & Timestamp

### Định Dạng Message ID

```
format: "{username}-{timestamp}"
ví dụ: "alice-1715000000"
```

Message ID được tự động tạo bởi server khi nhận tin nhắn.

### Timestamp

Timestamp là UNIX epoch time (giây từ 1970-01-01 UTC).

Ví dụ:
- `1715000000` = May 6, 2024

---

## 5. Kiến Trúc Lưu Trữ (Message Store)

### MessageStore

`MessageStore` là một cấu trúc dữ liệu trong bộ nhớ quản lý tất cả tin nhắn:

```rust
pub struct MessageStore {
    // Lưu trữ theo room: room_name -> Vec<ChatMessage>
    pub room_messages: Arc<RwLock<HashMap<String, Vec<ChatMessage>>>>,
    
    // Lưu trữ index theo message_id: message_id -> (room_name, index_in_room)
    pub message_index: Arc<RwLock<HashMap<String, (String, usize)>>>,
}
```

### Các Phương Thức

- `add_message(msg)` - Thêm tin nhắn mới
- `get_room_messages(room)` - Lấy tất cả tin nhắn của room
- `delete_message(message_id)` - Xóa tin nhắn (đánh dấu là deleted)
- `edit_message(message_id, new_content)` - Sửa nội dung tin nhắn
- `get_message(message_id)` - Lấy một tin nhắn cụ thể
- `can_delete_message(message_id, username)` - Kiểm tra quyền xóa
- `clear_room(room)` - Xóa toàn bộ tin nhắn của room

---

## 6. Tích Hợp Với Client

### Quy Trình Gửi Media

1. Client tải file lên server (ngoài WebSocket)
2. Server trả về file URL
3. Client gửi media message với file URL qua WebSocket:
   ```json
   {
     "msg_type": "media",
     "username": "user1",
     "content": "description",
     "room": "general",
     "media": {
       "media_type": "image",
       "file_url": "https://example.com/uploads/xyz.jpg",
       "file_name": "xyz.jpg",
       "file_size": 204800,
       "mime_type": "image/jpeg"
     }
   }
   ```
4. Server phát sóng tin nhắn tới tất cả thành viên trong room

### Quy Trình Xóa Tin Nhắn

1. Client gửi delete request:
   ```json
   {
     "msg_type": "delete",
     "username": "alice",
     "content": "alice-1715000000",
     "room": "general"
   }
   ```
2. Server kiểm tra quyền
3. Server đánh dấu tin nhắn là deleted
4. Server gửi xác nhận tới client
5. Server phát sóng thông báo xóa tới tất cả thành viên

---

## 7. Định Dạng ChatMessage Mở Rộng

```rust
pub struct ChatMessage {
    pub msg_type: String,              // "message", "media", "delete", "edit", etc.
    pub username: String,
    pub content: String,
    pub room: String,
    pub target: String,                // Cho private chat
    pub users: Vec<String>,
    pub message_id: String,            // Unique ID
    pub timestamp: u64,                // UNIX epoch time
    pub media: Option<MediaInfo>,      // Thông tin media (nếu có)
}

pub struct MediaInfo {
    pub media_type: String,            // "image", "video", "file"
    pub file_url: String,
    pub file_name: String,
    pub file_size: u64,
    pub mime_type: Option<String>,
}
```

---

## 8. Các Lỗi Phổ Biến & Xử Lý

| Lỗi | Nguyên Nhân | Cách Xử Lý |
|-----|-----------|----------|
| "You can only delete your own messages" | Không phải tác giả | Kiểm tra message_id và username |
| "Message not found or already deleted" | Tin nhắn không tồn tại | Kiểm tra message_id hợp lệ |
| "Join the room before sending media" | Chưa join room | Join room trước khi gửi |
| "Room name is required" | Room trống | Cung cấp room name hợp lệ |

---

## 9. Tương Lai & Cải Tiến

### Những Tính Năng Có Thể Thêm

1. **Attachment Storage** - Lưu file trực tiếp trên server
2. **Message Reactions** - Emoji reactions (👍, ❤️, etc.)
3. **Typing Indicators** - Hiển thị "đang gõ"
4. **Read Receipts** - Xác nhận đã đọc
5. **Forwarding** - Chuyển tiếp tin nhắn
6. **Pinned Messages** - Tin nhắn được ghim
7. **Message Search** - Tìm kiếm tin nhắn
8. **Message Threading** - Trả lời tin nhắn cụ thể
9. **Bulk Delete** - Xóa nhiều tin nhắn cùng lúc
10. **Message Backup** - Sao lưu tin nhắn

---

## 10. Ví Dụ Thực Tế (Flow)

### Ví Dụ: Gửi ảnh và xóa

```
[Client A]
1. Gửi join request -> Join room "photos"
2. Tải ảnh lên -> Nhận URL: https://example.com/img123.jpg
3. Gửi media message:
   {
     "msg_type": "media",
     "username": "alice",
     "content": "Check this out!",
     "room": "photos",
     "media": {
       "media_type": "image",
       "file_url": "https://example.com/img123.jpg",
       "file_name": "photo.jpg",
       "file_size": 512000,
       "mime_type": "image/jpeg"
     }
   }

[Server]
- Tạo message_id: "alice-1715000000"
- Lưu vào MessageStore
- Phát sóng tới tất cả thành viên room "photos"

[Other Clients]
- Nhận media message
- Hiển thị ảnh

[5 phút sau - Client A thay đổi ý định]
4. Gửi delete request:
   {
     "msg_type": "delete",
     "username": "alice",
     "content": "alice-1715000000",
     "room": "photos"
   }

[Server]
- Kiểm tra: alice là tác giả ✓
- Đánh dấu tin nhắn là "deleted"
- Phát sóng thông báo xóa

[Other Clients]
- Nhận message_deleted notification
- Cập nhật UI: hiển thị "[Tin nhắn đã bị xóa]"
```

---

## License & Support

Để có hỗ trợ thêm, vui lòng mở issue trên GitHub hoặc liên hệ với nhóm phát triển.
