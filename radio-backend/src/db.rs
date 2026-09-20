/// 数据库初始化、连接池和迁移。
use crate::config::DatabaseConfig;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

/// 初始化 SQLite 数据库连接池并运行迁移。
pub(crate) async fn init_database(config: &DatabaseConfig) -> anyhow::Result<SqlitePool> {
    if config.url.starts_with("sqlite:") {
        if let Some(path) = config.url.strip_prefix("sqlite://") {
            if path.contains('/') {
                if let Some(parent) = std::path::Path::new(path).parent() {
                    std::fs::create_dir_all(parent)?;
                }
            }
        }
    }

    let options = SqliteConnectOptions::from_str(&config.url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5))
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .pragma("cache_size", "-64000");

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    tracing::info!("Database initialized successfully (WAL mode enabled)");

    Ok(pool)
}

/// 获取当前 World 的唯一稳定 ID；若不存在则生成 UUIDv4 并持久化到 world_meta 表中。
pub async fn get_or_create_world_id(db: &SqlitePool) -> Result<String, crate::error::AppError> {
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM world_meta WHERE key = 'world_id'",
    )
    .fetch_optional(db)
    .await?;

    if let Some((world_id,)) = existing {
        return Ok(world_id);
    }

    let new_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO world_meta (key, value) VALUES ('world_id', ?)",
    )
    .bind(&new_id)
    .execute(db)
    .await?;

    tracing::info!("Initialized new persistent World ID: {}", new_id);
    Ok(new_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_world_id_persistence_and_idempotency() -> anyhow::Result<()> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        sqlx::migrate!("./migrations").run(&pool).await?;

        let id1 = get_or_create_world_id(&pool).await?;
        assert!(!id1.is_empty());
        assert!(uuid::Uuid::parse_str(&id1).is_ok());

        let id2 = get_or_create_world_id(&pool).await?;
        assert_eq!(id1, id2, "world_id must be idempotent and persistent");

        Ok(())
    }
}

