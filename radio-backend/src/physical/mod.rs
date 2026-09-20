//! Physical Side abstraction for RakurakuMusicWorld.
//!
//! Formalizes the boundary between the Logical World runtime and physical hosting details:
//! - [`PhysicalStorage`]: SQLite database access, media files, persistent world metadata.
//! - [`PhysicalTransport`]: Ring buffer audio stream output and WebSocket message delivery.
//! - [`PhysicalPlayerRegistry`]: Active listener sessions and device connections.
//! - [`PhysicalLifecycle`]: Server runtime lifecycle and health.
//!
//! [`IntegratedPhysicalSide`] represents the existing single-process monomachine deployment,
//! implementing these traits directly against [`AppState`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sqlx::SqlitePool;

use crate::app::state::AppState;
use crate::error::AppError;

/// Physical persistence and storage capabilities.
pub trait PhysicalStorage: Send + Sync {
    /// SQLite connection pool.
    fn db_pool(&self) -> &SqlitePool;

    /// Root directory path for media assets (audio, covers, lyrics).
    fn media_path(&self) -> &Path;

    /// Query a value from persistent world metadata (`world_meta` table).
    fn get_world_meta(
        &self,
        key: &str,
    ) -> impl std::future::Future<Output = Result<Option<String>, AppError>> + Send;

    /// Store a key-value pair in persistent world metadata (`world_meta` table).
    fn set_world_meta(
        &self,
        key: &str,
        value: &str,
    ) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
}

/// Physical network, transport, and broadcasting capabilities.
pub trait PhysicalTransport: Send + Sync {
    /// Reference to the shared audio streaming ring buffer.
    fn stream_buffer(&self) -> Arc<radio_engine::ring_buffer::RingBuffer>;

    /// Broadcast a text payload to active WebSocket listener clients.
    fn broadcast_text(&self, message: &str) -> usize;
}

/// Physical client connection and session registry.
pub trait PhysicalPlayerRegistry: Send + Sync {
    /// Current count of active listener connections.
    fn active_listener_count(&self) -> usize;

    /// Unique deduplicated display names of active listeners.
    fn active_listener_names(&self) -> Vec<String>;
}

/// Physical runtime lifecycle and health.
pub trait PhysicalLifecycle: Send + Sync {
    /// Unique persistent ID identifying this physical world host instance.
    fn world_id(&self) -> &str;

    /// Public server port.
    fn server_port(&self) -> u16;

    /// Base path prefix.
    fn base_path(&self) -> &str;
}

/// Integrated/Hosted Physical Side implementation.
///
/// Wraps [`AppState`] to fulfill the Physical Side contracts for the
/// single-process monomachine deployment ("Open App & Play").
#[derive(Clone)]
pub struct IntegratedPhysicalSide {
    state: Arc<AppState>,
    media_path: PathBuf,
}

impl IntegratedPhysicalSide {
    /// Create a new IntegratedPhysicalSide backed by the shared AppState.
    pub fn new(state: Arc<AppState>) -> Self {
        let media_path = PathBuf::from(&state.config.audio_engine.media_path);
        Self { state, media_path }
    }

    /// Access the underlying AppState.
    pub fn state(&self) -> &Arc<AppState> {
        &self.state
    }
}

impl PhysicalStorage for IntegratedPhysicalSide {
    fn db_pool(&self) -> &SqlitePool {
        &self.state.db
    }

    fn media_path(&self) -> &Path {
        &self.media_path
    }

    async fn get_world_meta(&self, key: &str) -> Result<Option<String>, AppError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM world_meta WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.state.db)
                .await?;
        Ok(row.map(|(v,)| v))
    }

    async fn set_world_meta(&self, key: &str, value: &str) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO world_meta (key, value, updated_at) VALUES (?, ?, datetime('now')) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .execute(&self.state.db)
        .await?;
        Ok(())
    }
}

impl PhysicalTransport for IntegratedPhysicalSide {
    fn stream_buffer(&self) -> Arc<radio_engine::ring_buffer::RingBuffer> {
        self.state.ring_buffer.clone()
    }

    fn broadcast_text(&self, message: &str) -> usize {
        self.state.ws_tx.send(message.to_string()).unwrap_or(0)
    }
}

impl PhysicalPlayerRegistry for IntegratedPhysicalSide {
    fn active_listener_count(&self) -> usize {
        self.state.listeners.len()
    }

    fn active_listener_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .state
            .listeners
            .iter()
            .map(|entry| entry.value().display_name.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

impl PhysicalLifecycle for IntegratedPhysicalSide {
    fn world_id(&self) -> &str {
        &self.state.world_id
    }

    fn server_port(&self) -> u16 {
        self.state.config.server.port
    }

    fn base_path(&self) -> &str {
        &self.state.config.server.base_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_physical_traits<
        T: PhysicalStorage + PhysicalTransport + PhysicalPlayerRegistry + PhysicalLifecycle + Clone + Send + Sync,
    >() {
    }

    #[test]
    fn integrated_implements_physical_traits() {
        assert_physical_traits::<IntegratedPhysicalSide>();
    }
}

