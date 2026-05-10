use bcrypt::{DEFAULT_COST, hash, verify};
use tiberius::Client;
use tokio::net::TcpStream;
use tokio_util::compat::Compat;

#[derive(Debug, Clone)]
pub struct UserSession {
    pub user_id: i32,
    pub username: String,
    pub role: String,
    pub permissions: Vec<String>,
}

impl UserSession {
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission)
    }
}

pub async fn register(db: &mut Client<Compat<TcpStream>>, user: &str, pass: &str) -> bool {
    let hashed = match hash(pass, DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => return false,
    };

    let res = db
        .execute(
            "INSERT INTO Users (username, password_hash, display_name, role_id)
             SELECT @P1, @P2, @P1, id FROM Roles WHERE name = N'user'",
            &[&user, &hashed],
        )
        .await;

    res.is_ok()
}

pub async fn login(
    db: &mut Client<Compat<TcpStream>>,
    user: &str,
    pass: &str,
) -> Option<UserSession> {
    let stream = db
        .query(
            "SELECT u.id, u.username, u.password_hash, u.is_active, r.name
             FROM Users u
             JOIN Roles r ON r.id = u.role_id
             WHERE u.username = @P1",
            &[&user],
        )
        .await
        .ok()?;

    let row = stream.into_row().await.ok()??;
    let user_id: i32 = row.get(0)?;
    let username: &str = row.get(1)?;
    let db_hash: &str = row.get(2)?;
    let is_active: bool = row.get(3)?;
    let role: &str = row.get(4)?;

    if !is_active || !verify(pass, db_hash).unwrap_or(false) {
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
