# Architecture Guide

This document explains the current code layout after the refactor. Its goal is to help contributors find the right place for a change quickly.

## Runtime Shape

- `radio-backend` is the single runtime binary.
- `radio-engine` is a local Rust library embedded by `radio-backend`.
- `radio-backend/frontend` is the Vue 3 application; production assets are built into `radio-backend/static/`.
- The backend serves REST APIs, WebSocket `/ws`, audio stream `/stream`, and static frontend files on the same port.

Do not split `radio-engine` into a separate process or introduce Redis/HTTP IPC for playback control.

## Backend Entry Points

```text
radio-backend/src/main.rs
radio-backend/src/app/bootstrap.rs
radio-backend/src/app/state.rs
```

- `main.rs` only declares modules and calls `app::bootstrap::run()`.
- `app/bootstrap.rs` wires together config, audio engine, database state, WebSocket polling, router layers, CORS, and TCP keepalive binding.
- `app/state.rs` owns `AppState`, the shared state passed to Axum handlers.
- `db.rs` initializes SQLite and runs migrations.

## Backend HTTP Layout

```text
radio-backend/src/http/middleware.rs
radio-backend/src/http/stream.rs
radio-backend/src/routes/mod.rs
radio-backend/src/routes/station.rs
radio-backend/src/routes/*
```

- `http/middleware.rs` handles the device cookie middleware.
- `http/stream.rs` owns `/stream` and reads from the engine ring buffer.
- `routes/mod.rs` assembles route trees and base path mounting.
- `routes/station.rs` owns `/api/station` and `/manifest.json`.
- Feature routes stay in `routes/`, but business logic should move toward `services/` when it grows beyond request/response handling.

## Backend Services

```text
radio-backend/src/services/metadata.rs
radio-backend/src/services/download_tasks.rs
radio-backend/src/services/playback_broadcast.rs
radio-backend/src/services/playback_snapshot.rs
radio-backend/src/services/queue.rs
radio-backend/src/services/ncm/*
radio-backend/src/services/netdisk.rs
```

- `metadata.rs` handles filename parsing, duration lookup, sidecar cover lookup, and lazy embedded cover extraction.
- `download_tasks.rs` owns in-memory batch download task state and snapshots.
- `playback_broadcast.rs` owns the engine state polling loop.
- `playback_snapshot.rs` converts engine playback state into WebSocket messages with DB song info and lyrics.
- `queue.rs` contains the queue business logic.
- `ncm/*` and `netdisk.rs` handle external music sources.

## Cover Artwork Flow

Album cover lookup is lazy and cache-backed.

1. Upload/rescan records a sidecar cover path if one exists near the audio file.
2. `/api/songs/:id/cover` checks `songs.cover_path`.
3. If no valid sidecar cover exists, the backend tries to extract embedded artwork with `ffmpeg`.
4. Extracted covers are cached under `media/.covers/{song_id}.jpg`.
5. Failed extraction writes `media/.covers/{song_id}.missing` to avoid repeated ffmpeg work.
6. The endpoint always falls back to a default SVG image.

WebSocket and `/api/now-playing` now return `cover_url` for any valid song id; the cover endpoint decides whether to return real artwork or fallback SVG.

## WebSocket Playback Flow

```text
radio-backend/src/websocket.rs
radio-backend/src/services/playback_broadcast.rs
radio-backend/src/services/playback_snapshot.rs
```

- `websocket.rs` handles connection lifecycle, heartbeat, broadcast sending, and engine command publishing.
- `playback_broadcast.rs` polls `player_handle.get_state()` every 500 ms.
- `playback_snapshot.rs` enriches engine state with DB song metadata, cover URLs, and parsed lyrics.
- Full `lyrics_lines` are sent once per song change. Later messages send only `lyrics_line`.
- Song changes are keyed by `PlaybackState.file_path`, not `playlist_index`.

## Frontend Entry Points

```text
radio-backend/frontend/src/App.vue
radio-backend/frontend/src/app/*
radio-backend/frontend/src/api/*
radio-backend/frontend/src/store.ts
```

- `App.vue` is now mostly layout and top-level lifecycle wiring.
- `src/app/useLiveAudio.ts` owns the global audio element lifecycle.
- `src/app/useThemeSync.ts` owns Vuetify/system theme synchronization.
- `src/app/filePlayback.ts` contains the backup MediaSource file playback mode.
- `src/store.ts` is still the global reactive store. It is a future candidate for domain splitting.

## Frontend WebSocket Layout

```text
radio-backend/frontend/src/api/websocket.ts
radio-backend/frontend/src/api/websocket/connection.ts
radio-backend/frontend/src/api/websocket/messages.ts
radio-backend/frontend/src/api/websocket/interpolation.ts
radio-backend/frontend/src/api/websocket/fallbackPolling.ts
```

- `api/websocket.ts` is a compatibility aggregation layer and owns poller start/stop.
- `connection.ts` owns WebSocket URL construction, connect/reconnect, and `getWs()`.
- `messages.ts` handles WebSocket message types and updates store state.
- `interpolation.ts` smooths playback progress between server updates.
- `fallbackPolling.ts` polls `/api/now-playing` only when WebSocket is not open.

## Where To Put New Work

- New HTTP endpoint: add a handler in `routes/`, keep it thin, move reusable logic to `services/`.
- New queue behavior: start in `services/queue.rs`; split into submodules when editing becomes painful.
- Public queue responses must not expose real requester names; `/api/queue/history` is admin-only.
- New playback/now-playing behavior: use `services/playback_snapshot.rs` unless it is connection-specific.
- New WebSocket connection behavior: use backend `websocket.rs` or frontend `api/websocket/connection.ts`.
- New WebSocket message handling: backend message type in `models/ws.rs`, frontend handling in `api/websocket/messages.ts`.
- New artwork behavior: use `services/metadata.rs` for now.
- New frontend UI state: currently add to `store.ts`, but prefer grouping related state and functions so it can be moved later.

## Verification Commands

From the repository root or listed directories:

```bash
cd radio-backend && cargo build
cd radio-engine && cargo test ring_buffer
cd radio-backend/frontend && npm run build
```

Run the frontend build whenever `.vue` or `.ts` files change; it also refreshes `radio-backend/static/`.
