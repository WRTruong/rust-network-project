use bcrypt::{DEFAULT_COST, hash, verify};
use serde::{Deserialize, Serialize};
use tiberius::Client;
use tokio::net::TcpStream;
use tokio_util::compat::Compat;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    pub user_id: i32,
    pub username: String,
    pub email: String,
    pub phone: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub role: String,
    pub permissions: Vec<String>,
}

impl UserSession {
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission)
    }
}

pub fn validate_email(email: &str) -> bool {
    email.contains('@') && email.contains('.')
}

pub fn validate_phone(phone: &str) -> bool {
    phone.len() >= 10 && phone.len() <= 20 && phone.chars().all(|c| c.is_numeric() || c == '+')
}

pub async fn register(
    db: &mut Client<Compat<TcpStream>>,
    username: &str,
    email: &str,
    phone: &str,
    password: &str,
) -> bool {
    if !validate_email(email) || !validate_phone(phone) {
        return false;
    }

    let hashed = match hash(password, DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => return false,
    };

    let res = db
        .execute(
            "INSERT INTO Users (username, email, phone, password_hash, display_name, role_id)
             SELECT @P1, @P2, @P3, @P4, @P1, id FROM Roles WHERE name = N'user'",
            &[&username, &email, &phone, &hashed],
        )
        .await;

    res.is_ok()
}

pub async fn login(
    db: &mut Client<Compat<TcpStream>>,
    identifier: &str,
    password: &str,
) -> Option<UserSession> {
    let stream = db
        .query(
            "SELECT u.id, u.username, u.email, u.phone, u.display_name, u.avatar_url, 
                    u.password_hash, u.is_active, r.name
             FROM Users u
             JOIN Roles r ON r.id = u.role_id
             WHERE u.username = @P1 OR u.email = @P1 OR u.phone = @P1",
            &[&identifier],
        )
        .await
        .ok()?;

    let row = stream.into_row().await.ok()??;
    let user_id: i32 = row.get(0)?;
    let username: &str = row.get(1)?;
    let email: &str = row.get(2)?;
    let phone: &str = row.get(3)?;
    let display_name: Option<&str> = row.get(4);
    let avatar_url: Option<&str> = row.get(5);
    let db_hash: &str = row.get(6)?;
    let is_active: bool = row.get(7)?;
    let role: &str = row.get(8)?;

    if !is_active || !verify(password, db_hash).unwrap_or(false) {
        return None;
    }

    let _ = db
        .execute(
            "UPDATE Users SET last_login_at = SYSUTCDATETIME() WHERE id = @P1",
            &[&user_id],
        )
        .await;

    let permissions = get_permissions(db, user_id).await.unwrap_or_default();

    Some(UserSession {
        user_id,
        username: username.to_string(),
        email: email.to_string(),
        phone: phone.to_string(),
        display_name: display_name.map(|s| s.to_string()),
        avatar_url: avatar_url.map(|s| s.to_string()),
        role: role.to_string(),
        permissions,
    })
}

pub async fn update_profile(
    db: &mut Client<Compat<TcpStream>>,
    user_id: i32,
    display_name: Option<&str>,
    bio: Option<&str>,
    avatar_url: Option<&str>,
) -> bool {
    let res = db
        .execute(
            "UPDATE Users 
             SET display_name = COALESCE(@P1, display_name),
                 bio = COALESCE(@P2, bio),
                 avatar_url = COALESCE(@P3, avatar_url),
                 updated_at = SYSUTCDATETIME()
             WHERE id = @P4",
            &[&display_name, &bio, &avatar_url, &user_id],
        )
        .await;

    res.is_ok()
}

pub async fn get_user_profile(
    db: &mut Client<Compat<TcpStream>>,
    user_id: i32,
) -> Option<UserSession> {
    let stream = db
        .query(
            "SELECT u.id, u.username, u.email, u.phone, u.display_name, u.avatar_url, r.name
             FROM Users u
             JOIN Roles r ON r.id = u.role_id
             WHERE u.id = @P1",
            &[&user_id],
        )
        .await
        .ok()?;

    let row = stream.into_row().await.ok()??;
    let id: i32 = row.get(0)?;
    let username: &str = row.get(1)?;
    let email: &str = row.get(2)?;
    let phone: &str = row.get(3)?;
    let display_name: Option<&str> = row.get(4);
    let avatar_url: Option<&str> = row.get(5);
    let role: &str = row.get(6)?;

    let permissions = get_permissions(db, id).await.unwrap_or_default();

    Some(UserSession {
        user_id: id,
        username: username.to_string(),
        email: email.to_string(),
        phone: phone.to_string(),
        display_name: display_name.map(|s| s.to_string()),
        avatar_url: avatar_url.map(|s| s.to_string()),
        role: role.to_string(),
        permissions,
    })
}

pub async fn get_permissions(
    db: &mut Client<Compat<TcpStream>>,
    user_id: i32,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let stream = db
        .query(
            "SELECT p.name
             FROM Users u
             JOIN RolePermissions rp ON rp.role_id = u.role_id
             JOIN Permissions p ON p.id = rp.permission_id
             WHERE u.id = @P1
             ORDER BY p.name",
            &[&user_id],
        )
        .await?;

    let rows = stream.into_first_result().await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| row.get::<&str, _>(0).map(|p| p.to_string()))
        .collect())
}

pub async fn change_password(
    db: &mut Client<Compat<TcpStream>>,
    user_id: i32,
    old_pass: &str,
    new_pass: &str,
) -> bool {
    let stream = match db
        .query(
            "SELECT password_hash FROM Users WHERE id = @P1",
            &[&user_id],
        )
        .await
    {
        Ok(stream) => stream,
        Err(_) => return false,
    };

    let row = match stream.into_row().await {
        Ok(Some(row)) => row,
        _ => return false,
    };

    let db_hash: &str = row.get(0).unwrap_or("");
    if !verify(old_pass, db_hash).unwrap_or(false) {
        return false;
    }

    let new_hash = match hash(new_pass, DEFAULT_COST) {
        Ok(hash) => hash,
        Err(_) => return false,
    };

    db.execute(
        "UPDATE Users SET password_hash = @P1, updated_at = SYSUTCDATETIME() WHERE id = @P2",
        &[&new_hash, &user_id],
    )
    .await
    .is_ok()
}
