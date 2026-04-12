use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub username: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum PrivateMessage {
    #[serde(rename = "private_invite")]
    PrivateInvite { from: String, to: String },
    #[serde(rename = "private_invite_accepted")]
    PrivateInviteAccepted {
        from: String,
        to: String,
        room_id: String,
    },
    #[serde(rename = "private_invite_declined")]
    PrivateInviteDeclined { from: String, to: String },
    #[serde(rename = "private_chat")]
    PrivateChat {
        from: String,
        room_id: String,
        content: String,
    },
    #[serde(rename = "leave_private_room")]
    LeavePrivateRoom { from: String, room_id: String },
    #[serde(rename = "system")]
    System {
        to: Option<String>,
        content: String,
    },
}
