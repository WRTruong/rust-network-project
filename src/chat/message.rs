use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize,Default)]
pub struct ChatMessage {
    pub msg_type: String,
    pub username: String,
    pub password: String,
    pub content: String,
    pub room: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub users: Vec<String>,
    #[serde(default)]
    pub message_id: String,
    #[serde(default)]
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<MediaInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub media_type: String,
    pub file_url: String,
    pub file_name: String,
    pub file_size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

impl ChatMessage {
    pub fn new(msg_type: &str, username: &str, content: &str, room: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        ChatMessage {
            msg_type: msg_type.to_string(),
            username: username.to_string(),
            password: String::new(),
            content: content.to_string(),
            room: room.to_string(),
            target: String::new(),
            users: Vec::new(),
            message_id: format!("{}-{}", username, now.as_nanos()),
            timestamp: now.as_secs(),
            media: None,
        }
    }

}

impl MediaInfo {}
