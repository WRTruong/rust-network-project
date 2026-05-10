use crate::chat::message::{ChatMessage, MediaInfo};
use serde::Serialize;
use std::env;
use tiberius::{AuthMethod, Client, Config, SqlBrowser};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

type DbError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Serialize)]
pub struct ProfileInfo {
    pub username: String,
    pub display_name: String,
    pub bio: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct UserSummary {
    pub username: String,
    pub display_name: String,
    pub bio: String,
}

#[derive(Serialize)]
pub struct FriendLists {
    pub friends: Vec<UserSummary>,
    pub incoming: Vec<UserSummary>,
    pub outgoing: Vec<UserSummary>,
}

#[derive(Serialize)]
pub struct GroupSummary {
    pub name: String,
    pub role: String,
    pub owner: String,
}

#[derive(Serialize)]
pub struct PendingGroupItem {
    pub group: String,
    pub username: String,
}

#[derive(Serialize)]
pub struct GroupLists {
    pub groups: Vec<GroupSummary>,
    pub invites: Vec<GroupSummary>,
    pub join_requests: Vec<PendingGroupItem>,
}

pub async fn get_db_client() -> Result<Client<Compat<TcpStream>>, DbError> {
    let raw_host =
        env::var("DB_HOST").unwrap_or_else(|_| r"DESKTOP-1IU3963\SQLEXPRESS".to_string());
    let configured_port = env::var("DB_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok());
    let database = env::var("DB_NAME").unwrap_or_else(|_| "ChatDB".to_string());
    let user = env::var("DB_USER").unwrap_or_else(|_| "sa".to_string());
    let password = env::var("DB_PASSWORD").unwrap_or_else(|_| "123456".to_string());
    let (host, instance_name) = raw_host
        .split_once('\\')
        .map(|(host, instance)| (host.to_string(), Some(instance.to_string())))
        .unwrap_or((raw_host, None));

    let mut config = Config::new();
    config.host(host);
    if let (None, Some(instance_name)) = (configured_port, instance_name.as_deref()) {
        config.port(1434);
        config.instance_name(instance_name);
    } else {
        config.port(configured_port.unwrap_or(1433));
    }
    config.database(database);
    config.authentication(AuthMethod::sql_server(user, password));
    config.trust_cert();

    let tcp = if configured_port.is_none() && instance_name.is_some() {
        TcpStream::connect_named(&config).await?
    } else {
        TcpStream::connect(config.get_addr()).await?
    };
    tcp.set_nodelay(true)?;

    Ok(Client::connect(config, tcp.compat_write()).await?)
}

pub async fn save_chat_message(msg: &ChatMessage, sender_id: i32) -> Result<(), DbError> {
    let mut client = get_db_client().await?;
    let (media_type, file_url, file_name, file_size, mime_type) = media_params(msg);

    client
        .execute(
            "INSERT INTO ChatHistory
             (message_id, sender_id, sender, message, room, msg_type, created_at,
              media_type, file_url, file_name, file_size, mime_type)
             VALUES
             (@P1, @P2, @P3, @P4, @P5, @P6, SYSUTCDATETIME(),
              @P7, @P8, @P9, @P10, @P11)",
            &[
                &msg.message_id,
                &sender_id,
                &msg.username,
                &msg.content,
                &msg.room,
                &msg.msg_type,
                &media_type,
                &file_url,
                &file_name,
                &file_size,
                &mime_type,
            ],
        )
        .await?;

    Ok(())
}

pub async fn get_profile(user_id: i32) -> Result<ProfileInfo, DbError> {
    let mut client = get_db_client().await?;
    let row = client
        .query(
            "SELECT u.username, COALESCE(u.display_name, u.username), COALESCE(u.bio, N''), r.name
             FROM Users u
             JOIN Roles r ON r.id = u.role_id
             WHERE u.id = @P1",
            &[&user_id],
        )
        .await?
        .into_row()
        .await?
        .ok_or("profile not found")?;

    Ok(ProfileInfo {
        username: row.get::<&str, _>(0).unwrap_or("").to_string(),
        display_name: row.get::<&str, _>(1).unwrap_or("").to_string(),
        bio: row.get::<&str, _>(2).unwrap_or("").to_string(),
        role: row.get::<&str, _>(3).unwrap_or("").to_string(),
    })
}

pub async fn update_profile(
    user_id: i32,
    display_name: &str,
    bio: &str,
) -> Result<ProfileInfo, DbError> {
    let mut client = get_db_client().await?;
    client
        .execute(
            "UPDATE Users
             SET display_name = @P1, bio = @P2, updated_at = SYSUTCDATETIME()
             WHERE id = @P3",
            &[&display_name, &bio, &user_id],
        )
        .await?;

    get_profile(user_id).await
}

pub async fn search_user(
    username: &str,
    current_user_id: i32,
) -> Result<Option<UserSummary>, DbError> {
    let mut client = get_db_client().await?;
    let row = client
        .query(
            "SELECT username, COALESCE(display_name, username), COALESCE(bio, N'')
             FROM Users
             WHERE username = @P1 AND id <> @P2 AND is_active = 1",
            &[&username, &current_user_id],
        )
        .await?
        .into_row()
        .await?;

    Ok(row.map(|row| UserSummary {
        username: row.get::<&str, _>(0).unwrap_or("").to_string(),
        display_name: row.get::<&str, _>(1).unwrap_or("").to_string(),
        bio: row.get::<&str, _>(2).unwrap_or("").to_string(),
    }))
}

pub async fn send_friend_request(requester_id: i32, target_username: &str) -> Result<(), DbError> {
    let mut client = get_db_client().await?;
    let target_id = user_id_by_name(&mut client, target_username).await?;
    if requester_id == target_id || are_friends_by_id(&mut client, requester_id, target_id).await? {
        return Err("friend request not allowed".into());
    }

    let has_pending = client
        .query(
            "SELECT TOP 1 id
             FROM FriendRequests
             WHERE status = N'pending'
               AND ((requester_id = @P1 AND addressee_id = @P2)
                    OR (requester_id = @P2 AND addressee_id = @P1))",
            &[&requester_id, &target_id],
        )
        .await?
        .into_first_result()
        .await?
        .len()
        > 0;

    if has_pending {
        return Err("friend request already pending".into());
    }

    client
        .execute(
            "INSERT INTO FriendRequests (requester_id, addressee_id) VALUES (@P1, @P2)",
            &[&requester_id, &target_id],
        )
        .await?;
    Ok(())
}

pub async fn respond_friend_request(
    current_user_id: i32,
    requester_username: &str,
    accept: bool,
) -> Result<(), DbError> {
    let mut client = get_db_client().await?;
    let requester_id = user_id_by_name(&mut client, requester_username).await?;
    let status = if accept { "accepted" } else { "declined" };
    let pending = client
        .query(
            "SELECT 1 FROM FriendRequests
             WHERE requester_id = @P1 AND addressee_id = @P2 AND status = N'pending'",
            &[&requester_id, &current_user_id],
        )
        .await?
        .into_first_result()
        .await?;
    if pending.is_empty() {
        return Err("friend request not found".into());
    }

    client
        .execute(
            "UPDATE FriendRequests
             SET status = @P1, responded_at = SYSUTCDATETIME()
             WHERE requester_id = @P2 AND addressee_id = @P3 AND status = N'pending'",
            &[&status, &requester_id, &current_user_id],
        )
        .await?;

    if accept {
        let low = requester_id.min(current_user_id);
        let high = requester_id.max(current_user_id);
        client
            .execute(
                "IF NOT EXISTS (SELECT 1 FROM Friends WHERE user_low_id = @P1 AND user_high_id = @P2)
                 INSERT INTO Friends (user_low_id, user_high_id) VALUES (@P1, @P2)",
                &[&low, &high],
            )
            .await?;
    }

    Ok(())
}

pub async fn are_friends(username: &str, other_username: &str) -> Result<bool, DbError> {
    let mut client = get_db_client().await?;
    let left = user_id_by_name(&mut client, username).await?;
    let right = user_id_by_name(&mut client, other_username).await?;
    are_friends_by_id(&mut client, left, right).await
}

pub async fn friend_lists(user_id: i32) -> Result<FriendLists, DbError> {
    let mut client = get_db_client().await?;
    let rows = client
        .query(
            "SELECT u.username, COALESCE(u.display_name, u.username), COALESCE(u.bio, N'')
             FROM Friends f
             JOIN Users u ON u.id = CASE WHEN f.user_low_id = @P1 THEN f.user_high_id ELSE f.user_low_id END
             WHERE f.user_low_id = @P1 OR f.user_high_id = @P1
             ORDER BY u.username",
            &[&user_id],
        )
        .await?
        .into_first_result()
        .await?;
    let friends = rows.into_iter().map(user_summary_from_row).collect();

    let incoming_rows = client
        .query(
            "SELECT u.username, COALESCE(u.display_name, u.username), COALESCE(u.bio, N'')
             FROM FriendRequests fr
             JOIN Users u ON u.id = fr.requester_id
             WHERE fr.addressee_id = @P1 AND fr.status = N'pending'
             ORDER BY fr.created_at DESC",
            &[&user_id],
        )
        .await?
        .into_first_result()
        .await?;
    let incoming = incoming_rows
        .into_iter()
        .map(user_summary_from_row)
        .collect();

    let outgoing_rows = client
        .query(
            "SELECT u.username, COALESCE(u.display_name, u.username), COALESCE(u.bio, N'')
             FROM FriendRequests fr
             JOIN Users u ON u.id = fr.addressee_id
             WHERE fr.requester_id = @P1 AND fr.status = N'pending'
             ORDER BY fr.created_at DESC",
            &[&user_id],
        )
        .await?
        .into_first_result()
        .await?;
    let outgoing = outgoing_rows
        .into_iter()
        .map(user_summary_from_row)
        .collect();

    Ok(FriendLists {
        friends,
        incoming,
        outgoing,
    })
}

pub async fn get_room_history(room: &str) -> Result<Vec<ChatMessage>, DbError> {
    let mut client = get_db_client().await?;
    let stream = client
        .query(
            "SELECT message_id, sender, message, room, msg_type, is_deleted,
                    media_type, file_url, file_name, file_size, mime_type
             FROM (
                 SELECT TOP 50 message_id, sender, message, room, msg_type, is_deleted,
                               media_type, file_url, file_name, file_size, mime_type, created_at
                 FROM ChatHistory
                 WHERE room = @P1
                 ORDER BY created_at DESC
             ) AS recent
             ORDER BY created_at ASC",
            &[&room],
        )
        .await?;

    rows_to_messages(stream.into_first_result().await?)
}

pub async fn create_group(owner_id: i32, group_name: &str) -> Result<(), DbError> {
    let mut client = get_db_client().await?;
    client
        .execute(
            "INSERT INTO Groups (name, owner_id) VALUES (@P1, @P2)",
            &[&group_name, &owner_id],
        )
        .await?;
    let group_id = group_id_by_name(&mut client, group_name).await?;
    client
        .execute(
            "INSERT INTO GroupMembers (group_id, user_id, role) VALUES (@P1, @P2, N'owner')",
            &[&group_id, &owner_id],
        )
        .await?;
    Ok(())
}

pub async fn group_invite(
    owner_id: i32,
    group_name: &str,
    invitee_username: &str,
) -> Result<(), DbError> {
    let mut client = get_db_client().await?;
    let group_id = group_id_by_name(&mut client, group_name).await?;
    if !is_group_owner_by_id(&mut client, group_id, owner_id).await? {
        return Err("only group owner can invite".into());
    }
    let invitee_id = user_id_by_name(&mut client, invitee_username).await?;
    if is_group_member_by_id(&mut client, group_id, invitee_id).await? {
        return Err("user already in group".into());
    }

    client
        .execute(
            "IF NOT EXISTS (
                 SELECT 1 FROM GroupInvites
                 WHERE group_id = @P1 AND invitee_id = @P2 AND status = N'pending'
             )
             INSERT INTO GroupInvites (group_id, inviter_id, invitee_id) VALUES (@P1, @P3, @P2)",
            &[&group_id, &invitee_id, &owner_id],
        )
        .await?;
    Ok(())
}

pub async fn group_invite_accept(user_id: i32, group_name: &str) -> Result<(), DbError> {
    let mut client = get_db_client().await?;
    let group_id = group_id_by_name(&mut client, group_name).await?;
    let pending = client
        .query(
            "SELECT 1 FROM GroupInvites
             WHERE group_id = @P1 AND invitee_id = @P2 AND status = N'pending'",
            &[&group_id, &user_id],
        )
        .await?
        .into_first_result()
        .await?;
    if pending.is_empty() {
        return Err("group invite not found".into());
    }
    client
        .execute(
            "UPDATE GroupInvites
             SET status = N'accepted', responded_at = SYSUTCDATETIME()
             WHERE group_id = @P1 AND invitee_id = @P2 AND status = N'pending'",
            &[&group_id, &user_id],
        )
        .await?;
    add_group_member(&mut client, group_id, user_id).await
}

pub async fn group_join_request(user_id: i32, group_name: &str) -> Result<(), DbError> {
    let mut client = get_db_client().await?;
    let group_id = group_id_by_name(&mut client, group_name).await?;
    if is_group_member_by_id(&mut client, group_id, user_id).await? {
        return Err("already in group".into());
    }
    client
        .execute(
            "IF NOT EXISTS (
                 SELECT 1 FROM GroupJoinRequests
                 WHERE group_id = @P1 AND requester_id = @P2 AND status = N'pending'
             )
             INSERT INTO GroupJoinRequests (group_id, requester_id) VALUES (@P1, @P2)",
            &[&group_id, &user_id],
        )
        .await?;
    Ok(())
}

pub async fn group_join_respond(
    owner_id: i32,
    group_name: &str,
    requester_username: &str,
    accept: bool,
) -> Result<(), DbError> {
    let mut client = get_db_client().await?;
    let group_id = group_id_by_name(&mut client, group_name).await?;
    if !is_group_owner_by_id(&mut client, group_id, owner_id).await? {
        return Err("only group owner can respond".into());
    }
    let requester_id = user_id_by_name(&mut client, requester_username).await?;
    let pending = client
        .query(
            "SELECT 1 FROM GroupJoinRequests
             WHERE group_id = @P1 AND requester_id = @P2 AND status = N'pending'",
            &[&group_id, &requester_id],
        )
        .await?
        .into_first_result()
        .await?;
    if pending.is_empty() {
        return Err("join request not found".into());
    }
    let status = if accept { "accepted" } else { "declined" };
    client
        .execute(
            "UPDATE GroupJoinRequests
             SET status = @P1, responded_at = SYSUTCDATETIME()
             WHERE group_id = @P2 AND requester_id = @P3 AND status = N'pending'",
            &[&status, &group_id, &requester_id],
        )
        .await?;
    if accept {
        add_group_member(&mut client, group_id, requester_id).await?;
    }
    Ok(())
}

pub async fn is_group_member(group_name: &str, user_id: i32) -> Result<bool, DbError> {
    let mut client = get_db_client().await?;
    let group_id = group_id_by_name(&mut client, group_name).await?;
    is_group_member_by_id(&mut client, group_id, user_id).await
}

pub async fn group_lists(user_id: i32) -> Result<GroupLists, DbError> {
    let mut client = get_db_client().await?;
    let group_rows = client
        .query(
            "SELECT g.name, gm.role, owner.username
             FROM GroupMembers gm
             JOIN Groups g ON g.id = gm.group_id
             JOIN Users owner ON owner.id = g.owner_id
             WHERE gm.user_id = @P1
             ORDER BY g.name",
            &[&user_id],
        )
        .await?
        .into_first_result()
        .await?;
    let groups = group_rows.into_iter().map(group_summary_from_row).collect();

    let invite_rows = client
        .query(
            "SELECT g.name, N'invited', owner.username
             FROM GroupInvites gi
             JOIN Groups g ON g.id = gi.group_id
             JOIN Users owner ON owner.id = g.owner_id
             WHERE gi.invitee_id = @P1 AND gi.status = N'pending'
             ORDER BY gi.created_at DESC",
            &[&user_id],
        )
        .await?
        .into_first_result()
        .await?;
    let invites = invite_rows
        .into_iter()
        .map(group_summary_from_row)
        .collect();

    let request_rows = client
        .query(
            "SELECT g.name, requester.username
             FROM Groups g
             JOIN GroupJoinRequests gjr ON gjr.group_id = g.id
             JOIN Users requester ON requester.id = gjr.requester_id
             WHERE g.owner_id = @P1 AND gjr.status = N'pending'
             ORDER BY gjr.created_at DESC",
            &[&user_id],
        )
        .await?
        .into_first_result()
        .await?;
    let join_requests = request_rows
        .into_iter()
        .map(|row| PendingGroupItem {
            group: row.get::<&str, _>(0).unwrap_or("").to_string(),
            username: row.get::<&str, _>(1).unwrap_or("").to_string(),
        })
        .collect();

    Ok(GroupLists {
        groups,
        invites,
        join_requests,
    })
}

pub async fn get_private_history(username: &str) -> Result<Vec<ChatMessage>, DbError> {
    let mut client = get_db_client().await?;
    let stream = client
        .query(
            "SELECT message_id, sender, message, room, msg_type, is_deleted,
                    media_type, file_url, file_name, file_size, mime_type
             FROM ChatHistory
             WHERE (room LIKE 'dm:' + @P1 + ':%' OR room LIKE 'dm:%:' + @P2)
             ORDER BY created_at ASC",
            &[&username, &username],
        )
        .await?;

    rows_to_messages(stream.into_first_result().await?)
}

pub async fn delete_message(
    message_id: &str,
    username: &str,
) -> Result<Option<ChatMessage>, DbError> {
    let existing = get_message_by_id(message_id).await?;
    let Some(existing) = existing else {
        return Ok(None);
    };

    if existing.username != username || existing.msg_type == "deleted" {
        return Ok(None);
    }

    let mut client = get_db_client().await?;
    client
        .execute(
            "UPDATE ChatHistory
             SET is_deleted = 1,
                 deleted_at = SYSUTCDATETIME(),
                 updated_at = SYSUTCDATETIME(),
                 msg_type = N'deleted',
                 message = N'[message deleted]',
                 media_type = NULL,
                 file_url = NULL,
                 file_name = NULL,
                 file_size = NULL,
                 mime_type = NULL
             WHERE message_id = @P1 AND sender = @P2 AND is_deleted = 0",
            &[&message_id, &username],
        )
        .await?;

    get_message_by_id(message_id).await
}

pub async fn edit_message(
    message_id: &str,
    username: &str,
    new_content: &str,
) -> Result<Option<ChatMessage>, DbError> {
    let existing = get_message_by_id(message_id).await?;
    let Some(existing) = existing else {
        return Ok(None);
    };

    if existing.username != username || existing.msg_type == "deleted" {
        return Ok(None);
    }

    let mut client = get_db_client().await?;
    client
        .execute(
            "UPDATE ChatHistory
             SET message = @P1,
                 msg_type = N'edited',
                 edited_at = SYSUTCDATETIME(),
                 updated_at = SYSUTCDATETIME(),
                 media_type = NULL,
                 file_url = NULL,
                 file_name = NULL,
                 file_size = NULL,
                 mime_type = NULL
             WHERE message_id = @P2 AND sender = @P3 AND is_deleted = 0",
            &[&new_content, &message_id, &username],
        )
        .await?;

    get_message_by_id(message_id).await
}

async fn get_message_by_id(message_id: &str) -> Result<Option<ChatMessage>, DbError> {
    let mut client = get_db_client().await?;
    let stream = client
        .query(
            "SELECT message_id, sender, message, room, msg_type, is_deleted,
                    media_type, file_url, file_name, file_size, mime_type
             FROM ChatHistory
             WHERE message_id = @P1",
            &[&message_id],
        )
        .await?;

    let messages = rows_to_messages(stream.into_first_result().await?)?;
    Ok(messages.into_iter().next())
}

async fn user_id_by_name(
    client: &mut Client<Compat<TcpStream>>,
    username: &str,
) -> Result<i32, DbError> {
    let row = client
        .query("SELECT id FROM Users WHERE username = @P1", &[&username])
        .await?
        .into_row()
        .await?
        .ok_or("user not found")?;
    Ok(row.get::<i32, _>(0).ok_or("user id missing")?)
}

async fn group_id_by_name(
    client: &mut Client<Compat<TcpStream>>,
    group_name: &str,
) -> Result<i32, DbError> {
    let row = client
        .query("SELECT id FROM Groups WHERE name = @P1", &[&group_name])
        .await?
        .into_row()
        .await?
        .ok_or("group not found")?;
    Ok(row.get::<i32, _>(0).ok_or("group id missing")?)
}

async fn are_friends_by_id(
    client: &mut Client<Compat<TcpStream>>,
    left_id: i32,
    right_id: i32,
) -> Result<bool, DbError> {
    let low = left_id.min(right_id);
    let high = left_id.max(right_id);
    let rows = client
        .query(
            "SELECT 1 FROM Friends WHERE user_low_id = @P1 AND user_high_id = @P2",
            &[&low, &high],
        )
        .await?
        .into_first_result()
        .await?;
    Ok(!rows.is_empty())
}

async fn is_group_owner_by_id(
    client: &mut Client<Compat<TcpStream>>,
    group_id: i32,
    user_id: i32,
) -> Result<bool, DbError> {
    let rows = client
        .query(
            "SELECT 1 FROM Groups WHERE id = @P1 AND owner_id = @P2",
            &[&group_id, &user_id],
        )
        .await?
        .into_first_result()
        .await?;
    Ok(!rows.is_empty())
}

async fn is_group_member_by_id(
    client: &mut Client<Compat<TcpStream>>,
    group_id: i32,
    user_id: i32,
) -> Result<bool, DbError> {
    let rows = client
        .query(
            "SELECT 1 FROM GroupMembers WHERE group_id = @P1 AND user_id = @P2",
            &[&group_id, &user_id],
        )
        .await?
        .into_first_result()
        .await?;
    Ok(!rows.is_empty())
}

async fn add_group_member(
    client: &mut Client<Compat<TcpStream>>,
    group_id: i32,
    user_id: i32,
) -> Result<(), DbError> {
    client
        .execute(
            "IF NOT EXISTS (SELECT 1 FROM GroupMembers WHERE group_id = @P1 AND user_id = @P2)
             INSERT INTO GroupMembers (group_id, user_id, role) VALUES (@P1, @P2, N'member')",
            &[&group_id, &user_id],
        )
        .await?;
    Ok(())
}

fn user_summary_from_row(row: tiberius::Row) -> UserSummary {
    UserSummary {
        username: row.get::<&str, _>(0).unwrap_or("").to_string(),
        display_name: row.get::<&str, _>(1).unwrap_or("").to_string(),
        bio: row.get::<&str, _>(2).unwrap_or("").to_string(),
    }
}

fn group_summary_from_row(row: tiberius::Row) -> GroupSummary {
    GroupSummary {
        name: row.get::<&str, _>(0).unwrap_or("").to_string(),
        role: row.get::<&str, _>(1).unwrap_or("").to_string(),
        owner: row.get::<&str, _>(2).unwrap_or("").to_string(),
    }
}

fn media_params(
    msg: &ChatMessage,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
) {
    if let Some(media) = &msg.media {
        (
            Some(media.media_type.clone()),
            Some(media.file_url.clone()),
            Some(media.file_name.clone()),
            Some(media.file_size as i64),
            media.mime_type.clone(),
        )
    } else {
        (None, None, None, None, None)
    }
}

fn rows_to_messages(rows: Vec<tiberius::Row>) -> Result<Vec<ChatMessage>, DbError> {
    let mut messages = Vec::new();

    for row in rows {
        let message_id: &str = row.get(0).unwrap_or("");
        let sender: &str = row.get(1).unwrap_or("");
        let message: &str = row.get(2).unwrap_or("");
        let room: &str = row.get(3).unwrap_or("");
        let msg_type: &str = row.get(4).unwrap_or("message");
        let is_deleted: bool = row.get(5).unwrap_or(false);
        let media_type: Option<&str> = row.get(6);

        let mut chat_msg = ChatMessage {
            msg_type: if is_deleted {
                "deleted".to_string()
            } else {
                msg_type.to_string()
            },
            username: sender.to_string(),
            content: message.to_string(),
            room: room.to_string(),
            message_id: message_id.to_string(),
            ..Default::default()
        };

        if !is_deleted {
            if let Some(media_type) = media_type {
                let file_url: &str = row.get(7).unwrap_or("");
                let file_name: &str = row.get(8).unwrap_or("");
                let file_size: i64 = row.get(9).unwrap_or(0);
                let mime_type: Option<&str> = row.get(10);

                chat_msg.media = Some(MediaInfo {
                    media_type: media_type.to_string(),
                    file_url: file_url.to_string(),
                    file_name: file_name.to_string(),
                    file_size: file_size.max(0) as u64,
                    mime_type: mime_type.map(|value| value.to_string()),
                });
            }
        }

        messages.push(chat_msg);
    }

    Ok(messages)
}
