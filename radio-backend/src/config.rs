/// 应用配置，从 config.toml 加载，支持通过环境变量覆盖。
/// 环境变量使用 `RADIO_` 前缀和大写路径表示法。
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub audio_engine: AudioEngineConfig,
    pub device: DeviceConfig,
    pub queue: QueueConfig,
    pub station: StationConfig,
    pub logging: LoggingConfig,
    #[serde(default)]
    pub ncm: NcmConfig,
    #[serde(default)]
    pub discovery: DiscoveryConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_base_path")]
    pub base_path: String,
    /// 是否以 headless / dedicated 模式运行（不提供前端静态文件服务，仅提供 API/WS/Stream）
    #[serde(default)]
    pub headless: bool,
    /// 允许的 CORS Origin 白名单；None 或空则不启用跨域凭证。
    #[serde(default)]
    pub allowed_origins: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_sqlite_url")]
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AudioEngineConfig {
    #[serde(default = "default_media_path")]
    pub media_path: String,
    /// 音频流 URL — 可以是绝对路径 (http://...) 或相对路径 (/stream)。
    /// 相对路径会与 base_url 拼接。
    #[serde(default = "default_stream_base")]
    pub stream_base: String,
    /// 是否启用交叉淡入淡出
    #[serde(default)]
    pub crossfade_enabled: bool,
}

impl AudioEngineConfig {
    /// 将 stream_base 解析为完整路径。
    /// - "auto"     → 根据请求头动态推断（默认）
    /// - "/stream"  → 相对路径
    /// - "http://..." → 绝对路径
    pub fn resolve_stream_url(
        &self,
        headers: Option<&axum::http::HeaderMap>,
        server_port: u16,
        base_path: &str,
    ) -> String {
        match self.stream_base.as_str() {
            "auto" => match headers {
                Some(h) => Self::infer_from_headers(h, server_port, base_path),
                None => join_base_path(base_path, "/stream"),
            },
            url if url.starts_with("http://") || url.starts_with("https://") => url.to_string(),
            path if path.starts_with('/') => join_base_path(base_path, path),
            path => path.to_string(),
        }
    }

    fn infer_from_headers(
        headers: &axum::http::HeaderMap,
        server_port: u16,
        base_path: &str,
    ) -> String {
        let proto = headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("http");
        let host = headers
            .get("x-forwarded-host")
            .or_else(|| headers.get("host"))
            .and_then(|v| v.to_str().ok())
            .unwrap_or("localhost");

        let stream_path = join_base_path(base_path, "/stream");

        if host.contains(':') {
            format!("{}://{}{}", proto, host, stream_path)
        } else if (proto == "https" && server_port != 443) || (proto == "http" && server_port != 80)
        {
            format!("{}://{}:{}{}", proto, host, server_port, stream_path)
        } else {
            format!("{}://{}{}", proto, host, stream_path)
        }
    }

    /// 构建当前播放曲目的文件 URL。
    pub fn resolve_file_url(&self, song_id: i64, base_path: &str) -> String {
        join_base_path(base_path, &format!("/api/songs/{}/file", song_id))
    }

    /// 构建当前播放曲目的封面 URL。
    pub fn resolve_cover_url(&self, song_id: i64, base_path: &str) -> String {
        join_base_path(base_path, &format!("/api/songs/{}/cover", song_id))
    }
}

/// 设备身份验证配置
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceConfig {
    /// Cookie 最大存活天数
    #[serde(default = "default_cookie_max_age_days")]
    pub cookie_max_age_days: u64,
    /// 管理员设置令牌（在 claim-admin 端点中输入此令牌以升级为管理员）
    #[serde(default = "default_admin_setup_token")]
    pub admin_setup_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueueConfig {
    #[serde(default = "default_max_queue_size")]
    pub max_size: usize,
    #[serde(default = "default_max_user_submissions")]
    pub max_user_submissions: usize,
    #[serde(default = "default_rate_limit_window")]
    pub rate_limit_window_secs: u64,
    #[serde(default = "default_request_cooldown")]
    pub request_cooldown_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StationConfig {
    #[serde(default = "default_station_name")]
    pub name: String,
    #[serde(default = "default_station_short_name")]
    pub short_name: String,
    #[serde(default = "default_subtitle")]
    pub subtitle: String,
    #[serde(default = "default_station_description")]
    pub description: String,
    #[serde(default)]
    pub icon_url: String,
    #[serde(default)]
    pub icon_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct NcmConfig {
    /// 网易云设备 ID（32 位 hex，首次自动生成）
    #[serde(default = "default_device_id")]
    pub device_id: String,
    /// 批量下载并发数（默认 1，最大 8）
    #[serde(default = "default_download_concurrency")]
    pub download_concurrency: usize,
}

fn default_device_id() -> String {
    String::new()
}
fn default_download_concurrency() -> usize {
    1
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscoveryConfig {
    /// 是否开启局域网 UDP 广播与发现服务
    #[serde(default = "default_discovery_enabled")]
    pub enabled: bool,
    /// 发现监听与广播的 UDP 端口（默认 42241）
    #[serde(default = "default_discovery_port")]
    pub port: u16,
    /// 发送信标广播的间隔秒数（默认 5 秒）
    #[serde(default = "default_discovery_interval")]
    pub interval_secs: u64,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            enabled: default_discovery_enabled(),
            port: default_discovery_port(),
            interval_secs: default_discovery_interval(),
        }
    }
}

fn default_discovery_enabled() -> bool {
    true
}
fn default_discovery_port() -> u16 {
    42241
}
fn default_discovery_interval() -> u64 {
    5
}

// ─── 默认值 ─────────────────────────────────────────────

fn default_host() -> String {
    "0.0.0.0".into()
}
fn default_port() -> u16 {
    2241
}
fn default_base_path() -> String {
    "/".into()
}
fn default_sqlite_url() -> String {
    "sqlite://data/radio.db?mode=rwc".into()
}
fn default_media_path() -> String {
    "./media".into()
}
fn default_stream_base() -> String {
    "auto".into()
}
fn default_cookie_max_age_days() -> u64 {
    365
}
fn default_admin_setup_token() -> String {
    String::new()
}
fn default_max_queue_size() -> usize {
    100
}
fn default_max_user_submissions() -> usize {
    3
}
fn default_rate_limit_window() -> u64 {
    300
}
fn default_request_cooldown() -> u64 {
    0
}
fn default_station_name() -> String {
    "RakurakuMusicWorld".into()
}
fn default_station_short_name() -> String {
    "RakurakuWorld".into()
}
fn default_subtitle() -> String {
    "A Community Radio".into()
}
fn default_station_description() -> String {
    "Community Radio - Low Latency Audio Streaming".into()
}
fn default_log_level() -> String {
    "info".into()
}

impl AppConfig {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut config: Self = toml::from_str(&content)?;

        if let Ok(v) = std::env::var("RADIO_DATABASE_URL") {
            config.database.url = v;
        }
        if let Ok(v) = std::env::var("RADIO_SERVER_PORT") {
            config.server.port = v.parse().unwrap_or(config.server.port);
        }
        if let Ok(v) = std::env::var("RADIO_BASE_PATH") {
            config.server.base_path = v;
        }
        if let Ok(v) = std::env::var("RADIO_LOG_LEVEL") {
            config.logging.level = v;
        }
        if let Ok(v) = std::env::var("RADIO_MEDIA_PATH") {
            config.audio_engine.media_path = v;
        }
        if let Ok(v) = std::env::var("RADIO_STREAM_BASE") {
            config.audio_engine.stream_base = v;
        }
        if let Ok(v) = std::env::var("RADIO_CROSSFADE_ENABLED") {
            config.audio_engine.crossfade_enabled = v.parse().unwrap_or(false);
        }
        if let Ok(v) = std::env::var("RADIO_STATION_NAME") {
            config.station.name = v;
        }
        if let Ok(v) = std::env::var("RADIO_ADMIN_SETUP_TOKEN") {
            config.device.admin_setup_token = v;
        }
        if let Ok(v) = std::env::var("RADIO_NCM_DEVICE_ID") {
            config.ncm.device_id = v;
        }
        if let Ok(v) = std::env::var("RADIO_NCM_DOWNLOAD_CONCURRENCY") {
            config.ncm.download_concurrency = v.parse().unwrap_or(config.ncm.download_concurrency);
        }
        if let Ok(v) = std::env::var("RADIO_HEADLESS") {
            let v = v.trim();
            config.server.headless = v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes");
        }
        if let Ok(v) = std::env::var("RADIO_DISCOVERY_ENABLED") {
            let v = v.trim();
            config.discovery.enabled = v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes");
        }
        if let Ok(v) = std::env::var("RADIO_DISCOVERY_PORT") {
            config.discovery.port = v.parse().unwrap_or(config.discovery.port);
        }

        config.server.base_path = normalize_base_path(&config.server.base_path);

        Ok(config)
    }

    pub fn load_default() -> anyhow::Result<Self> {
        let path = std::env::var("RADIO_CONFIG").unwrap_or_else(|_| "config.toml".to_string());
        Self::load(&path)
    }
}

pub fn normalize_base_path(path: &str) -> String {
    let trimmed = path.trim().trim_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", trimmed)
    }
}

pub fn join_base_path(base_path: &str, path: &str) -> String {
    let base = normalize_base_path(base_path);
    let path = if path.starts_with('/') {
        path
    } else {
        &format!("/{}", path)
    };
    if base == "/" {
        path.to_string()
    } else {
        format!("{}{}", base, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_config_defaults_headless_to_false() {
        let toml_str = r#"
            [server]
            port = 2241
            [database]
            url = "sqlite://:memory:"
            [audio_engine]
            media_path = "./media"
            [device]
            [queue]
            [station]
            name = "Test"
            short_name = "T"
            subtitle = ""
            description = ""
            [logging]
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.server.headless);
    }

    #[test]
    fn server_config_accepts_headless_true() {
        let toml_str = r#"
            [server]
            headless = true
            [database]
            url = "sqlite://:memory:"
            [audio_engine]
            media_path = "./media"
            [device]
            [queue]
            [station]
            name = "Test"
            short_name = "T"
            subtitle = ""
            description = ""
            [logging]
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(config.server.headless);
    }

    #[test]
    fn discovery_config_defaults() {
        let toml_str = r#"
            [server]
            [database]
            url = "sqlite://:memory:"
            [audio_engine]
            media_path = "./media"
            [device]
            [queue]
            [station]
            name = "Test"
            short_name = "T"
            subtitle = ""
            description = ""
            [logging]
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(config.discovery.enabled);
        assert_eq!(config.discovery.port, 42241);
        assert_eq!(config.discovery.interval_secs, 5);
    }
}

