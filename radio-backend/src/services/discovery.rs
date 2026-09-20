//! LAN Discovery Service for RakurakuMusicWorld.
//!
//! Uses UDP broadcast on a dedicated port (default 42241) to announce and discover
//! peer worlds on the local network, decoupled from the World playback/REST protocol.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

/// Discovery UDP packet types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum DiscoveryPacket {
    #[serde(rename = "rakuraku_world_beacon")]
    Beacon {
        world_id: String,
        station_name: String,
        short_name: String,
        port: u16,
        base_path: String,
        version: String,
        is_headless: bool,
    },
    #[serde(rename = "rakuraku_world_probe")]
    Probe,
}

/// A discovered peer world on the LAN.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredWorld {
    pub world_id: String,
    pub station_name: String,
    pub short_name: String,
    pub host: String,
    pub port: u16,
    pub base_path: String,
    pub url: String,
    pub version: String,
    pub is_headless: bool,
    pub last_seen_ms: i64,
}

pub struct DiscoveryService {
    self_world_id: String,
    self_station_name: Arc<RwLock<String>>,
    self_short_name: Arc<RwLock<String>>,
    self_port: u16,
    self_base_path: String,
    self_is_headless: bool,
    discovered: Arc<DashMap<String, DiscoveredWorld>>,
    socket: Arc<UdpSocket>,
    broadcast_addr: SocketAddr,
    ttl: Duration,
}

impl DiscoveryService {
    /// Initialize and bind the discovery service on the specified UDP port.
    pub async fn start(
        world_id: String,
        station_name: Arc<RwLock<String>>,
        short_name: Arc<RwLock<String>>,
        port: u16,
        base_path: String,
        is_headless: bool,
        discovery_port: u16,
        interval: Duration,
    ) -> anyhow::Result<Arc<Self>> {
        let bind_addr: SocketAddr = format!("0.0.0.0:{}", discovery_port).parse()?;
        let socket2_sock = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        )?;
        socket2_sock.set_reuse_address(true)?;
        #[cfg(unix)]
        let _ = socket2_sock.set_reuse_port(true);
        socket2_sock.set_nonblocking(true)?;
        socket2_sock.set_broadcast(true)?;
        socket2_sock.bind(&bind_addr.into())?;
        let std_socket: std::net::UdpSocket = socket2_sock.into();
        let socket = UdpSocket::from_std(std_socket)?;

        let broadcast_addr: SocketAddr = format!("255.255.255.255:{}", discovery_port).parse()?;
        let discovered = Arc::new(DashMap::new());

        let service = Arc::new(Self {
            self_world_id: world_id,
            self_station_name: station_name,
            self_short_name: short_name,
            self_port: port,
            self_base_path: base_path,
            self_is_headless: is_headless,
            discovered: discovered.clone(),
            socket: Arc::new(socket),
            broadcast_addr,
            ttl: Duration::from_secs(30),
        });

        // Spawn listener loop
        let listener_svc = service.clone();
        tokio::spawn(async move {
            listener_svc.listen_loop().await;
        });

        // Spawn periodic broadcast beacon loop
        let beacon_svc = service.clone();
        tokio::spawn(async move {
            beacon_svc.beacon_loop(interval).await;
        });

        tracing::info!(
            "LAN Discovery service listening on UDP :{} (broadcast interval: {:?})",
            discovery_port,
            interval
        );

        Ok(service)
    }

    /// Build the self beacon packet.
    async fn build_self_beacon(&self) -> DiscoveryPacket {
        let station_name = self.self_station_name.read().await.clone();
        let short_name = self.self_short_name.read().await.clone();
        DiscoveryPacket::Beacon {
            world_id: self.self_world_id.clone(),
            station_name,
            short_name,
            port: self.self_port,
            base_path: self.self_base_path.clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            is_headless: self.self_is_headless,
        }
    }

    /// Background loop receiving UDP discovery packets.
    async fn listen_loop(&self) {
        let mut buf = [0u8; 4096];
        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((len, peer_addr)) => {
                    if let Ok(packet) = serde_json::from_slice::<DiscoveryPacket>(&buf[..len]) {
                        self.handle_packet(packet, peer_addr).await;
                    }
                }
                Err(e) => {
                    tracing::debug!("UDP discovery recv error: {:?}", e);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }

    /// Process a received discovery packet.
    async fn handle_packet(&self, packet: DiscoveryPacket, peer_addr: SocketAddr) {
        match packet {
            DiscoveryPacket::Beacon {
                world_id,
                station_name,
                short_name,
                port,
                base_path,
                version,
                is_headless,
            } => {
                // Ignore self beacons
                if world_id == self.self_world_id {
                    return;
                }

                let host = peer_addr.ip().to_string();
                let clean_base = if base_path == "/" { "" } else { &base_path };
                let url = format!("http://{}:{}{}", host, port, clean_base);
                let now_ms = Utc::now().timestamp_millis();

                self.discovered.insert(
                    world_id.clone(),
                    DiscoveredWorld {
                        world_id,
                        station_name,
                        short_name,
                        host,
                        port,
                        base_path,
                        url,
                        version,
                        is_headless,
                        last_seen_ms: now_ms,
                    },
                );
            }
            DiscoveryPacket::Probe => {
                // Send self beacon back to the prober
                let beacon = self.build_self_beacon().await;
                if let Ok(bytes) = serde_json::to_vec(&beacon) {
                    let _ = self.socket.send_to(&bytes, peer_addr).await;
                }
            }
        }
    }

    /// Background loop sending periodic beacons.
    async fn beacon_loop(&self, interval: Duration) {
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            let beacon = self.build_self_beacon().await;
            if let Ok(bytes) = serde_json::to_vec(&beacon) {
                let _ = self.socket.send_to(&bytes, self.broadcast_addr).await;
            }
        }
    }

    /// Send an active probe broadcast to request immediate beacons from LAN peers.
    pub async fn probe(&self) -> anyhow::Result<()> {
        let probe = DiscoveryPacket::Probe;
        let bytes = serde_json::to_vec(&probe)?;
        self.socket.send_to(&bytes, self.broadcast_addr).await?;
        Ok(())
    }

    /// Get list of all currently discovered LAN worlds (excluding self, pruned by TTL).
    pub fn get_discovered_worlds(&self) -> Vec<DiscoveredWorld> {
        let now_ms = Utc::now().timestamp_millis();
        let max_age_ms = self.ttl.as_millis() as i64;

        // Prune expired entries
        self.discovered.retain(|_, world| {
            now_ms - world.last_seen_ms <= max_age_ms
        });

        let mut worlds: Vec<DiscoveredWorld> = self
            .discovered
            .iter()
            .map(|entry| entry.value().clone())
            .collect();

        worlds.sort_by(|a, b| a.station_name.cmp(&b.station_name));
        worlds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_serialization_roundtrip() {
        let beacon = DiscoveryPacket::Beacon {
            world_id: "test-id-1".into(),
            station_name: "Test Station".into(),
            short_name: "TS".into(),
            port: 2241,
            base_path: "/".into(),
            version: "3.1.0".into(),
            is_headless: false,
        };

        let json = serde_json::to_string(&beacon).unwrap();
        assert!(json.contains("rakuraku_world_beacon"));

        let deserialized: DiscoveryPacket = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, beacon);

        let probe = DiscoveryPacket::Probe;
        let json_probe = serde_json::to_string(&probe).unwrap();
        assert!(json_probe.contains("rakuraku_world_probe"));
        let deserialized_probe: DiscoveryPacket = serde_json::from_str(&json_probe).unwrap();
        assert_eq!(deserialized_probe, probe);
    }
}
