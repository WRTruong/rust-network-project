use axum::{
    extract::State,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
use crate::chat::message::{ChatMessage, PrivateMessage};
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};
use tokio::sync::{
    broadcast::Sender,
    mpsc::{self, UnboundedSender},
};

#[derive(Clone)]
pub struct AppState {
    pub tx: Sender<ChatMessage>,
    pub history: Arc<Mutex<Vec<ChatMessage>>>,
    pub private_state: Arc<Mutex<PrivateState>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateRoom {
    room_id: String,
    user_a: String,
    user_b: String,
}

impl PrivateRoom {
    fn partner_of(&self, username: &str) -> Option<String> {
        if self.user_a == username {
            Some(self.user_b.clone())
        } else if self.user_b == username {
            Some(self.user_a.clone())
        } else {
            None
        }
    }

    fn members(&self) -> [String; 2] {
        [self.user_a.clone(), self.user_b.clone()]
    }
}

#[derive(Default)]
pub(crate) struct PrivateState {
    online_users: HashMap<String, UnboundedSender<PrivateMessage>>,
    pending_invites: HashSet<(String, String)>,
    private_rooms: HashMap<String, PrivateRoom>,
    active_room_by_user: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Eq)]
struct DisconnectCleanup {
    canceled_invites: Vec<(String, String)>,
    closed_room: Option<(PrivateRoom, String)>,
}

impl PrivateState {
    fn register_user(
        &mut self,
        username: String,
        sender: UnboundedSender<PrivateMessage>,
    ) -> Result<(), String> {
        if self.online_users.contains_key(&username) {
            return Err(format!("Username '{}' is already online.", username));
        }

        self.online_users.insert(username, sender);
        Ok(())
    }

    fn sender_for(&self, username: &str) -> Option<UnboundedSender<PrivateMessage>> {
        self.online_users.get(username).cloned()
    }

    fn handle_invite(&mut self, from: &str, to: &str) -> Result<(), String> {
        if from == to {
            return Err("You cannot invite yourself to a private chat.".to_string());
        }

        if !self.online_users.contains_key(to) {
            return Err(format!("User '{}' is not online.", to));
        }

        if self.active_room_by_user.contains_key(from) {
            return Err("You are already in a private chat.".to_string());
        }

        if self.active_room_by_user.contains_key(to) {
            return Err(format!("User '{}' is already in a private chat.", to));
        }

        let pair = (from.to_string(), to.to_string());
        let reverse_pair = (to.to_string(), from.to_string());
        if self.pending_invites.contains(&pair) || self.pending_invites.contains(&reverse_pair) {
            return Err(format!(
                "There is already a pending private invite between '{}' and '{}'.",
                from, to
            ));
        }

        self.pending_invites.insert(pair);
        Ok(())
    }

    fn handle_accept(&mut self, acceptor: &str, inviter: &str) -> Result<PrivateRoom, String> {
        let pair = (inviter.to_string(), acceptor.to_string());
        if !self.pending_invites.remove(&pair) {
            return Err(format!("No pending invite from '{}' was found.", inviter));
        }

        if self.active_room_by_user.contains_key(inviter)
            || self.active_room_by_user.contains_key(acceptor)
        {
            return Err("One of the users is already in a private chat.".to_string());
        }

        let room = PrivateRoom {
            room_id: make_room_id(inviter, acceptor),
            user_a: inviter.to_string(),
            user_b: acceptor.to_string(),
        };

        self.active_room_by_user
            .insert(inviter.to_string(), room.room_id.clone());
        self.active_room_by_user
            .insert(acceptor.to_string(), room.room_id.clone());
        self.private_rooms
            .insert(room.room_id.clone(), room.clone());

        Ok(room)
    }

    fn handle_decline(&mut self, decliner: &str, inviter: &str) -> Result<(), String> {
        let pair = (inviter.to_string(), decliner.to_string());
        if self.pending_invites.remove(&pair) {
            Ok(())
        } else {
            Err(format!("No pending invite from '{}' was found.", inviter))
        }
    }

    fn validate_private_chat(&self, username: &str, room_id: &str) -> Result<PrivateRoom, String> {
        let active_room = self
            .active_room_by_user
            .get(username)
            .ok_or_else(|| "You are not in a private chat.".to_string())?;

        if active_room != room_id {
            return Err("This private room is not active for you.".to_string());
        }

        let room = self
            .private_rooms
            .get(room_id)
            .cloned()
            .ok_or_else(|| "Private room not found.".to_string())?;

        if room.partner_of(username).is_none() {
            return Err("You are not a member of this private room.".to_string());
        }

        Ok(room)
    }

    fn leave_room(&mut self, username: &str, room_id: &str) -> Result<PrivateRoom, String> {
        let room = self.validate_private_chat(username, room_id)?;
        self.private_rooms.remove(&room.room_id);
        self.active_room_by_user.remove(&room.user_a);
        self.active_room_by_user.remove(&room.user_b);
        Ok(room)
    }

    fn disconnect(&mut self, username: &str) -> DisconnectCleanup {
        self.online_users.remove(username);

        let mut canceled_invites = Vec::new();
        let pending_pairs: Vec<(String, String)> = self
            .pending_invites
            .iter()
            .filter(|(from, to)| from == username || to == username)
            .cloned()
            .collect();

        for pair in pending_pairs {
            self.pending_invites.remove(&pair);
            canceled_invites.push(pair);
        }

        let closed_room = self
            .active_room_by_user
            .get(username)
            .cloned()
            .and_then(|room_id| self.leave_room(username, &room_id).ok().map(|room| (room, username.to_string())));

        DisconnectCleanup {
            canceled_invites,
            closed_room,
        }
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    println!("Client connected");
    let (mut sender, mut receiver) = socket.split();
    let mut group_rx = state.tx.subscribe();
    let (private_tx, mut private_rx) = mpsc::unbounded_channel::<PrivateMessage>();

    let mut username: Option<String> = None;

    let history = { state.history.lock().unwrap().clone() };
    for msg in history {
        let json = serde_json::to_string(&msg).unwrap();
        let _ = sender.send(Message::Text(json.into())).await;
    }

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(mut msg) = serde_json::from_str::<ChatMessage>(&text) {
                            if msg.content == "__join__" {
                                if username.is_some() {
                                    continue;
                                }

                                let register_result = {
                                    state.private_state.lock().unwrap().register_user(
                                        msg.username.clone(),
                                        private_tx.clone(),
                                    )
                                };

                                match register_result {
                                    Ok(()) => {
                                        username = Some(msg.username.clone());

                                        let join_msg = ChatMessage {
                                            username: "SYSTEM".to_string(),
                                            content: format!("{} joined the group chat", msg.username),
                                        };

                                        let _ = state.tx.send(join_msg.clone());
                                        state.history.lock().unwrap().push(join_msg);
                                    }
                                    Err(error) => {
                                        let system_message = PrivateMessage::System {
                                            to: Some(msg.username.clone()),
                                            content: error,
                                        };
                                        let json = serde_json::to_string(&system_message).unwrap();
                                        let _ = sender.send(Message::Text(json.into())).await;
                                        break;
                                    }
                                }
                            } else if let Some(ref current_user) = username {
                                msg.username = current_user.clone();
                                state.history.lock().unwrap().push(msg.clone());
                                let _ = state.tx.send(msg);
                            }
                        } else if let Ok(private_message) = serde_json::from_str::<PrivateMessage>(&text) {
                            if let Some(ref current_user) = username {
                                handle_private_message(&state, current_user, private_message);
                            } else {
                                let system_message = PrivateMessage::System {
                                    to: None,
                                    content: "Join the server before using private chat.".to_string(),
                                };
                                let json = serde_json::to_string(&system_message).unwrap();
                                let _ = sender.send(Message::Text(json.into())).await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
            outgoing = group_rx.recv() => {
                if let Ok(msg) = outgoing {
                    let json = serde_json::to_string(&msg).unwrap();
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
            private_outgoing = private_rx.recv() => {
                if let Some(msg) = private_outgoing {
                    let json = serde_json::to_string(&msg).unwrap();
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    if let Some(user) = username {
        let cleanup = state.private_state.lock().unwrap().disconnect(&user);

        for (from, to) in cleanup.canceled_invites {
            if from == user {
                send_private_message(
                    &state,
                    &to,
                    PrivateMessage::System {
                        to: Some(to.clone()),
                        content: format!("Private invite from '{}' was canceled because they disconnected.", from),
                    },
                );
            } else {
                send_private_message(
                    &state,
                    &from,
                    PrivateMessage::System {
                        to: Some(from.clone()),
                        content: format!("Private invite to '{}' was canceled because they disconnected.", to),
                    },
                );
            }
        }

        if let Some((room, leaver)) = cleanup.closed_room {
            for member in room.members() {
                send_private_message(
                    &state,
                    &member,
                    PrivateMessage::LeavePrivateRoom {
                        from: leaver.clone(),
                        room_id: room.room_id.clone(),
                    },
                );
            }
        }

        let leave_msg = ChatMessage {
            username: "SYSTEM".to_string(),
            content: format!("{} left the group chat", user),
        };

        let _ = state.tx.send(leave_msg.clone());
        state.history.lock().unwrap().push(leave_msg);
    }

    println!("Client disconnected");
}

fn handle_private_message(state: &AppState, username: &str, message: PrivateMessage) {
    match message {
        PrivateMessage::PrivateInvite { from, to } => {
            if from != username {
                send_private_message(
                    state,
                    username,
                    PrivateMessage::System {
                        to: Some(username.to_string()),
                        content: "Invite sender does not match the authenticated user.".to_string(),
                    },
                );
                return;
            }

            let result = state.private_state.lock().unwrap().handle_invite(&from, &to);
            match result {
                Ok(()) => {
                    send_private_message(
                        state,
                        &to,
                        PrivateMessage::PrivateInvite {
                            from: from.clone(),
                            to: to.clone(),
                        },
                    );
                    send_private_message(
                        state,
                        &from,
                        PrivateMessage::System {
                            to: Some(from.clone()),
                            content: format!("Private invite sent to '{}'.", to),
                        },
                    );
                }
                Err(error) => {
                    send_private_message(
                        state,
                        &from,
                        PrivateMessage::System {
                            to: Some(from.clone()),
                            content: error,
                        },
                    );
                }
            }
        }
        PrivateMessage::PrivateInviteAccepted { from, to, .. } => {
            if from != username {
                send_private_message(
                    state,
                    username,
                    PrivateMessage::System {
                        to: Some(username.to_string()),
                        content: "Accept sender does not match the authenticated user.".to_string(),
                    },
                );
                return;
            }

            let result = state.private_state.lock().unwrap().handle_accept(&from, &to);
            match result {
                Ok(room) => {
                    let event = PrivateMessage::PrivateInviteAccepted {
                        from: to.clone(),
                        to: from.clone(),
                        room_id: room.room_id.clone(),
                    };
                    send_private_message(state, &room.user_a, event.clone());
                    send_private_message(state, &room.user_b, event);
                }
                Err(error) => {
                    send_private_message(
                        state,
                        &from,
                        PrivateMessage::System {
                            to: Some(from.clone()),
                            content: error,
                        },
                    );
                }
            }
        }
        PrivateMessage::PrivateInviteDeclined { from, to } => {
            if from != username {
                send_private_message(
                    state,
                    username,
                    PrivateMessage::System {
                        to: Some(username.to_string()),
                        content: "Decline sender does not match the authenticated user.".to_string(),
                    },
                );
                return;
            }

            let result = state.private_state.lock().unwrap().handle_decline(&from, &to);
            match result {
                Ok(()) => {
                    send_private_message(
                        state,
                        &to,
                        PrivateMessage::PrivateInviteDeclined {
                            from: from.clone(),
                            to: to.clone(),
                        },
                    );
                    send_private_message(
                        state,
                        &from,
                        PrivateMessage::System {
                            to: Some(from.clone()),
                            content: format!("You declined '{}'s private invite.", to),
                        },
                    );
                }
                Err(error) => {
                    send_private_message(
                        state,
                        &from,
                        PrivateMessage::System {
                            to: Some(from.clone()),
                            content: error,
                        },
                    );
                }
            }
        }
        PrivateMessage::PrivateChat {
            from,
            room_id,
            content,
        } => {
            if from != username {
                send_private_message(
                    state,
                    username,
                    PrivateMessage::System {
                        to: Some(username.to_string()),
                        content: "Private chat sender does not match the authenticated user.".to_string(),
                    },
                );
                return;
            }

            let result = state
                .private_state
                .lock()
                .unwrap()
                .validate_private_chat(&from, &room_id);

            match result {
                Ok(room) => {
                    let event = PrivateMessage::PrivateChat {
                        from: from.clone(),
                        room_id: room.room_id.clone(),
                        content,
                    };
                    for member in room.members() {
                        send_private_message(state, &member, event.clone());
                    }
                }
                Err(error) => {
                    send_private_message(
                        state,
                        &from,
                        PrivateMessage::System {
                            to: Some(from.clone()),
                            content: error,
                        },
                    );
                }
            }
        }
        PrivateMessage::LeavePrivateRoom { from, room_id } => {
            if from != username {
                send_private_message(
                    state,
                    username,
                    PrivateMessage::System {
                        to: Some(username.to_string()),
                        content: "Leave sender does not match the authenticated user.".to_string(),
                    },
                );
                return;
            }

            let result = state
                .private_state
                .lock()
                .unwrap()
                .leave_room(&from, &room_id);

            match result {
                Ok(room) => {
                    for member in room.members() {
                        send_private_message(
                            state,
                            &member,
                            PrivateMessage::LeavePrivateRoom {
                                from: from.clone(),
                                room_id: room.room_id.clone(),
                            },
                        );
                    }
                }
                Err(error) => {
                    send_private_message(
                        state,
                        &from,
                        PrivateMessage::System {
                            to: Some(from.clone()),
                            content: error,
                        },
                    );
                }
            }
        }
        PrivateMessage::System { .. } => {}
    }
}

fn send_private_message(state: &AppState, username: &str, message: PrivateMessage) {
    let sender = { state.private_state.lock().unwrap().sender_for(username) };
    if let Some(sender) = sender {
        let _ = sender.send(message);
    }
}

fn make_room_id(user_a: &str, user_b: &str) -> String {
    let mut pair = [user_a.to_string(), user_b.to_string()];
    pair.sort();
    format!("pm:{}:{}", pair[0], pair[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sender() -> UnboundedSender<PrivateMessage> {
        let (tx, _) = mpsc::unbounded_channel();
        tx
    }

    #[test]
    fn invite_requires_online_user() {
        let mut state = PrivateState::default();
        state
            .register_user("alice".to_string(), make_sender())
            .unwrap();

        let result = state.handle_invite("alice", "bob");
        assert_eq!(result, Err("User 'bob' is not online.".to_string()));
    }

    #[test]
    fn duplicate_invite_is_rejected() {
        let mut state = PrivateState::default();
        state
            .register_user("alice".to_string(), make_sender())
            .unwrap();
        state
            .register_user("bob".to_string(), make_sender())
            .unwrap();

        assert!(state.handle_invite("alice", "bob").is_ok());
        let result = state.handle_invite("alice", "bob");

        assert_eq!(
            result,
            Err("There is already a pending private invite between 'alice' and 'bob'.".to_string())
        );
    }

    #[test]
    fn accept_creates_room_for_both_users() {
        let mut state = PrivateState::default();
        state
            .register_user("alice".to_string(), make_sender())
            .unwrap();
        state
            .register_user("bob".to_string(), make_sender())
            .unwrap();
        state.handle_invite("alice", "bob").unwrap();

        let room = state.handle_accept("bob", "alice").unwrap();

        assert_eq!(room.room_id, "pm:alice:bob");
        assert_eq!(state.active_room_by_user.get("alice"), Some(&room.room_id));
        assert_eq!(state.active_room_by_user.get("bob"), Some(&room.room_id));
    }

    #[test]
    fn decline_removes_pending_invite() {
        let mut state = PrivateState::default();
        state
            .register_user("alice".to_string(), make_sender())
            .unwrap();
        state
            .register_user("bob".to_string(), make_sender())
            .unwrap();
        state.handle_invite("alice", "bob").unwrap();

        state.handle_decline("bob", "alice").unwrap();

        assert!(!state
            .pending_invites
            .contains(&("alice".to_string(), "bob".to_string())));
    }

    #[test]
    fn leave_clears_active_room_for_both_users() {
        let mut state = PrivateState::default();
        state
            .register_user("alice".to_string(), make_sender())
            .unwrap();
        state
            .register_user("bob".to_string(), make_sender())
            .unwrap();
        state.handle_invite("alice", "bob").unwrap();
        let room = state.handle_accept("bob", "alice").unwrap();

        let closed_room = state.leave_room("alice", &room.room_id).unwrap();

        assert_eq!(closed_room.room_id, room.room_id);
        assert!(!state.active_room_by_user.contains_key("alice"));
        assert!(!state.active_room_by_user.contains_key("bob"));
        assert!(!state.private_rooms.contains_key(&room.room_id));
    }

    #[test]
    fn disconnect_cleans_up_invites_and_rooms() {
        let mut state = PrivateState::default();
        state
            .register_user("alice".to_string(), make_sender())
            .unwrap();
        state
            .register_user("bob".to_string(), make_sender())
            .unwrap();
        state
            .register_user("charlie".to_string(), make_sender())
            .unwrap();
        state.handle_invite("alice", "charlie").unwrap();
        state.handle_invite("alice", "bob").unwrap();
        let room = state.handle_accept("bob", "alice").unwrap();

        let cleanup = state.disconnect("alice");

        assert!(cleanup
            .canceled_invites
            .contains(&("alice".to_string(), "charlie".to_string())));
        assert_eq!(cleanup.closed_room, Some((room, "alice".to_string())));
        assert!(!state.online_users.contains_key("alice"));
    }
}
