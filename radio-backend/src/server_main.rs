#![allow(dead_code)]

/// RakurakuMusicWorld — Dedicated Server Entrypoint (Headless)
///
/// Runs RakurakuMusicWorld as a headless dedicated server without static frontend
/// file dependencies, suitable for server, Linux container, and systemd deployments.
mod app;
mod auth;
mod config;
mod db;
mod error;
mod http;
mod lyrics;
mod models;
mod physical;
mod routes;
mod services;
mod websocket;
mod world;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Default to headless mode for dedicated server
    std::env::set_var("RADIO_HEADLESS", "1");

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-c" | "--config" => {
                if i + 1 < args.len() {
                    std::env::set_var("RADIO_CONFIG", &args[i + 1]);
                    i += 1;
                }
            }
            "-h" | "--help" => {
                println!("RakurakuMusicWorld — Dedicated Headless Server");
                println!();
                println!("Usage: rakuraku-music-world-server [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -c, --config <PATH>  Path to configuration file (default: config.toml)");
                println!("  -h, --help           Print help information");
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    app::bootstrap::run().await
}
