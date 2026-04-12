// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Embedded Snapcast backend — in-process server via snapcast-server crate.

use std::collections::HashMap;

use anyhow::{Context, Result};
use tokio::sync::mpsc;

use snapcast_server::{
    AudioData, AudioFrame, ServerCommand, ServerConfig, ServerEvent, SnapServer,
};

use super::backend::{BoxFuture, SnapcastBackend, SnapcastEvent};
use crate::config::AppConfig;
use crate::player::{ClientAction, GroupAction, SnapcastCmd};
use crate::state;

/// Default audio buffer size in milliseconds for the embedded server.
const DEFAULT_BUFFER_MS: u32 = 1000;

/// Per-zone pacing state: logical clock anchored at stream start.
struct ZonePacer {
    total_frames: u64,
    ts: snapcast_server::time::ChunkTimestamper,
    last_send: tokio::time::Instant,
    /// Accumulator for fixed-size chunk output (interleaved F32 samples).
    buf: Vec<f32>,
}

/// Embedded Snapcast server backend.
pub struct EmbeddedBackend {
    cmd_tx: mpsc::Sender<ServerCommand>,
    audio_txs: HashMap<usize, mpsc::Sender<AudioFrame>>,
    pacers: std::sync::Mutex<HashMap<usize, ZonePacer>>,
    store: state::SharedState,
}

/// Event receiver from the embedded server.
pub struct EmbeddedEventReceiver {
    event_rx: mpsc::Receiver<ServerEvent>,
}

impl EmbeddedBackend {
    /// Start the embedded server. Returns the backend + event receiver.
    pub async fn start(
        config: &AppConfig,
        store: state::SharedState,
    ) -> Result<(Self, EmbeddedEventReceiver)> {
        // Resolve f32lz4e → f32lz4 + encryption
        let (codec, encryption_psk) = if config.audio.codec == "f32lz4e" {
            let psk = config
                .audio
                .encryption_psk
                .clone()
                .unwrap_or_else(|| snapcast_server::DEFAULT_ENCRYPTION_PSK.into());
            ("f32lz4".into(), Some(psk))
        } else {
            (
                config.audio.codec.clone(),
                config.audio.encryption_psk.clone(),
            )
        };

        let server_config = ServerConfig {
            stream_port: config.snapcast.streaming_port,
            buffer_ms: DEFAULT_BUFFER_MS,
            codec,
            sample_format: config.audio.sample_format(),
            encryption_psk,
            state_file: Some("snapcast-state.json".into()),
            ..ServerConfig::default()
        };

        let (mut server, event_rx) = SnapServer::new(server_config);

        // One stream per zone
        let mut audio_txs = HashMap::new();
        for zone in &config.zones {
            let tx = server.add_stream(&zone.stream_name);
            audio_txs.insert(zone.index, tx);
        }

        let cmd_tx = server.command_sender();

        tokio::spawn(async move {
            if let Err(e) = server.run().await {
                tracing::error!(error = %e, "Embedded Snapcast server error");
            }
        });

        tracing::info!(
            port = config.snapcast.streaming_port,
            zones = audio_txs.len(),
            "Embedded Snapcast server started"
        );

        // mDNS is handled automatically by SnapServer::run()

        Ok((
            Self {
                cmd_tx,
                audio_txs,
                pacers: std::sync::Mutex::new(HashMap::new()),
                store,
            },
            EmbeddedEventReceiver { event_rx },
        ))
    }

    /// Map a `SnapcastCmd` to one or more `ServerCommand`s.
    fn map_command(cmd: SnapcastCmd) -> Vec<ServerCommand> {
        match cmd {
            SnapcastCmd::Group { group_id, action } => match action {
                GroupAction::Stream(stream_id) => {
                    vec![ServerCommand::SetGroupStream {
                        group_id,
                        stream_id,
                    }]
                }
                GroupAction::Clients(clients) => {
                    vec![ServerCommand::SetGroupClients { group_id, clients }]
                }
                GroupAction::Name(name) => {
                    vec![ServerCommand::SetGroupName { group_id, name }]
                }
                GroupAction::Volume(_percent) => vec![], // TODO: group volume
                GroupAction::Mute(muted) => {
                    vec![ServerCommand::SetGroupMute { group_id, muted }]
                }
            },
            SnapcastCmd::Client { client_id, action } => match action {
                ClientAction::Volume(percent) => {
                    vec![ServerCommand::SetClientVolume {
                        client_id,
                        volume: percent.clamp(0, 100) as u16,
                        muted: false, // preserve current — server merges
                    }]
                }
                ClientAction::Mute(muted) => {
                    vec![ServerCommand::SetClientVolume {
                        client_id,
                        volume: 0, // preserve current — server merges
                        muted,
                    }]
                }
                ClientAction::Latency(ms) => {
                    vec![ServerCommand::SetClientLatency {
                        client_id,
                        latency: ms,
                    }]
                }
            },
            SnapcastCmd::ReconcileZones => unreachable!("handled in execute"),
        }
    }

    /// Map a `ServerEvent` to a `SnapcastEvent`.
    fn map_event(event: ServerEvent) -> Option<SnapcastEvent> {
        match event {
            ServerEvent::ClientConnected { id, name, mac } => {
                Some(SnapcastEvent::ClientConnected { id, name, mac })
            }
            ServerEvent::ClientDisconnected { id } => {
                Some(SnapcastEvent::ClientDisconnected { id })
            }
            ServerEvent::ClientVolumeChanged {
                client_id,
                volume,
                muted,
            } => Some(SnapcastEvent::ClientVolumeChanged {
                id: client_id,
                volume: volume as i32,
                muted,
            }),
            ServerEvent::ClientLatencyChanged { client_id, latency } => {
                Some(SnapcastEvent::ClientLatencyChanged {
                    id: client_id,
                    latency,
                })
            }
            ServerEvent::ClientNameChanged { client_id, name } => {
                Some(SnapcastEvent::ClientNameChanged {
                    id: client_id,
                    name,
                })
            }
            ServerEvent::GroupStreamChanged { .. }
            | ServerEvent::GroupMuteChanged { .. }
            | ServerEvent::GroupNameChanged { .. }
            | ServerEvent::StreamStatus { .. }
            | ServerEvent::ServerUpdated => Some(SnapcastEvent::ServerUpdated),
            _ => None,
        }
    }
}

impl EmbeddedEventReceiver {
    /// Receive the next mapped event. Returns `None` when the server shuts down.
    pub async fn recv(&mut self) -> Option<SnapcastEvent> {
        loop {
            let event = self.event_rx.recv().await?;
            if let Some(mapped) = EmbeddedBackend::map_event(event) {
                return Some(mapped);
            }
        }
    }
}

impl SnapcastBackend for EmbeddedBackend {
    fn send_audio(
        &self,
        zone_index: usize,
        samples: &[f32],
        sample_rate: u32,
        channels: u16,
    ) -> BoxFuture<'_, Result<()>> {
        let Some(tx) = self.audio_txs.get(&zone_index) else {
            return Box::pin(async move { anyhow::bail!("No audio stream for zone {zone_index}") });
        };
        let ch = channels.max(1) as usize;
        // 20ms fixed chunks (matches C++ snapserver default for non-FLAC)
        let chunk_samples = (sample_rate as usize * 20 / 1000) * ch;

        // Accumulate samples and extract fixed-size chunks
        let chunks: Vec<(Vec<f32>, i64)> = {
            let mut pacers = self.pacers.lock().unwrap();
            let now = tokio::time::Instant::now();
            let p = pacers.entry(zone_index).or_insert_with(|| ZonePacer {
                total_frames: 0,
                ts: snapcast_server::time::ChunkTimestamper::new(sample_rate),
                last_send: now,
                buf: Vec::with_capacity(chunk_samples * 2),
            });

            // Reset on new playback (gap > 500ms since last send)
            if now.duration_since(p.last_send) > std::time::Duration::from_millis(500) {
                p.total_frames = 0;
                p.ts = snapcast_server::time::ChunkTimestamper::new(sample_rate);
                p.buf.clear();
            }
            p.last_send = now;

            p.buf.extend_from_slice(samples);

            let mut out = Vec::new();
            while p.buf.len() >= chunk_samples {
                let chunk: Vec<f32> = p.buf.drain(..chunk_samples).collect();
                let frames = (chunk_samples / ch) as u32;
                if p.total_frames == 0 {
                    p.ts = snapcast_server::time::ChunkTimestamper::new(sample_rate);
                }
                let timestamp_usec = p.ts.next(frames);
                p.total_frames += frames as u64;
                out.push((chunk, timestamp_usec));
            }
            out
        };

        let tx = tx.clone();
        Box::pin(async move {
            for (samples, timestamp_usec) in chunks {
                let frame = AudioFrame {
                    data: AudioData::F32(samples),
                    timestamp_usec,
                };
                tx.send(frame)
                    .await
                    .map_err(|_| anyhow::anyhow!("Audio channel closed for zone {zone_index}"))?;
            }
            Ok(())
        })
    }

    fn execute(&self, cmd: SnapcastCmd) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let commands = match &cmd {
                // Mute needs current volume — SetClientVolume sets both
                SnapcastCmd::Client {
                    client_id,
                    action: ClientAction::Mute(muted),
                } => {
                    let s = self.store.read().await;
                    let volume = s
                        .clients
                        .values()
                        .find(|c| c.snapcast_id.as_deref() == Some(client_id))
                        .map(|c| c.volume.clamp(0, 100) as u16)
                        .unwrap_or(100);
                    drop(s);
                    vec![ServerCommand::SetClientVolume {
                        client_id: client_id.clone(),
                        volume,
                        muted: *muted,
                    }]
                }
                // Volume needs current mute state — SetClientVolume sets both
                SnapcastCmd::Client {
                    client_id,
                    action: ClientAction::Volume(percent),
                } => {
                    let s = self.store.read().await;
                    let muted = s
                        .clients
                        .values()
                        .find(|c| c.snapcast_id.as_deref() == Some(client_id))
                        .map(|c| c.muted)
                        .unwrap_or(false);
                    drop(s);
                    vec![ServerCommand::SetClientVolume {
                        client_id: client_id.clone(),
                        volume: (*percent).clamp(0, 100) as u16,
                        muted,
                    }]
                }
                SnapcastCmd::ReconcileZones => {
                    // Move each connected client to its zone's Snapcast group
                    let s = self.store.read().await;
                    // Build zone_index → Vec<snapcast_id>
                    let mut zone_clients: HashMap<usize, Vec<String>> = HashMap::new();
                    for c in s.clients.values() {
                        if let Some(ref sid) = c.snapcast_id {
                            zone_clients
                                .entry(c.zone_index)
                                .or_default()
                                .push(sid.clone());
                        }
                    }
                    // For each zone that has a group, issue SetGroupClients
                    let mut cmds = vec![];
                    for (zi, clients) in &zone_clients {
                        if let Some(gid) =
                            s.zones.get(zi).and_then(|z| z.snapcast_group_id.as_ref())
                        {
                            cmds.push(ServerCommand::SetGroupClients {
                                group_id: gid.clone(),
                                clients: clients.clone(),
                            });
                        }
                    }
                    drop(s);
                    cmds
                }
                _ => Self::map_command(cmd),
            };
            for c in commands {
                self.cmd_tx
                    .send(c)
                    .await
                    .context("Server command channel closed")?;
            }
            Ok(())
        })
    }

    fn stop(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            self.cmd_tx
                .send(ServerCommand::Stop)
                .await
                .context("Failed to send stop command")
        })
    }

    fn get_status(&self) -> BoxFuture<'_, Result<serde_json::Value>> {
        Box::pin(async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            self.cmd_tx
                .send(ServerCommand::GetStatus { response_tx: tx })
                .await
                .context("Server command channel closed")?;
            let status = rx.await.context("Status response channel closed")?;
            serde_json::to_value(status).context("Failed to serialize status")
        })
    }

    fn delete_client(&self, id: &str) -> BoxFuture<'_, Result<()>> {
        let client_id = id.to_string();
        Box::pin(async move {
            self.cmd_tx
                .send(ServerCommand::DeleteClient { client_id })
                .await
                .context("Server command channel closed")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_command_group_stream() {
        let cmd = SnapcastCmd::Group {
            group_id: "g1".into(),
            action: GroupAction::Stream("Zone1".into()),
        };
        let cmds = EmbeddedBackend::map_command(cmd);
        assert_eq!(cmds.len(), 1);
        assert!(
            matches!(&cmds[0], ServerCommand::SetGroupStream { group_id, stream_id } if group_id == "g1" && stream_id == "Zone1")
        );
    }

    #[test]
    fn map_command_group_clients() {
        let cmd = SnapcastCmd::Group {
            group_id: "g1".into(),
            action: GroupAction::Clients(vec!["c1".into(), "c2".into()]),
        };
        let cmds = EmbeddedBackend::map_command(cmd);
        assert_eq!(cmds.len(), 1);
        assert!(
            matches!(&cmds[0], ServerCommand::SetGroupClients { clients, .. } if clients.len() == 2)
        );
    }

    #[test]
    fn map_command_group_mute() {
        let cmd = SnapcastCmd::Group {
            group_id: "g1".into(),
            action: GroupAction::Mute(true),
        };
        let cmds = EmbeddedBackend::map_command(cmd);
        assert!(matches!(
            &cmds[0],
            ServerCommand::SetGroupMute { muted: true, .. }
        ));
    }

    #[test]
    fn map_command_group_volume_noop() {
        let cmd = SnapcastCmd::Group {
            group_id: "g1".into(),
            action: GroupAction::Volume(50),
        };
        assert!(EmbeddedBackend::map_command(cmd).is_empty());
    }

    #[test]
    fn map_command_client_volume() {
        let cmd = SnapcastCmd::Client {
            client_id: "c1".into(),
            action: ClientAction::Volume(75),
        };
        let cmds = EmbeddedBackend::map_command(cmd);
        assert!(matches!(
            &cmds[0],
            ServerCommand::SetClientVolume { volume: 75, .. }
        ));
    }

    #[test]
    fn map_command_client_volume_clamps() {
        let cmd = SnapcastCmd::Client {
            client_id: "c1".into(),
            action: ClientAction::Volume(200),
        };
        let cmds = EmbeddedBackend::map_command(cmd);
        assert!(matches!(
            &cmds[0],
            ServerCommand::SetClientVolume { volume: 100, .. }
        ));
    }

    #[test]
    fn map_command_client_mute() {
        let cmd = SnapcastCmd::Client {
            client_id: "c1".into(),
            action: ClientAction::Mute(true),
        };
        let cmds = EmbeddedBackend::map_command(cmd);
        assert!(matches!(
            &cmds[0],
            ServerCommand::SetClientVolume { muted: true, .. }
        ));
    }

    #[test]
    fn map_command_client_latency() {
        let cmd = SnapcastCmd::Client {
            client_id: "c1".into(),
            action: ClientAction::Latency(50),
        };
        let cmds = EmbeddedBackend::map_command(cmd);
        assert!(matches!(
            &cmds[0],
            ServerCommand::SetClientLatency { latency: 50, .. }
        ));
    }

    #[test]
    fn map_event_client_connected() {
        let event = ServerEvent::ClientConnected {
            id: "c1".into(),
            name: "Kitchen".into(),
            mac: "aa:bb:cc:dd:ee:ff".into(),
        };
        let mapped = EmbeddedBackend::map_event(event).unwrap();
        assert!(
            matches!(mapped, SnapcastEvent::ClientConnected { id, name, .. } if id == "c1" && name == "Kitchen")
        );
    }

    #[test]
    fn map_event_client_disconnected() {
        let event = ServerEvent::ClientDisconnected { id: "c1".into() };
        let mapped = EmbeddedBackend::map_event(event).unwrap();
        assert!(matches!(mapped, SnapcastEvent::ClientDisconnected { id } if id == "c1"));
    }

    #[test]
    fn map_event_volume_changed() {
        let event = ServerEvent::ClientVolumeChanged {
            client_id: "c1".into(),
            volume: 80,
            muted: false,
        };
        let mapped = EmbeddedBackend::map_event(event).unwrap();
        assert!(matches!(
            mapped,
            SnapcastEvent::ClientVolumeChanged {
                volume: 80,
                muted: false,
                ..
            }
        ));
    }

    #[test]
    fn map_event_group_events_become_server_updated() {
        let event = ServerEvent::GroupMuteChanged {
            group_id: "g1".into(),
            muted: true,
        };
        assert!(matches!(
            EmbeddedBackend::map_event(event),
            Some(SnapcastEvent::ServerUpdated)
        ));
    }

    #[test]
    fn map_event_group_name_changed() {
        let event = ServerEvent::GroupNameChanged {
            group_id: "g1".into(),
            name: "Living Room".into(),
        };
        assert!(matches!(
            EmbeddedBackend::map_event(event),
            Some(SnapcastEvent::ServerUpdated)
        ));
    }

    #[test]
    fn map_event_server_updated() {
        assert!(matches!(
            EmbeddedBackend::map_event(ServerEvent::ServerUpdated),
            Some(SnapcastEvent::ServerUpdated)
        ));
    }
}
