//! Playlist domain module.
//!
//! Subdivided into:
//! - [`rules`]: Logical playlist rules (validation, capacity, ordering, orchestration).
//! - [`persistence`]: Physical persistence adapter (SQLite queries and atomic mutations).
//!
//! HTTP/background callers enter through `crate::world::WorldRuntime`.
//! This module re-exports the primary queue operations for seamless compatibility.

pub mod persistence;
pub mod rules;

// Re-export public functions to preserve the existing crate::services::queue::* contract.
#[allow(unused_imports)]
pub use rules::{
    add_to_queue, check_cooldown, check_rate_limit, get_history, get_next_song, get_queue_display,
    mark_playing, move_queue_item, purge_song, queue_size, rehydrate_engine_queue,
    remove_queue_item, skip_current,
};
