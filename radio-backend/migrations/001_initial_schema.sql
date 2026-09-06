-- 迁移 001：RakurakuMusicWorld 初始模式

-- 用户表：存储注册听众账号
CREATE TABLE IF NOT EXISTS users (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    username    TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role        TEXT NOT NULL DEFAULT 'user' CHECK (role IN ('user', 'admin')),
    banned_until DATETIME,  -- NULL = 未被封禁，否则封禁至此时
    created_at  DATETIME NOT NULL DEFAULT (datetime('now'))
);

-- 歌曲表：音乐库（元数据）
CREATE TABLE IF NOT EXISTS songs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    title       TEXT NOT NULL,
    artist      TEXT NOT NULL DEFAULT '',
    album       TEXT NOT NULL DEFAULT '',
    genre       TEXT NOT NULL DEFAULT '',
    year        INTEGER DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    file_path   TEXT NOT NULL,          -- media/ 目录内的相对路径
    lyrics_path TEXT NOT NULL DEFAULT '', -- .lrc 歌词文件的相对路径（或为空）
    cover_path  TEXT NOT NULL DEFAULT '', -- 封面图片的相对路径（或为空）
    filesize    INTEGER NOT NULL DEFAULT 0,
    created_at  DATETIME NOT NULL DEFAULT (datetime('now'))
);

-- 歌曲索引，用于曲库搜索
CREATE INDEX IF NOT EXISTS idx_songs_title  ON songs(title);
CREATE INDEX IF NOT EXISTS idx_songs_artist ON songs(artist);

-- 歌单表：用户创建的个人歌单
CREATE TABLE IF NOT EXISTS playlists (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    is_public   INTEGER NOT NULL DEFAULT 0,
    created_at  DATETIME NOT NULL DEFAULT (datetime('now'))
);

-- 歌单项目表：歌单中的歌曲
CREATE TABLE IF NOT EXISTS playlist_songs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    song_id     INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL DEFAULT 0,
    added_at    DATETIME NOT NULL DEFAULT (datetime('now')),
    UNIQUE(playlist_id, song_id)
);

-- 队列表：共享的电台队列（先进先出）
CREATE TABLE IF NOT EXISTS queue_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    song_id     INTEGER NOT NULL REFERENCES songs(id) ON DELETE SET NULL,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE SET NULL,
    -- status：pending（等待中）、playing（正在播放）、played（已播放）、skipped（已跳过）
    status      TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'playing', 'played', 'skipped')),
    position    INTEGER NOT NULL DEFAULT 0,  -- 队列中的顺序（越小越靠前）
    added_at    DATETIME NOT NULL DEFAULT (datetime('now')),
    played_at   DATETIME
);

-- 按位置高效获取队列的索引
CREATE INDEX IF NOT EXISTS idx_queue_status_pos ON queue_items(status, position);

-- 播放历史表：之前播放过的歌曲
CREATE TABLE IF NOT EXISTS play_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    song_id     INTEGER NOT NULL REFERENCES songs(id) ON DELETE SET NULL,
    user_id     INTEGER REFERENCES users(id) ON DELETE SET NULL,  -- 点歌人
    played_at   DATETIME NOT NULL DEFAULT (datetime('now'))
);

-- 管理员操作日志表：用于审计管理员操作
CREATE TABLE IF NOT EXISTS admin_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    admin_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    action      TEXT NOT NULL,
    details     TEXT NOT NULL DEFAULT '',
    created_at  DATETIME NOT NULL DEFAULT (datetime('now'))
);

-- 用户收藏表：用户收藏的歌曲或歌单
CREATE TABLE IF NOT EXISTS favorites (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    song_id     INTEGER REFERENCES songs(id) ON DELETE CASCADE,
    playlist_id INTEGER REFERENCES playlists(id) ON DELETE CASCADE,
    created_at  DATETIME NOT NULL DEFAULT (datetime('now')),
    CHECK (song_id IS NOT NULL OR playlist_id IS NOT NULL)
);
