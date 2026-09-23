/// 基于设备的身份验证：通过 httpOnly Cookie 中的 device_token 识别设备。
use crate::error::AppError;
use crate::models::DeviceUser;
use axum::http::{header, HeaderMap};
use sqlx::SqlitePool;

/// Axum 的已认证设备用户提取器。
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i64,
    pub display_name: String,
    pub role: String,
    pub device_token: String,
}

/// 检查用户是否为管理员。
pub fn require_admin(auth_user: &AuthUser) -> Result<(), AppError> {
    if auth_user.role == "admin" {
        Ok(())
    } else {
        Err(AppError::Forbidden("Admin privileges required".into()))
    }
}

/// 生成一个随机的设备令牌（使用 UUID v4）。
pub fn generate_device_token() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 从 Cookie 头提取 device_token 值。
pub fn extract_device_token_from_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookie_str| {
            cookie_str.split(';').find_map(|pair| {
                let pair = pair.trim();
                pair.strip_prefix("device_token=").map(|v| v.to_string())
            })
        })
}

/// 从请求中提取 device_token。
/// 优先级：
/// 1. Cookie 中的 device_token
/// 2. X-Device-Token 请求头（跨域或第三方 Cookie 受限时客户端使用）
/// 3. Authorization: Bearer <token> 请求头
pub fn extract_device_token(headers: &HeaderMap) -> Option<String> {
    extract_device_token_from_cookie(headers)
        .or_else(|| {
            headers
                .get("x-device-token")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|auth| auth.strip_prefix("Bearer "))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// 通过 device_token 查找已注册设备用户，不会创建新记录。
pub async fn lookup_device_user(
    db: &SqlitePool,
    device_token: &str,
) -> Result<Option<AuthUser>, AppError> {
    let user = sqlx::query_as::<_, DeviceUser>("SELECT * FROM device_users WHERE device_token = ?")
        .bind(device_token)
        .fetch_optional(db)
        .await?;

    match user {
        Some(u) => {
            if u.is_banned() {
                return Err(AppError::Banned);
            }
            Ok(Some(AuthUser {
                id: u.id,
                display_name: u.display_name.clone(),
                role: u.role.clone(),
                device_token: u.device_token.clone(),
            }))
        }
        None => Ok(None),
    }
}

/// 通过 device_token 查找或创建设备用户。
/// 新设备仅在使用需要身份的操作时以默认名称和 user 角色创建。
pub async fn ensure_device_user(db: &SqlitePool, device_token: &str) -> Result<AuthUser, AppError> {
    match lookup_device_user(db, device_token).await? {
        Some(user) => Ok(user),
        None => {
            // 创建新设备用户
            let result = sqlx::query(
                "INSERT INTO device_users (device_token, display_name, role) VALUES (?, '', 'user')"
            )
            .bind(device_token)
            .execute(db)
            .await?;

            let id = result.last_insert_rowid();
            let display_name = DeviceUser::default_display_name(id);

            sqlx::query("UPDATE device_users SET display_name = ? WHERE id = ?")
                .bind(&display_name)
                .bind(id)
                .execute(db)
                .await?;

            Ok(AuthUser {
                id,
                display_name,
                role: "user".into(),
                device_token: device_token.to_string(),
            })
        }
    }
}

/// 从 HeaderMap 认证设备用户（完整认证，用于受保护路由）。
pub async fn require_device_auth(
    headers: &HeaderMap,
    db: &SqlitePool,
) -> Result<AuthUser, AppError> {
    let device_token = extract_device_token(headers).ok_or(AppError::Unauthorized)?;
    ensure_device_user(db, &device_token).await
}

/// 从 HeaderMap 查找已注册设备用户，不会为公开读取请求创建记录。
pub async fn lookup_device_auth(
    headers: &HeaderMap,
    db: &SqlitePool,
) -> Result<AuthUser, AppError> {
    let device_token = extract_device_token(headers).ok_or(AppError::Unauthorized)?;
    lookup_device_user(db, &device_token)
        .await?
        .ok_or(AppError::Unauthorized)
}

/// 从 HeaderMap 认证管理员设备用户。
pub async fn require_admin_from_headers(
    headers: &HeaderMap,
    db: &SqlitePool,
) -> Result<AuthUser, AppError> {
    let user = lookup_device_auth(headers, db).await?;
    require_admin(&user)?;
    Ok(user)
}

/// 可选认证（已登录返回 Some，访客返回 None）。
pub async fn optional_device_auth(headers: &HeaderMap, db: &SqlitePool) -> Option<AuthUser> {
    let device_token = extract_device_token(headers)?;
    lookup_device_user(db, &device_token).await.ok().flatten()
}

/// 针对本地主机（Loopback 回路请求）确保其拥有管理员角色（OP/房主）。
/// 若不存在则以 display_name = "房主", role = "admin" 插入；
/// 若已存在但 role != "admin"，则自动提升为 admin。
pub async fn ensure_local_admin(db: &SqlitePool, device_token: &str) -> Result<AuthUser, AppError> {
    match lookup_device_user(db, device_token).await? {
        Some(mut user) => {
            if user.role != "admin" {
                sqlx::query("UPDATE device_users SET role = 'admin' WHERE id = ?")
                    .bind(user.id)
                    .execute(db)
                    .await?;
                user.role = "admin".into();
            }
            Ok(user)
        }
        None => {
            let result = sqlx::query(
                "INSERT INTO device_users (device_token, display_name, role) VALUES (?, '房主', 'admin')"
            )
            .bind(device_token)
            .execute(db)
            .await?;

            let id = result.last_insert_rowid();
            Ok(AuthUser {
                id,
                display_name: "房主".into(),
                role: "admin".into(),
                device_token: device_token.to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn extracts_token_from_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_static("foo=bar; device_token=test-token-123; other=baz"));
        assert_eq!(extract_device_token(&headers), Some("test-token-123".to_string()));
    }

    #[test]
    fn extracts_token_from_x_device_token_header() {
        let mut headers = HeaderMap::new();
        headers.insert("x-device-token", HeaderValue::from_static("test-token-456"));
        assert_eq!(extract_device_token(&headers), Some("test-token-456".to_string()));
    }

    #[test]
    fn extracts_token_from_authorization_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer test-token-789"));
        assert_eq!(extract_device_token(&headers), Some("test-token-789".to_string()));
    }

    #[test]
    fn cookie_takes_priority_over_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_static("device_token=cookie-token"));
        headers.insert("x-device-token", HeaderValue::from_static("header-token"));
        assert_eq!(extract_device_token(&headers), Some("cookie-token".to_string()));
    }
}

