//! Persistent local-scan and online-enrichment jobs.

use crate::models::Song;
use crate::services::metadata::{
    cache_lyrics, ensure_cover_cached, find_cover, find_lyrics, read_local_metadata,
};
use crate::world::{WorldCommand, WorldCommandDispatcher};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::mpsc;

const AUTO_MATCH_SCORE: i32 = 80;
const AUTO_MATCH_MARGIN: i32 = 10;
const MAX_ASSET_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct CreateMetadataJob {
    #[serde(default = "default_job_kind")]
    pub kind: String,
    #[serde(default = "default_job_scope")]
    pub scope: String,
    #[serde(default)]
    pub song_ids: Vec<i64>,
    #[serde(default)]
    pub force: bool,
}

fn default_job_kind() -> String {
    "full".into()
}
fn default_job_scope() -> String {
    "library".into()
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct MetadataJob {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub scope: String,
    pub total: i64,
    pub processed: i64,
    pub matched: i64,
    pub needs_review: i64,
    pub failed: i64,
    pub error: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub finished_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct MetadataJobItem {
    pub id: i64,
    pub job_id: String,
    pub song_id: i64,
    pub status: String,
    pub stage: String,
    pub message: String,
    pub candidates_json: String,
    pub attempts: i64,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataCandidate {
    pub provider: String,
    pub external_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: i64,
    pub score: i32,
    pub cover_url: Option<String>,
    pub release_group_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct MetadataFieldState {
    pub field_name: String,
    pub source: String,
    pub locked: bool,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Clone)]
pub struct MetadataJobManager {
    tx: mpsc::Sender<String>,
}

impl MetadataJobManager {
    pub async fn new(
        db: SqlitePool,
        media_root: PathBuf,
        commands: WorldCommandDispatcher,
        revision_signal: Arc<AtomicU64>,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<String>(32);
        let worker_db = db.clone();
        tokio::spawn(async move {
            while let Some(job_id) = rx.recv().await {
                if let Err(error) = run_job(
                    &worker_db,
                    &media_root,
                    &commands,
                    &revision_signal,
                    &job_id,
                )
                .await
                {
                    tracing::error!(job_id, ?error, "metadata job failed");
                    let _ = sqlx::query(
                        "UPDATE metadata_jobs SET status='failed', error=?, updated_at=datetime('now'), finished_at=datetime('now') WHERE id=?",
                    )
                    .bind(error.to_string())
                    .bind(&job_id)
                    .execute(&worker_db)
                    .await;
                }
            }
        });

        // A process can die between any two item updates. Reset unfinished
        // work to queued and feed it back to the single worker in order.
        let _ = sqlx::query("UPDATE metadata_jobs SET status='queued', updated_at=datetime('now') WHERE status='running'")
            .execute(&db)
            .await;
        let resumable = sqlx::query_scalar::<_, String>(
            "SELECT id FROM metadata_jobs WHERE status='queued' ORDER BY created_at",
        )
        .fetch_all(&db)
        .await
        .unwrap_or_default();
        for id in resumable {
            let _ = tx.send(id).await;
        }
        Self { tx }
    }

    pub async fn create(&self, db: &SqlitePool, request: CreateMetadataJob) -> Result<MetadataJob> {
        if !matches!(request.kind.as_str(), "local" | "online" | "full") {
            anyhow::bail!("kind must be local, online, or full");
        }
        if !matches!(request.scope.as_str(), "library" | "songs") {
            anyhow::bail!("scope must be library or songs");
        }
        if request.scope == "songs" && request.song_ids.is_empty() {
            anyhow::bail!("song_ids is required for song scope");
        }
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO metadata_jobs (id,kind,status,scope,song_ids_json,force) VALUES (?,?,'queued',?,?,?)",
        )
        .bind(&id)
        .bind(&request.kind)
        .bind(&request.scope)
        .bind(serde_json::to_string(&request.song_ids)?)
        .bind(request.force)
        .execute(db)
        .await?;
        self.tx
            .send(id.clone())
            .await
            .context("metadata worker stopped")?;
        get_job(db, &id).await
    }

    pub async fn wake(&self, id: String) -> Result<()> {
        self.tx.send(id).await.context("metadata worker stopped")
    }
}

pub async fn get_job(db: &SqlitePool, id: &str) -> Result<MetadataJob> {
    sqlx::query_as::<_, MetadataJob>(
        "SELECT id,kind,status,scope,total,processed,matched,needs_review,failed,error,created_at,updated_at,finished_at FROM metadata_jobs WHERE id=?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?
    .context("metadata job not found")
}

pub async fn get_job_items(
    db: &SqlitePool,
    id: &str,
    status: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<MetadataJobItem>> {
    let rows = if let Some(status) = status.filter(|value| !value.is_empty()) {
        sqlx::query_as::<_, MetadataJobItem>(
            "SELECT id,job_id,song_id,status,stage,message,candidates_json,attempts,updated_at FROM metadata_job_items WHERE job_id=? AND status=? ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(id).bind(status).bind(limit).bind(offset).fetch_all(db).await?
    } else {
        sqlx::query_as::<_, MetadataJobItem>(
            "SELECT id,job_id,song_id,status,stage,message,candidates_json,attempts,updated_at FROM metadata_job_items WHERE job_id=? ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(id).bind(limit).bind(offset).fetch_all(db).await?
    };
    Ok(rows)
}

pub async fn cancel_job(db: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("UPDATE metadata_jobs SET status='cancelled', updated_at=datetime('now'), finished_at=datetime('now') WHERE id=? AND status IN ('queued','running')")
        .bind(id).execute(db).await?;
    sqlx::query("UPDATE metadata_job_items SET status='cancelled', updated_at=datetime('now') WHERE job_id=? AND status IN ('queued','running')")
        .bind(id).execute(db).await?;
    Ok(())
}

pub async fn retry_job(
    db: &SqlitePool,
    manager: &MetadataJobManager,
    id: &str,
) -> Result<MetadataJob> {
    sqlx::query("UPDATE metadata_job_items SET status='queued', stage='queued', message='', attempts=attempts+1, updated_at=datetime('now') WHERE job_id=? AND status IN ('failed','needs_review','cancelled')")
        .bind(id).execute(db).await?;
    sqlx::query("UPDATE metadata_jobs SET status='queued', processed=0, matched=0, needs_review=0, failed=0, error='', finished_at=NULL, updated_at=datetime('now') WHERE id=?")
        .bind(id).execute(db).await?;
    manager.wake(id.to_string()).await?;
    get_job(db, id).await
}

async fn run_job(
    db: &SqlitePool,
    media_root: &Path,
    commands: &WorldCommandDispatcher,
    revision_signal: &Arc<AtomicU64>,
    job_id: &str,
) -> Result<()> {
    let config = sqlx::query_as::<_, (String, String, String, bool, String)>(
        "SELECT kind,scope,song_ids_json,force,status FROM metadata_jobs WHERE id=?",
    )
    .bind(job_id)
    .fetch_one(db)
    .await?;
    if matches!(config.4.as_str(), "cancelled" | "completed" | "failed") {
        return Ok(());
    }
    sqlx::query("UPDATE metadata_jobs SET status='running', error='', updated_at=datetime('now') WHERE id=? AND status='queued'")
        .bind(job_id).execute(db).await?;

    if config.1 == "library" && matches!(config.0.as_str(), "local" | "full") {
        discover_library(db, media_root).await?;
    }
    let song_ids = if config.1 == "songs" {
        serde_json::from_str::<Vec<i64>>(&config.2).unwrap_or_default()
    } else {
        sqlx::query_scalar::<_, i64>("SELECT id FROM songs WHERE file_path != '' ORDER BY id")
            .fetch_all(db)
            .await?
    };
    for song_id in &song_ids {
        sqlx::query("INSERT INTO metadata_job_items (job_id,song_id,status) VALUES (?,?,'queued') ON CONFLICT(job_id,song_id) DO NOTHING")
            .bind(job_id).bind(song_id).execute(db).await?;
    }
    sqlx::query("UPDATE metadata_jobs SET total=?, updated_at=datetime('now') WHERE id=?")
        .bind(song_ids.len() as i64)
        .bind(job_id)
        .execute(db)
        .await?;

    for song_id in song_ids {
        let status: String = sqlx::query_scalar("SELECT status FROM metadata_jobs WHERE id=?")
            .bind(job_id)
            .fetch_one(db)
            .await?;
        if status == "cancelled" {
            break;
        }
        let item_status: String = sqlx::query_scalar(
            "SELECT status FROM metadata_job_items WHERE job_id=? AND song_id=?",
        )
        .bind(job_id)
        .bind(song_id)
        .fetch_one(db)
        .await?;
        if !matches!(item_status.as_str(), "queued" | "running") {
            continue;
        }
        sqlx::query("UPDATE metadata_job_items SET status='running', stage='local', updated_at=datetime('now') WHERE job_id=? AND song_id=?")
            .bind(job_id).bind(song_id).execute(db).await?;

        let result = process_song(db, media_root, song_id, &config.0, config.3).await;
        let (item_status, stage, message, candidates) = match result {
            Ok(result) => result,
            Err(error) => (
                "failed".to_string(),
                "failed".to_string(),
                error.to_string(),
                Vec::new(),
            ),
        };
        sqlx::query("UPDATE metadata_job_items SET status=?,stage=?,message=?,candidates_json=?,updated_at=datetime('now') WHERE job_id=? AND song_id=?")
            .bind(&item_status).bind(stage).bind(message).bind(serde_json::to_string(&candidates)?)
            .bind(job_id).bind(song_id).execute(db).await?;
        refresh_job_counts(db, job_id).await?;
        if item_status == "updated" {
            revision_signal.fetch_add(1, Ordering::SeqCst);
        }
    }

    let status: String = sqlx::query_scalar("SELECT status FROM metadata_jobs WHERE id=?")
        .bind(job_id)
        .fetch_one(db)
        .await?;
    if status != "cancelled" {
        refresh_job_counts(db, job_id).await?;
        sqlx::query("UPDATE metadata_jobs SET status='completed', updated_at=datetime('now'), finished_at=datetime('now') WHERE id=?")
            .bind(job_id).execute(db).await?;
    }
    commands.dispatch(WorldCommand::ReloadQueue);
    Ok(())
}

async fn refresh_job_counts(db: &SqlitePool, job_id: &str) -> Result<()> {
    let (processed, matched, review, failed): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT SUM(CASE WHEN status NOT IN ('queued','running') THEN 1 ELSE 0 END), SUM(CASE WHEN status='updated' THEN 1 ELSE 0 END), SUM(CASE WHEN status='needs_review' THEN 1 ELSE 0 END), SUM(CASE WHEN status='failed' THEN 1 ELSE 0 END) FROM metadata_job_items WHERE job_id=?",
    )
    .bind(job_id).fetch_one(db).await?;
    sqlx::query("UPDATE metadata_jobs SET processed=?,matched=?,needs_review=?,failed=?,updated_at=datetime('now') WHERE id=?")
        .bind(processed).bind(matched).bind(review).bind(failed).bind(job_id).execute(db).await?;
    Ok(())
}

fn walk_audio_files(root: &Path) -> Vec<PathBuf> {
    fn visit(dir: &Path, output: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .file_name()
                    .is_some_and(|name| name == ".covers" || name == ".lyrics")
                {
                    continue;
                }
                if path.is_dir() {
                    visit(&path, output);
                } else if path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|ext| {
                        radio_engine::config::SUPPORTED_FORMATS
                            .iter()
                            .any(|supported| supported.eq_ignore_ascii_case(ext))
                    })
                {
                    output.push(path);
                }
            }
        }
    }
    let mut output = Vec::new();
    visit(root, &mut output);
    output.sort();
    output
}

async fn discover_library(db: &SqlitePool, media_root: &Path) -> Result<()> {
    for path in walk_audio_files(media_root) {
        let relative = radio_engine::util::relativize_media_path(&path, media_root);
        if sqlx::query_scalar::<_, i64>("SELECT id FROM songs WHERE file_path=?")
            .bind(&relative)
            .fetch_optional(db)
            .await?
            .is_some()
        {
            continue;
        }
        let metadata = read_local_metadata(&path);
        sqlx::query("INSERT INTO songs (title,artist,album,file_path,duration_ms,filesize,metadata_source) VALUES (?,?,?,?,?,?,'local')")
            .bind(&metadata.title).bind(&metadata.artist).bind(&metadata.album).bind(&relative)
            .bind(metadata.duration_ms).bind(metadata.filesize).execute(db).await?;
    }
    Ok(())
}

async fn process_song(
    db: &SqlitePool,
    media_root: &Path,
    song_id: i64,
    kind: &str,
    force: bool,
) -> Result<(String, String, String, Vec<MetadataCandidate>)> {
    let mut changed = false;
    if matches!(kind, "local" | "full") {
        changed |= apply_local_metadata(db, media_root, song_id, force).await?;
    }
    if matches!(kind, "online" | "full") {
        let song = load_song(db, song_id).await?;
        let mut candidates = search_ncm_candidates(&song).await.unwrap_or_else(|error| {
            tracing::warn!(song_id, ?error, "NCM metadata search failed");
            Vec::new()
        });
        if candidates.iter().map(|item| item.score).max().unwrap_or(0) < 60 {
            candidates.extend(
                search_musicbrainz_candidates(&song)
                    .await
                    .unwrap_or_else(|error| {
                        tracing::warn!(song_id, ?error, "MusicBrainz metadata search failed");
                        Vec::new()
                    }),
            );
        }
        candidates.sort_by(|a, b| b.score.cmp(&a.score));
        candidates.dedup_by(|a, b| a.provider == b.provider && a.external_id == b.external_id);
        let first = candidates.first();
        let margin = first.map(|item| item.score).unwrap_or(0)
            - candidates.get(1).map(|item| item.score).unwrap_or(0);
        let artist_known = !song.artist.trim().is_empty();
        let artist_ok = first.is_some_and(|item| artist_matches(&song.artist, &item.artists));
        if first.is_some_and(|item| item.score >= AUTO_MATCH_SCORE)
            && margin >= AUTO_MATCH_MARGIN
            && (!artist_known || artist_ok)
        {
            apply_candidate(db, media_root, song_id, first.unwrap()).await?;
            changed = true;
        } else if !candidates.is_empty() {
            return Ok((
                "needs_review".into(),
                "online".into(),
                "需要管理员确认在线候选".into(),
                candidates,
            ));
        }
    }
    Ok(if changed {
        (
            "updated".into(),
            "complete".into(),
            "元数据已更新".into(),
            Vec::new(),
        )
    } else {
        (
            "skipped".into(),
            "complete".into(),
            "没有需要更新的字段".into(),
            Vec::new(),
        )
    })
}

async fn load_song(db: &SqlitePool, id: i64) -> Result<Song> {
    sqlx::query_as::<_, Song>("SELECT * FROM songs WHERE id=?")
        .bind(id)
        .fetch_optional(db)
        .await?
        .context("song not found")
}

async fn field_state(db: &SqlitePool, song_id: i64) -> Result<HashMap<String, (String, bool)>> {
    let rows = sqlx::query_as::<_, (String, String, bool)>(
        "SELECT field_name,source,locked FROM song_metadata_fields WHERE song_id=?",
    )
    .bind(song_id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(field, source, locked)| (field, (source, locked)))
        .collect())
}

async fn set_field_state(
    db: &SqlitePool,
    song_id: i64,
    field: &str,
    source: &str,
    locked: bool,
) -> Result<()> {
    sqlx::query("INSERT INTO song_metadata_fields (song_id,field_name,source,locked) VALUES (?,?,?,?) ON CONFLICT(song_id,field_name) DO UPDATE SET source=excluded.source,locked=excluded.locked,updated_at=datetime('now')")
        .bind(song_id).bind(field).bind(source).bind(locked).execute(db).await?;
    Ok(())
}

fn can_replace(state: Option<&(String, bool)>, current_empty: bool, local: bool) -> bool {
    match state {
        Some((_, true)) => false,
        None => current_empty,
        Some((source, false)) if local => matches!(
            source.as_str(),
            "filename" | "local_tag" | "ncm" | "musicbrainz" | "online"
        ),
        Some((source, false)) => {
            current_empty
                || matches!(
                    source.as_str(),
                    "filename" | "ncm" | "musicbrainz" | "online"
                )
        }
    }
}

async fn apply_local_metadata(
    db: &SqlitePool,
    media_root: &Path,
    song_id: i64,
    force: bool,
) -> Result<bool> {
    let song = load_song(db, song_id).await?;
    let full = media_root.join(&song.file_path);
    if !full.is_file() {
        anyhow::bail!("媒体文件不存在: {}", song.file_path);
    }
    let metadata = read_local_metadata(&full);
    let states = field_state(db, song_id).await?;
    let mut title = song.title.clone();
    let mut artist = song.artist.clone();
    let mut album = song.album.clone();
    let mut cover = song.cover_path.clone();
    let mut lyrics = song.lyrics_path.clone();
    let mut changed =
        metadata.duration_ms != song.duration_ms || metadata.filesize != song.filesize;

    for (field, incoming, tagged, current) in [
        (
            "title",
            metadata.title.as_str(),
            metadata.title_from_tag,
            &mut title,
        ),
        (
            "artist",
            metadata.artist.as_str(),
            metadata.artist_from_tag,
            &mut artist,
        ),
        (
            "album",
            metadata.album.as_str(),
            metadata.album_from_tag,
            &mut album,
        ),
    ] {
        if incoming.is_empty() {
            continue;
        }
        let state = states.get(field);
        let inferred_filename = state.is_none() && !tagged && *current == incoming;
        if inferred_filename {
            set_field_state(db, song_id, field, "filename", false).await?;
        }
        if can_replace(state, current.is_empty(), tagged) {
            if *current != incoming {
                *current = incoming.to_string();
                changed = true;
            }
            set_field_state(
                db,
                song_id,
                field,
                if tagged { "local_tag" } else { "filename" },
                false,
            )
            .await?;
        }
    }

    let cover_locked = states.get("cover").is_some_and(|(_, locked)| *locked);
    let sidecar = find_cover(&full, media_root);
    if !cover_locked && !sidecar.is_empty() {
        if cover != sidecar {
            cover = sidecar;
            changed = true;
        }
        set_field_state(db, song_id, "cover", "sidecar", false).await?;
    } else if !cover_locked && (force || cover.is_empty() || !media_root.join(&cover).is_file()) {
        if force {
            let marker = media_root
                .join(".covers")
                .join(format!("{}.missing", song_id));
            let _ = tokio::fs::remove_file(marker).await;
        }
        if let Some(path) =
            ensure_cover_cached(db, song_id, &song.file_path, "", media_root).await?
        {
            if cover != path {
                cover = path;
                changed = true;
            }
            set_field_state(db, song_id, "cover", "embedded", false).await?;
        } else if !cover.is_empty() && !media_root.join(&cover).is_file() {
            cover.clear();
            changed = true;
        }
    }

    let lyrics_locked = states.get("lyrics").is_some_and(|(_, locked)| *locked);
    if !lyrics_locked {
        let sidecar = find_lyrics(&full, media_root);
        if !sidecar.is_empty() {
            if lyrics != sidecar {
                lyrics = sidecar;
                changed = true;
            }
            set_field_state(db, song_id, "lyrics", "sidecar", false).await?;
        } else if !metadata.embedded_lyrics.trim().is_empty()
            && (force || lyrics.is_empty() || !media_root.join(&lyrics).is_file())
        {
            let path = cache_lyrics(media_root, song_id, &metadata.embedded_lyrics).await?;
            if lyrics != path {
                lyrics = path;
                changed = true;
            }
            set_field_state(db, song_id, "lyrics", "embedded", false).await?;
        } else if !lyrics.is_empty() && !media_root.join(&lyrics).is_file() {
            lyrics.clear();
            changed = true;
        }
    }

    if changed {
        sqlx::query("UPDATE songs SET title=?,artist=?,album=?,duration_ms=?,filesize=?,cover_path=?,lyrics_path=?,metadata_source='local',metadata_revision=metadata_revision+1 WHERE id=?")
            .bind(title).bind(artist).bind(album).bind(metadata.duration_ms).bind(metadata.filesize)
            .bind(cover).bind(lyrics).bind(song_id).execute(db).await?;
    }
    Ok(changed)
}

async fn search_ncm_candidates(song: &Song) -> Result<Vec<MetadataCandidate>> {
    let mut seen = HashSet::new();
    let queries = [
        format!("{} {}", song.title, song.artist).trim().to_string(),
        song.title.trim().to_string(),
        song.artist.trim().to_string(),
    ];
    let mut output = Vec::new();
    for query in queries.into_iter().filter(|value| !value.is_empty()) {
        for item in crate::services::ncm::api::search_song_anonymous(&query, 50).await? {
            if !seen.insert(item.id) {
                continue;
            }
            let title = item.name.clone();
            let album = item.album.name.clone();
            let artists = item
                .artists
                .iter()
                .map(|artist| artist.name.clone())
                .collect::<Vec<_>>();
            output.push(MetadataCandidate {
                provider: "ncm".into(),
                external_id: item.id.to_string(),
                title: title.clone(),
                artists: artists.clone(),
                album: album.clone(),
                duration_ms: item.duration,
                score: candidate_score(song, &title, &artists, &album, item.duration),
                cover_url: None,
                release_group_id: None,
            });
        }
    }
    Ok(output)
}

#[derive(Deserialize)]
struct MbSearchResponse {
    #[serde(default)]
    recordings: Vec<MbRecording>,
}
#[derive(Deserialize)]
struct MbRecording {
    id: String,
    title: String,
    #[serde(default)]
    length: i64,
    #[serde(rename = "artist-credit", default)]
    artist_credit: Vec<MbArtistCredit>,
    #[serde(default)]
    releases: Vec<MbRelease>,
}
#[derive(Deserialize)]
struct MbArtistCredit {
    name: String,
}
#[derive(Deserialize)]
struct MbRelease {
    #[serde(default)]
    title: String,
    #[serde(rename = "release-group")]
    release_group: Option<MbReleaseGroup>,
}
#[derive(Deserialize)]
struct MbReleaseGroup {
    id: String,
}

async fn search_musicbrainz_candidates(song: &Song) -> Result<Vec<MetadataCandidate>> {
    let query = if song.artist.trim().is_empty() {
        format!("recording:\"{}\"", song.title.replace('"', ""))
    } else {
        format!(
            "recording:\"{}\" AND artist:\"{}\"",
            song.title.replace('"', ""),
            song.artist.replace('"', "")
        )
    };
    let response = http_client()?
        .get("https://musicbrainz.org/ws/2/recording/")
        .query(&[
            ("query", query),
            ("fmt", "json".into()),
            ("limit", "25".into()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json::<MbSearchResponse>()
        .await?;
    Ok(response
        .recordings
        .into_iter()
        .map(|item| {
            let artists = item
                .artist_credit
                .into_iter()
                .map(|artist| artist.name)
                .collect::<Vec<_>>();
            let album = item
                .releases
                .first()
                .map(|release| release.title.clone())
                .unwrap_or_default();
            let release_group_id = item
                .releases
                .iter()
                .find_map(|release| release.release_group.as_ref().map(|group| group.id.clone()));
            MetadataCandidate {
                provider: "musicbrainz".into(),
                external_id: item.id,
                title: item.title.clone(),
                artists: artists.clone(),
                album: album.clone(),
                duration_ms: item.length,
                score: candidate_score(song, &item.title, &artists, &album, item.length),
                cover_url: release_group_id.as_ref().map(|id| {
                    format!("https://coverartarchive.org/release-group/{}/front-500", id)
                }),
                release_group_id,
            }
        })
        .collect())
}

fn http_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .user_agent("RakurakuMusicWorld/3.1 (https://github.com/Risaly-Noroki-Dev-Club/RakurakuMusicWorld)")
        .build()?)
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn artist_matches(local: &str, candidates: &[String]) -> bool {
    let local = normalize(local);
    local.is_empty()
        || candidates.iter().any(|candidate| {
            let candidate = normalize(candidate);
            !candidate.is_empty()
                && (local == candidate || local.contains(&candidate) || candidate.contains(&local))
        })
}

fn candidate_score(
    song: &Song,
    title: &str,
    artists: &[String],
    album: &str,
    duration_ms: i64,
) -> i32 {
    let local_title = normalize(&song.title);
    let candidate_title = normalize(title);
    let mut score = if !local_title.is_empty() && local_title == candidate_title {
        45
    } else if !local_title.is_empty()
        && !candidate_title.is_empty()
        && (local_title.contains(&candidate_title) || candidate_title.contains(&local_title))
    {
        30
    } else {
        0
    };
    if !normalize(&song.artist).is_empty() && artist_matches(&song.artist, artists) {
        score += 30;
    }
    if song.duration_ms > 0 && duration_ms > 0 {
        score += match (song.duration_ms - duration_ms).abs() {
            0..=2_000 => 20,
            2_001..=5_000 => 15,
            5_001..=10_000 => 8,
            _ => 0,
        };
    }
    if !normalize(&song.album).is_empty() && normalize(&song.album) == normalize(album) {
        score += 5;
    }
    score
}

pub async fn apply_candidate(
    db: &SqlitePool,
    media_root: &Path,
    song_id: i64,
    candidate: &MetadataCandidate,
) -> Result<()> {
    let song = load_song(db, song_id).await?;
    let states = field_state(db, song_id).await?;
    let unlocked = |field: &str| !states.get(field).is_some_and(|(_, locked)| *locked);
    let mut title = song.title.clone();
    let mut artist = song.artist.clone();
    let mut album = song.album.clone();
    let mut cover = song.cover_path.clone();
    let mut lyrics = song.lyrics_path.clone();
    let source = candidate.provider.as_str();
    for (field, incoming, current) in [
        ("title", candidate.title.clone(), &mut title),
        ("artist", candidate.artists.join(", "), &mut artist),
        ("album", candidate.album.clone(), &mut album),
    ] {
        if incoming.is_empty() || !unlocked(field) {
            continue;
        }
        let replaceable = current.is_empty()
            || states.get(field).is_some_and(|(old, _)| {
                matches!(old.as_str(), "filename" | "ncm" | "musicbrainz" | "online")
            });
        if replaceable {
            *current = incoming;
            set_field_state(db, song_id, field, source, false).await?;
        }
    }

    let mut ncm_id = song.ncm_song_id;
    let mut mb_id = song.musicbrainz_recording_id.clone();
    if source == "ncm" {
        let id = candidate.external_id.parse::<i64>()?;
        ncm_id = Some(id);
        if let Some(detail) = crate::services::ncm::api::get_song_detail_anonymous(id).await? {
            if unlocked("cover")
                && (cover.is_empty() || !media_root.join(&cover).is_file())
                && !detail.al.pic_url.is_empty()
            {
                cover = crate::services::ncm::metadata::download_cover(
                    media_root,
                    song_id,
                    &detail.al.pic_url,
                )
                .await?;
                set_field_state(db, song_id, "cover", "ncm", false).await?;
            }
        }
        if unlocked("lyrics") && (lyrics.is_empty() || !media_root.join(&lyrics).is_file()) {
            if let Some(content) = crate::services::ncm::api::get_song_lyric_anonymous(id).await? {
                if !content.trim().is_empty() {
                    lyrics = cache_lyrics(media_root, song_id, &content).await?;
                    set_field_state(db, song_id, "lyrics", "ncm", false).await?;
                }
            }
        }
    } else {
        mb_id = Some(candidate.external_id.clone());
        if unlocked("cover") && (cover.is_empty() || !media_root.join(&cover).is_file()) {
            if let Some(url) = candidate.cover_url.as_deref() {
                if let Ok(path) = download_external_cover(media_root, song_id, url).await {
                    cover = path;
                    set_field_state(db, song_id, "cover", "cover_art_archive", false).await?;
                }
            }
        }
    }
    sqlx::query("UPDATE songs SET title=?,artist=?,album=?,cover_path=?,lyrics_path=?,ncm_song_id=?,musicbrainz_recording_id=?,metadata_source=?,metadata_matched_at=datetime('now'),metadata_revision=metadata_revision+1 WHERE id=?")
        .bind(title).bind(artist).bind(album).bind(cover).bind(lyrics).bind(ncm_id).bind(mb_id).bind(source).bind(song_id).execute(db).await?;
    Ok(())
}

async fn download_external_cover(media_root: &Path, song_id: i64, url: &str) -> Result<String> {
    let parsed = reqwest::Url::parse(url)?;
    if parsed.scheme() != "https" || parsed.host_str() != Some("coverartarchive.org") {
        anyhow::bail!("untrusted cover URL");
    }
    let response = http_client()?
        .get(parsed)
        .send()
        .await?
        .error_for_status()?;
    if !response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("image/"))
    {
        anyhow::bail!("cover response is not an image");
    }
    let bytes = response.bytes().await?;
    if bytes.is_empty() || bytes.len() > MAX_ASSET_BYTES {
        anyhow::bail!("invalid cover size");
    }
    let directory = media_root.join(".covers");
    tokio::fs::create_dir_all(&directory).await?;
    let relative = format!(".covers/{}.jpg", song_id);
    let temporary = directory.join(format!("{}.jpg.part", song_id));
    tokio::fs::write(&temporary, &bytes).await?;
    tokio::fs::rename(&temporary, media_root.join(&relative)).await?;
    Ok(relative)
}

pub async fn candidates_for_song(db: &SqlitePool, song_id: i64) -> Result<Vec<MetadataCandidate>> {
    let json = sqlx::query_scalar::<_, String>(
        "SELECT candidates_json FROM metadata_job_items WHERE song_id=? AND status='needs_review' ORDER BY updated_at DESC LIMIT 1",
    ).bind(song_id).fetch_optional(db).await?.unwrap_or_else(|| "[]".into());
    Ok(serde_json::from_str(&json).unwrap_or_default())
}

pub async fn metadata_fields(db: &SqlitePool, song_id: i64) -> Result<Vec<MetadataFieldState>> {
    Ok(sqlx::query_as::<_, MetadataFieldState>(
        "SELECT field_name,source,locked,updated_at FROM song_metadata_fields WHERE song_id=? ORDER BY field_name",
    ).bind(song_id).fetch_all(db).await?)
}

#[cfg(test)]
mod tests {
    use super::{artist_matches, candidate_score, normalize};
    use crate::models::Song;
    use chrono::Utc;

    fn song() -> Song {
        Song {
            id: 1,
            title: "晴天".into(),
            artist: "周杰伦".into(),
            album: "叶惠美".into(),
            genre: "".into(),
            year: 0,
            duration_ms: 269_000,
            file_path: "周杰伦 - 晴天.mp3".into(),
            lyrics_path: "".into(),
            cover_path: "".into(),
            filesize: 0,
            created_at: Utc::now().naive_utc(),
            ncm_song_id: None,
            metadata_source: "filename".into(),
            metadata_matched_at: None,
            musicbrainz_recording_id: None,
            metadata_revision: 0,
        }
    }

    #[test]
    fn scores_exact_candidate_as_automatic_match() {
        assert_eq!(
            candidate_score(&song(), "晴天", &["周杰伦".into()], "叶惠美", 269_500),
            100
        );
    }

    #[test]
    fn rejects_wrong_artist_even_with_same_title() {
        assert!(!artist_matches("周杰伦", &["翻唱歌手".into()]));
        assert!(candidate_score(&song(), "晴天", &["翻唱歌手".into()], "", 269_000) < 80);
    }

    #[test]
    fn normalization_handles_cjk_punctuation() {
        assert_eq!(normalize("Hello，晴天!"), "hello晴天");
    }
}
