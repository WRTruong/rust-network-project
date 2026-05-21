use crate::chat::message::{ChatMessage, MediaInfo};
use serde::Serialize;
use std::env;
use tiberius::{AuthMethod, Client, Config, EncryptionLevel};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

type DbError = Box<dyn std::error::Error + Send + Sync>;

fn build_config(database: &str) -> Config {
    let host = env::var("DB_HOST").unwrap_or_else(|_| "LAPTOP-E3ETC2TU".to_string());
    let port = env::var("DB_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(1433);
    let user = env::var("DB_USER").unwrap_or_else(|_| "sa".to_string());
    let password = env::var("DB_PASSWORD").unwrap_or_else(|_| "123456".to_string());

    let mut config = Config::new();
    config.host(&host);
    config.port(port);
    config.database(database);
    config.authentication(AuthMethod::sql_server(user, password));
    config.encryption(EncryptionLevel::NotSupported);
    config.trust_cert();
    config
}

pub async fn init_db() -> Result<(), DbError> {
    let host = env::var("DB_HOST").unwrap_or_else(|_| "LAPTOP-E3ETC2TU".to_string());
    println!("Connecting to SQL Server at {}...", host);

    // 1. Kết nối master → tạo database nếu chưa có
    let master_config = build_config("master");
    let tcp = TcpStream::connect(master_config.get_addr())
        .await
        .map_err(|e| format!("Cannot connect to SQL Server at {}: {}", host, e))?;
    tcp.set_nodelay(true)?;
    let mut master_client = Client::connect(master_config, tcp.compat_write()).await?;

    println!("Creating database ChatDB if not exists...");
    master_client
        .execute("IF DB_ID(N'ChatDB') IS NULL CREATE DATABASE ChatDB;", &[])
        .await?;
    drop(master_client);

    // 2. Kết nối trực tiếp ChatDB → tạo schema
    println!("Setting up schema in ChatDB...");
    let mut chatdb_client = get_db_client().await?;
    init_schema(&mut chatdb_client).await?;

    println!("Database initialized successfully!");
    Ok(())
}

async fn init_schema(client: &mut Client<Compat<TcpStream>>) -> Result<(), DbError> {

    // Create Roles table
    client
        .execute(
            "IF OBJECT_ID(N'dbo.Roles', N'U') IS NULL
            BEGIN
                CREATE TABLE dbo.Roles (
                    id INT IDENTITY(1,1) NOT NULL CONSTRAINT PK_Roles PRIMARY KEY,
                    name NVARCHAR(50) NOT NULL CONSTRAINT UQ_Roles_Name UNIQUE,
                    description NVARCHAR(255) NULL,
                    created_at DATETIME2(0) NOT NULL CONSTRAINT DF_Roles_CreatedAt DEFAULT SYSUTCDATETIME()
                );
            END",
            &[],
        )
        .await?;

    // Create Permissions table
    client
        .execute(
            "IF OBJECT_ID(N'dbo.Permissions', N'U') IS NULL
            BEGIN
                CREATE TABLE dbo.Permissions (
                    id INT IDENTITY(1,1) NOT NULL CONSTRAINT PK_Permissions PRIMARY KEY,
                    name NVARCHAR(100) NOT NULL CONSTRAINT UQ_Permissions_Name UNIQUE,
                    description NVARCHAR(255) NULL,
                    created_at DATETIME2(0) NOT NULL CONSTRAINT DF_Permissions_CreatedAt DEFAULT SYSUTCDATETIME()
                );
            END",
            &[],
        )
        .await?;

    // Create RolePermissions table
    client
        .execute(
            "IF OBJECT_ID(N'dbo.RolePermissions', N'U') IS NULL
            BEGIN
                CREATE TABLE dbo.RolePermissions (
                    role_id INT NOT NULL,
                    permission_id INT NOT NULL,
                    CONSTRAINT PK_RolePermissions PRIMARY KEY (role_id, permission_id),
                    CONSTRAINT FK_RolePermissions_Roles FOREIGN KEY (role_id) REFERENCES dbo.Roles(id) ON DELETE CASCADE,
                    CONSTRAINT FK_RolePermissions_Permissions FOREIGN KEY (permission_id) REFERENCES dbo.Permissions(id) ON DELETE CASCADE
                );
            END",
            &[],
        )
        .await?;

    // Create Users table
    client
        .execute(
            "IF OBJECT_ID(N'dbo.Users', N'U') IS NULL
            BEGIN
                CREATE TABLE dbo.Users (
                    id INT IDENTITY(1,1) NOT NULL CONSTRAINT PK_Users PRIMARY KEY,
                    username NVARCHAR(50) NOT NULL,
                    email NVARCHAR(100) NOT NULL,
                    phone NVARCHAR(20) NOT NULL,
                    password_hash NVARCHAR(255) NOT NULL,
                    avatar_url NVARCHAR(MAX) NULL,
                    role_id INT NOT NULL,
                    is_active BIT NOT NULL CONSTRAINT DF_Users_IsActive DEFAULT 1,
                    display_name NVARCHAR(100) NULL,
                    bio NVARCHAR(500) NULL,
                    created_at DATETIME2(0) NOT NULL CONSTRAINT DF_Users_CreatedAt DEFAULT SYSUTCDATETIME(),
                    updated_at DATETIME2(0) NULL,
                    last_login_at DATETIME2(0) NULL,
                    CONSTRAINT UQ_Users_Username UNIQUE (username),
                    CONSTRAINT UQ_Users_Email UNIQUE (email),
                    CONSTRAINT UQ_Users_Phone UNIQUE (phone),
                    CONSTRAINT FK_Users_Roles FOREIGN KEY (role_id) REFERENCES dbo.Roles(id)
                );
            END",
            &[],
        )
        .await?;

    // Create FriendRequests table
    client
        .execute(
            "IF OBJECT_ID(N'dbo.FriendRequests', N'U') IS NULL
            BEGIN
                CREATE TABLE dbo.FriendRequests (
                    id INT IDENTITY(1,1) NOT NULL CONSTRAINT PK_FriendRequests PRIMARY KEY,
                    requester_id INT NOT NULL,
                    addressee_id INT NOT NULL,
                    status NVARCHAR(20) NOT NULL CONSTRAINT DF_FriendRequests_Status DEFAULT N'pending',
                    created_at DATETIME2(0) NOT NULL CONSTRAINT DF_FriendRequests_CreatedAt DEFAULT SYSUTCDATETIME(),
                    responded_at DATETIME2(0) NULL,
                    CONSTRAINT FK_FriendRequests_Requester FOREIGN KEY (requester_id) REFERENCES dbo.Users(id),
                    CONSTRAINT FK_FriendRequests_Addressee FOREIGN KEY (addressee_id) REFERENCES dbo.Users(id),
                    CONSTRAINT CK_FriendRequests_NotSelf CHECK (requester_id <> addressee_id)
                );
            END",
            &[],
        )
        .await?;

    // Create Friends table
    client
        .execute(
            "IF OBJECT_ID(N'dbo.Friends', N'U') IS NULL
            BEGIN
                CREATE TABLE dbo.Friends (
                    user_low_id INT NOT NULL,
                    user_high_id INT NOT NULL,
                    created_at DATETIME2(0) NOT NULL CONSTRAINT DF_Friends_CreatedAt DEFAULT SYSUTCDATETIME(),
                    CONSTRAINT PK_Friends PRIMARY KEY (user_low_id, user_high_id),
                    CONSTRAINT FK_Friends_Low FOREIGN KEY (user_low_id) REFERENCES dbo.Users(id),
                    CONSTRAINT FK_Friends_High FOREIGN KEY (user_high_id) REFERENCES dbo.Users(id),
                    CONSTRAINT CK_Friends_Order CHECK (user_low_id < user_high_id)
                );
            END",
            &[],
        )
        .await?;

    // Create Groups table
    client
        .execute(
            "IF OBJECT_ID(N'dbo.Groups', N'U') IS NULL
            BEGIN
                CREATE TABLE dbo.Groups (
                    id INT IDENTITY(1,1) NOT NULL CONSTRAINT PK_Groups PRIMARY KEY,
                    name NVARCHAR(80) NOT NULL CONSTRAINT UQ_Groups_Name UNIQUE,
                    owner_id INT NOT NULL,
                    created_at DATETIME2(0) NOT NULL CONSTRAINT DF_Groups_CreatedAt DEFAULT SYSUTCDATETIME(),
                    CONSTRAINT FK_Groups_Owner FOREIGN KEY (owner_id) REFERENCES dbo.Users(id)
                );
            END",
            &[],
        )
        .await?;

    // Create GroupMembers table
    client
        .execute(
            "IF OBJECT_ID(N'dbo.GroupMembers', N'U') IS NULL
            BEGIN
                CREATE TABLE dbo.GroupMembers (
                    group_id INT NOT NULL,
                    user_id INT NOT NULL,
                    role NVARCHAR(20) NOT NULL CONSTRAINT DF_GroupMembers_Role DEFAULT N'member',
                    joined_at DATETIME2(0) NOT NULL CONSTRAINT DF_GroupMembers_JoinedAt DEFAULT SYSUTCDATETIME(),
                    CONSTRAINT PK_GroupMembers PRIMARY KEY (group_id, user_id),
                    CONSTRAINT FK_GroupMembers_Groups FOREIGN KEY (group_id) REFERENCES dbo.Groups(id) ON DELETE CASCADE,
                    CONSTRAINT FK_GroupMembers_Users FOREIGN KEY (user_id) REFERENCES dbo.Users(id)
                );
            END",
            &[],
        )
        .await?;

    // Create GroupInvites table
    client
        .execute(
            "IF OBJECT_ID(N'dbo.GroupInvites', N'U') IS NULL
            BEGIN
                CREATE TABLE dbo.GroupInvites (
                    id INT IDENTITY(1,1) NOT NULL CONSTRAINT PK_GroupInvites PRIMARY KEY,
                    group_id INT NOT NULL,
                    inviter_id INT NOT NULL,
                    invitee_id INT NOT NULL,
                    status NVARCHAR(20) NOT NULL CONSTRAINT DF_GroupInvites_Status DEFAULT N'pending',
                    created_at DATETIME2(0) NOT NULL CONSTRAINT DF_GroupInvites_CreatedAt DEFAULT SYSUTCDATETIME(),
                    responded_at DATETIME2(0) NULL,
                    CONSTRAINT FK_GroupInvites_Groups FOREIGN KEY (group_id) REFERENCES dbo.Groups(id) ON DELETE CASCADE,
                    CONSTRAINT FK_GroupInvites_Inviter FOREIGN KEY (inviter_id) REFERENCES dbo.Users(id),
                    CONSTRAINT FK_GroupInvites_Invitee FOREIGN KEY (invitee_id) REFERENCES dbo.Users(id)
                );
            END",
            &[],
        )
        .await?;

    // Create GroupJoinRequests table
    client
        .execute(
            "IF OBJECT_ID(N'dbo.GroupJoinRequests', N'U') IS NULL
            BEGIN
                CREATE TABLE dbo.GroupJoinRequests (
                    id INT IDENTITY(1,1) NOT NULL CONSTRAINT PK_GroupJoinRequests PRIMARY KEY,
                    group_id INT NOT NULL,
                    requester_id INT NOT NULL,
                    status NVARCHAR(20) NOT NULL CONSTRAINT DF_GroupJoinRequests_Status DEFAULT N'pending',
                    created_at DATETIME2(0) NOT NULL CONSTRAINT DF_GroupJoinRequests_CreatedAt DEFAULT SYSUTCDATETIME(),
                    responded_at DATETIME2(0) NULL,
                    CONSTRAINT FK_GroupJoinRequests_Groups FOREIGN KEY (group_id) REFERENCES dbo.Groups(id) ON DELETE CASCADE,
                    CONSTRAINT FK_GroupJoinRequests_Requester FOREIGN KEY (requester_id) REFERENCES dbo.Users(id)
                );
            END",
            &[],
        )
        .await?;

    // Create ChatHistory table
    client
        .execute(
            "IF OBJECT_ID(N'dbo.ChatHistory', N'U') IS NULL
            BEGIN
                CREATE TABLE dbo.ChatHistory (
                    id INT IDENTITY(1,1) NOT NULL CONSTRAINT PK_ChatHistory PRIMARY KEY,
                    message_id NVARCHAR(128) NOT NULL,
                    sender_id INT NULL,
                    sender NVARCHAR(50) NULL,
                    message NVARCHAR(MAX) NULL,
                    room NVARCHAR(100) NOT NULL,
                    msg_type NVARCHAR(20) NOT NULL CONSTRAINT DF_ChatHistory_MsgType DEFAULT N'message',
                    created_at DATETIME2(0) NOT NULL CONSTRAINT DF_ChatHistory_CreatedAt DEFAULT SYSUTCDATETIME(),
                    updated_at DATETIME2(0) NULL,
                    is_deleted BIT NOT NULL CONSTRAINT DF_ChatHistory_IsDeleted DEFAULT 0,
                    deleted_at DATETIME2(0) NULL,
                    edited_at DATETIME2(0) NULL,
                    media_type NVARCHAR(20) NULL,
                    file_url NVARCHAR(MAX) NULL,
                    file_name NVARCHAR(500) NULL,
                    file_size BIGINT NULL,
                    mime_type NVARCHAR(100) NULL,
                    CONSTRAINT UQ_ChatHistory_MessageId UNIQUE (message_id),
                    CONSTRAINT FK_ChatHistory_Users FOREIGN KEY (sender_id) REFERENCES dbo.Users(id)
                );
            END",
            &[],
        )
        .await?;

    // Seed Roles
    client
        .execute(
            "MERGE dbo.Roles AS target
            USING (VALUES
                (N'admin', N'Administrator role'),
                (N'user', N'Default chat user role')
            ) AS source(name, description)
            ON target.name = source.name
            WHEN NOT MATCHED THEN
                INSERT (name, description) VALUES (source.name, source.description);",
            &[],
        )
        .await?;

    // Seed Permissions
    client
        .execute(
            "MERGE dbo.Permissions AS target
            USING (VALUES
                (N'chat.send', N'Send text messages'),
                (N'chat.media', N'Send media messages'),
                (N'message.edit.own', N'Edit own messages'),
                (N'message.delete.own', N'Delete own messages'),
                (N'admin.manage_users', N'Manage users and roles'),
                (N'profile.update', N'Update own profile'),
                (N'friend.manage', N'Manage own friends'),
                (N'group.create', N'Create groups'),
                (N'group.manage.own', N'Manage owned groups'),
                (N'settings.update_password', N'Change own password')
            ) AS source(name, description)
            ON target.name = source.name
            WHEN NOT MATCHED THEN
                INSERT (name, description) VALUES (source.name, source.description);",
            &[],
        )
        .await?;

    // Assign admin permissions
    client
        .execute(
            "INSERT INTO dbo.RolePermissions (role_id, permission_id)
            SELECT r.id, p.id
            FROM dbo.Roles r
            CROSS JOIN dbo.Permissions p
            WHERE r.name = N'admin'
              AND NOT EXISTS (
                  SELECT 1 FROM dbo.RolePermissions rp
                  WHERE rp.role_id = r.id AND rp.permission_id = p.id
              );",
            &[],
        )
        .await?;

    // Assign user permissions
    client
        .execute(
            "INSERT INTO dbo.RolePermissions (role_id, permission_id)
            SELECT r.id, p.id
            FROM dbo.Roles r
            JOIN dbo.Permissions p ON p.name IN (
                N'chat.send',
                N'chat.media',
                N'message.edit.own',
                N'message.delete.own',
                N'profile.update',
                N'friend.manage',
                N'group.create',
                N'group.manage.own',
                N'settings.update_password'
            )
            WHERE r.name = N'user'
              AND NOT EXISTS (
                  SELECT 1 FROM dbo.RolePermissions rp
                  WHERE rp.role_id = r.id AND rp.permission_id = p.id
              );",
            &[],
        )
        .await?;

    Ok(())
}

#[derive(Serialize)]
pub struct ProfileInfo {
    pub username: String,
    pub email: String,
    pub phone: String,
    pub display_name: String,
    pub bio: String,
    pub avatar_url: Option<String>,
    pub role: String,
}

#[derive(Serialize)]
pub struct UserSummary {
    pub username: String,
    pub display_name: String,
    pub bio: String,
    pub avatar_url: Option<String>,
}

#[derive(Serialize)]
pub struct AdminUserSummary {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub is_active: bool,
    pub created_at: String,
    pub last_login_at: Option<String>,
}

#[derive(Serialize)]
pub struct AdminUserList {
    pub users: Vec<AdminUserSummary>,
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
    let config = build_config("ChatDB");
    let tcp = TcpStream::connect(config.get_addr()).await?;
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
            "SELECT u.username, u.email, u.phone, COALESCE(u.display_name, u.username), COALESCE(u.bio, N''), u.avatar_url, r.name
             FROM Users u
             JOIN Roles r ON r.id = u.role_id
             WHERE u.id = @P1",
            &[&user_id],
        )
        .await?
        .into_row()
        .await?
        .ok_or("profile not found")?;

    let avatar: Option<&str> = row.get(5);
    
    Ok(ProfileInfo {
        username: row.get::<&str, _>(0).unwrap_or("").to_string(),
        email: row.get::<&str, _>(1).unwrap_or("").to_string(),
        phone: row.get::<&str, _>(2).unwrap_or("").to_string(),
        display_name: row.get::<&str, _>(3).unwrap_or("").to_string(),
        bio: row.get::<&str, _>(4).unwrap_or("").to_string(),
        avatar_url: avatar.map(|s| s.to_string()),
        role: row.get::<&str, _>(6).unwrap_or("").to_string(),
    })
}

pub async fn update_profile(
    user_id: i32,
    display_name: &str,
    bio: &str,
    avatar_url: Option<&str>,
) -> Result<ProfileInfo, DbError> {
    let mut client = get_db_client().await?;
    client
        .execute(
            "UPDATE Users
             SET display_name = @P1, bio = @P2, avatar_url = COALESCE(@P3, avatar_url), updated_at = SYSUTCDATETIME()
             WHERE id = @P4",
            &[&display_name, &bio, &avatar_url, &user_id],
        )
        .await?;

    get_profile(user_id).await
}

pub async fn admin_list_users(query: &str) -> Result<AdminUserList, DbError> {
    let mut client = get_db_client().await?;
    let search = query.trim();
    let pattern = format!("%{}%", search);
    let rows = client
        .query(
            "SELECT TOP 100 u.id,
                    u.username,
                    COALESCE(u.display_name, u.username),
                    r.name,
                    u.is_active,
                    CONVERT(VARCHAR(19), u.created_at, 126),
                    CONVERT(VARCHAR(19), u.last_login_at, 126)
             FROM Users u
             JOIN Roles r ON r.id = u.role_id
             WHERE @P1 = N''
                OR u.username LIKE @P2
                OR COALESCE(u.display_name, u.username) LIKE @P2
             ORDER BY u.created_at DESC, u.username ASC",
            &[&search, &pattern],
        )
        .await?
        .into_first_result()
        .await?;

    Ok(AdminUserList {
        users: rows.into_iter().map(admin_user_summary_from_row).collect(),
    })
}

pub async fn admin_update_user(
    user_id: i32,
    role: &str,
    is_active: bool,
) -> Result<AdminUserList, DbError> {
    let role = role.trim();
    if role != "admin" && role != "user" {
        return Err("invalid role".into());
    }

    let mut client = get_db_client().await?;
    let result = client
        .execute(
            "UPDATE Users
             SET role_id = (SELECT id FROM Roles WHERE name = @P1),
                 is_active = @P2,
                 updated_at = SYSUTCDATETIME()
             WHERE id = @P3
               AND EXISTS (SELECT 1 FROM Roles WHERE name = @P1)",
            &[&role, &is_active, &user_id],
        )
        .await?;

    if result.rows_affected().iter().sum::<u64>() == 0 {
        return Err("user not found".into());
    }

    admin_list_users("").await
}

pub async fn search_user(
    username: &str,
    current_user_id: i32,
) -> Result<Option<UserSummary>, DbError> {
    let mut client = get_db_client().await?;
    let row: Option<tiberius::Row> = client
        .query(
            "SELECT username, COALESCE(display_name, username), COALESCE(bio, N''), avatar_url
             FROM Users
             WHERE username = @P1 AND id <> @P2 AND is_active = 1",
            &[&username, &current_user_id],
        )
        .await?
        .into_row()
        .await?;

    Ok(row.map(|row| {
        let avatar: Option<&str> = row.get(3);
        UserSummary {
            username: row.get::<&str, _>(0).unwrap_or("").to_string(),
            display_name: row.get::<&str, _>(1).unwrap_or("").to_string(),
            bio: row.get::<&str, _>(2).unwrap_or("").to_string(),
            avatar_url: avatar.map(|s| s.to_string()),
        }
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
            "SELECT u.username, COALESCE(u.display_name, u.username), COALESCE(u.bio, N''), u.avatar_url
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
            "SELECT u.username, COALESCE(u.display_name, u.username), COALESCE(u.bio, N''), u.avatar_url
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
            "SELECT u.username, COALESCE(u.display_name, u.username), COALESCE(u.bio, N''), u.avatar_url
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
                    media_type, file_url, file_name, file_size, mime_type, created_timestamp, avatar_url
             FROM (
                 SELECT TOP 50 ch.message_id, ch.sender, ch.message, ch.room, ch.msg_type, ch.is_deleted,
                               ch.media_type, ch.file_url, ch.file_name, ch.file_size, ch.mime_type,
                               DATEDIFF_BIG(SECOND, '1970-01-01', ch.created_at) AS created_timestamp,
                               u.avatar_url,
                               ch.created_at
                 FROM ChatHistory ch
                 LEFT JOIN Users u ON ch.sender_id = u.id
                 WHERE ch.room = @P1
                 ORDER BY ch.created_at DESC
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
                    media_type, file_url, file_name, file_size, mime_type, created_timestamp, avatar_url
             FROM (
                 SELECT TOP 50 ch.message_id, ch.sender, ch.message, ch.room, ch.msg_type, ch.is_deleted,
                               ch.media_type, ch.file_url, ch.file_name, ch.file_size, ch.mime_type,
                               DATEDIFF_BIG(SECOND, '1970-01-01', ch.created_at) AS created_timestamp,
                               u.avatar_url,
                               ch.created_at
                 FROM ChatHistory ch
                  LEFT JOIN Users u ON ch.sender_id = u.id
                 WHERE (ch.room LIKE 'dm:' + @P1 + ':%' OR ch.room LIKE 'dm:%:' + @P2)
                 ORDER BY ch.created_at DESC
             ) AS recent
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
    let mut client = get_db_client().await?;

    let existing = get_message_by_id_with(&mut client, message_id).await?;
    let Some(existing) = existing else {
        return Ok(None);
    };

    if existing.username != username || existing.msg_type == "deleted" {
        return Ok(None);
    }

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

    get_message_by_id_with(&mut client, message_id).await
}

pub async fn edit_message(
    message_id: &str,
    username: &str,
    new_content: &str,
) -> Result<Option<ChatMessage>, DbError> {
    let mut client = get_db_client().await?;

    let existing = get_message_by_id_with(&mut client, message_id).await?;
    let Some(existing) = existing else {
        return Ok(None);
    };

    if existing.username != username || existing.msg_type == "deleted" {
        return Ok(None);
    }

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

    get_message_by_id_with(&mut client, message_id).await
}

// Tái sử dụng connection đã mở — dùng nội bộ để tránh mở connection thêm
async fn get_message_by_id_with(
    client: &mut Client<Compat<TcpStream>>,
    message_id: &str,
) -> Result<Option<ChatMessage>, DbError> {
    let stream = client
        .query(
            "SELECT ch.message_id, ch.sender, ch.message, ch.room, ch.msg_type, ch.is_deleted,
                    ch.media_type, ch.file_url, ch.file_name, ch.file_size, ch.mime_type,
                    DATEDIFF_BIG(SECOND, '1970-01-01', ch.created_at), u.avatar_url
             FROM ChatHistory ch
             LEFT JOIN Users u ON ch.sender_id = u.id
             WHERE ch.message_id = @P1",
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
    let avatar: Option<&str> = row.get(3);
    UserSummary {
        username: row.get::<&str, _>(0).unwrap_or("").to_string(),
        display_name: row.get::<&str, _>(1).unwrap_or("").to_string(),
        bio: row.get::<&str, _>(2).unwrap_or("").to_string(),
        avatar_url: avatar.map(|s| s.to_string()),
    }
}

fn admin_user_summary_from_row(row: tiberius::Row) -> AdminUserSummary {
    AdminUserSummary {
        id: row.get::<i32, _>(0).unwrap_or_default(),
        username: row.get::<&str, _>(1).unwrap_or("").to_string(),
        display_name: row.get::<&str, _>(2).unwrap_or("").to_string(),
        role: row.get::<&str, _>(3).unwrap_or("").to_string(),
        is_active: row.get::<bool, _>(4).unwrap_or(false),
        created_at: row.get::<&str, _>(5).unwrap_or("").to_string(),
        last_login_at: row.get::<&str, _>(6).map(|value| value.to_string()),
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
        let timestamp = row
            .get::<i64, _>(11)
            .map(|value| value.max(0) as u64)
            .unwrap_or_default();
        let sender_avatar: Option<&str> = row.get(12);

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
            timestamp,
            sender_avatar: sender_avatar.map(|s| s.to_string()),
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
