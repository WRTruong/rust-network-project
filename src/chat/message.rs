use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub msg_type: String,
    pub username: String,
    pub content: String,
    pub room: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub users: Vec<String>,
}
