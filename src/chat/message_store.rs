use crate::chat::message::ChatMessage;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct MessageStore {
    pub room_messages: Arc<RwLock<HashMap<String, Vec<ChatMessage>>>>,
    pub message_index: Arc<RwLock<HashMap<String, (String, usize)>>>,
}

impl MessageStore {
    pub fn new() -> Self {
        MessageStore {
            room_messages: Arc::new(RwLock::new(HashMap::new())),
            message_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_message(&self, msg: ChatMessage) {
        let room = msg.room.clone();
        let message_id = msg.message_id.clone();

        let mut room_msgs = self.room_messages.write().await;
        let messages = room_msgs.entry(room.clone()).or_default();
        let index = messages.len();
        messages.push(msg);

        let mut msg_index = self.message_index.write().await;
        msg_index.insert(message_id, (room, index));
    }

    pub async fn get_room_messages(&self, room: &str) -> Vec<ChatMessage> {
        let room_msgs = self.room_messages.read().await;
        room_msgs.get(room).cloned().unwrap_or_default()
    }

    pub async fn delete_message(&self, message_id: &str) -> Option<ChatMessage> {
        let msg_index = self.message_index.read().await;

        if let Some((room, index)) = msg_index.get(message_id) {
            let mut room_msgs = self.room_messages.write().await;

            if let Some(messages) = room_msgs.get_mut(room) {
                if *index < messages.len() {
                    if messages[*index].msg_type == "deleted" {
                        return None;
                    }

                    messages[*index].msg_type = "deleted".to_string();
                    messages[*index].content = "[message deleted]".to_string();
                    messages[*index].media = None;
                    return Some(messages[*index].clone());
                }
            }
        }

        None
    }

    pub async fn edit_message(&self, message_id: &str, new_content: &str) -> Option<ChatMessage> {
        let msg_index = self.message_index.read().await;

        if let Some((room, index)) = msg_index.get(message_id) {
            let mut room_msgs = self.room_messages.write().await;

            if let Some(messages) = room_msgs.get_mut(room) {
                if *index < messages.len() {
                    if messages[*index].msg_type == "deleted" {
                        return None;
                    }

                    messages[*index].content = new_content.to_string();
                    messages[*index].msg_type = "edited".to_string();
                    messages[*index].media = None;
                    return Some(messages[*index].clone());
                }
            }
        }

        None
    }

    pub async fn get_message(&self, message_id: &str) -> Option<ChatMessage> {
        let msg_index = self.message_index.read().await;

        if let Some((room, index)) = msg_index.get(message_id) {
            let room_msgs = self.room_messages.read().await;

            if let Some(messages) = room_msgs.get(room) {
                if *index < messages.len() {
                    return Some(messages[*index].clone());
                }
            }
        }

        None
    }

    pub async fn can_modify_message(&self, message_id: &str, username: &str) -> bool {
        if let Some(msg) = self.get_message(message_id).await {
            msg.username == username
                && matches!(msg.msg_type.as_str(), "message" | "media" | "edited")
        } else {
            false
        }
    }
}
