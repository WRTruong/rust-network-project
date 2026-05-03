/// Ví Dụ Client Code - Cách Gửi Media, Delete, Edit Messages
use serde_json::json;

/// Gửi hình ảnh
pub fn send_image_example() -> String {
    let media_message = json!({
        "msg_type": "media",
        "username": "alice",
        "content": "My awesome photo!",
        "room": "general",
        "target": "",
        "users": [],
        "message_id": "",
        "timestamp": 0,
        "media": {
            "media_type": "image",
            "file_url": "https://cdn.example.com/photos/alice-beach.jpg",
            "file_name": "beach.jpg",
            "file_size": 1024000,
            "mime_type": "image/jpeg"
        }
    });

    media_message.to_string()
}

/// Gửi video
pub fn send_video_example() -> String {
    let media_message = json!({
        "msg_type": "media",
        "username": "bob",
        "content": "Check out this tutorial!",
        "room": "education",
        "target": "",
        "users": [],
        "message_id": "",
        "timestamp": 0,
        "media": {
            "media_type": "video",
            "file_url": "https://cdn.example.com/videos/tutorial.mp4",
            "file_name": "tutorial.mp4",
            "file_size": 52428800,
            "mime_type": "video/mp4"
        }
    });

    media_message.to_string()
}

/// Gửi file (PDF, Word, etc.)
pub fn send_document_example() -> String {
    let media_message = json!({
        "msg_type": "media",
        "username": "charlie",
        "content": "Here's the project proposal",
        "room": "work",
        "target": "",
        "users": [],
        "message_id": "",
        "timestamp": 0,
        "media": {
            "media_type": "file",
            "file_url": "https://cdn.example.com/docs/proposal-2024.pdf",
            "file_name": "proposal-2024.pdf",
            "file_size": 2097152,
            "mime_type": "application/pdf"
        }
    });

    media_message.to_string()
}

/// Xóa tin nhắn
///
/// # Arguments
/// * `message_id` - ID của tin nhắn cần xóa (format: "username-timestamp")
pub fn delete_message_example(message_id: &str) -> String {
    let delete_request = json!({
        "msg_type": "delete",
        "username": "alice",
        "content": message_id,  // message_id đặt trong content field
        "room": "general",
        "target": "",
        "users": [],
        "message_id": "",
        "timestamp": 0
    });

    delete_request.to_string()
}

/// Sửa tin nhắn
///
/// # Arguments
/// * `message_id` - ID của tin nhắn cần sửa
/// * `new_content` - Nội dung mới
pub fn edit_message_example(message_id: &str, new_content: &str) -> String {
    let edit_request = json!({
        "msg_type": "edit",
        "username": "alice",
        "content": new_content,      // Nội dung mới
        "target": message_id,        // message_id đặt trong target field
        "room": "general",
        "users": [],
        "message_id": "",
        "timestamp": 0
    });

    edit_request.to_string()
}

/// Gửi tin nhắn text thường (không phải media)
pub fn send_text_message_example() -> String {
    let text_message = json!({
        "msg_type": "message",
        "username": "alice",
        "content": "Hello everyone!",
        "room": "general",
        "target": "",
        "users": [],
        "message_id": "",
        "timestamp": 0
    });

    text_message.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_image() {
        let json_str = send_image_example();
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(json["msg_type"], "media");
        assert_eq!(json["username"], "alice");
        assert_eq!(json["media"]["media_type"], "image");
        assert!(
            json["media"]["file_url"]
                .as_str()
                .unwrap()
                .contains("beach.jpg")
        );
    }

    #[test]
    fn test_delete_message() {
        let json_str = delete_message_example("alice-1715000000");
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(json["msg_type"], "delete");
        assert_eq!(json["username"], "alice");
        assert_eq!(json["content"], "alice-1715000000");
    }

    #[test]
    fn test_edit_message() {
        let json_str = edit_message_example("alice-1715000000", "This is the updated content");
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(json["msg_type"], "edit");
        assert_eq!(json["username"], "alice");
        assert_eq!(json["target"], "alice-1715000000");
        assert_eq!(json["content"], "This is the updated content");
    }
}

/// Ví dụ cách sử dụng trong một async function
pub async fn example_workflow() {
    // 1. Gửi tin nhắn text
    let text_msg = send_text_message_example();
    println!("Sent text message: {}", text_msg);

    // 2. Gửi ảnh
    let image_msg = send_image_example();
    println!("Sent image: {}", image_msg);

    // 3. Gửi video
    let video_msg = send_video_example();
    println!("Sent video: {}", video_msg);

    // 4. Sửa tin nhắn (sau 1 phút)
    let edited_msg = edit_message_example("alice-1715000000", "I changed my mind about that!");
    println!("Edited message: {}", edited_msg);

    // 5. Xóa tin nhắn (sau 5 phút, thay đổi ý định lần nữa)
    let deleted_msg = delete_message_example("alice-1715000000");
    println!("Deleted message: {}", deleted_msg);
}

fn main() {
    println!("{}", send_image_example());
    println!("{}", send_video_example());
    println!("{}", send_document_example());
    println!(
        "{}",
        edit_message_example("alice-1715000000", "Updated message")
    );
    println!("{}", delete_message_example("alice-1715000000"));
}
