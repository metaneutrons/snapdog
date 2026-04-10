// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Embedded Snapcast backend — in-process server via snapcast-server crate.

use anyhow::{Context, Result};
use tokio::sync::mpsc;

use snapcast_server::{AudioFrame, ServerCommand, ServerConfig, ServerEvent, SnapServer};

use super::backend::{BoxFuture, SnapcastBackend, SnapcastEvent};
use crate::config::AppConfig;
use crate::player::{ClientAction, GroupAction, SnapcastCmd};

/// Embedded Snapcast server backend.
pub struct EmbeddedBackend {
    cmd_tx: mpsc::Sender<ServerCommand>,
    audio_tx: mpsc::Sender<AudioFrame>,
}

/// Event receiver from the embedded server.
pub struct EmbeddedEventReceiver {
    event_rx: mpsc::Receiver<ServerEvent>,
}

impl EmbeddedBackend {
    /// Start the embedded server. Returns the backend + event receiver.
    pub async fn start(config: &AppConfig) -> Result<(Self, EmbeddedEventReceiver)> {
        let server_config = ServerConfig {
            stream_port: config.snapcast.streaming_port,
            buffer_ms: 1000,
            codec: config.audio.codec.clone(),
            sample_format: format!(
                "{}:{}:{}",
                config.audio.sample_rate, config.audio.bit_depth, config.audio.channels
            ),
            ..ServerConfig::default()
        };

        let (mut server, event_rx, audio_tx) = SnapServer::new(server_config);
        let cmd_tx = server.command_sender();

        tokio::spawn(async move {
            if let Err(e) = server.run().await {
                tracing::error!(error = %e, "Embedded Snapcast server error");
            }
        });

        tracing::info!(
            port = config.snapcast.streaming_port,
            "Embedded Snapcast server started"
        );

        Ok((
            Self { cmd_tx, audio_tx },
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
            SnapcastCmd::ReconcileZones => vec![], // handled at higher level
        }
    }

    /// Map a `ServerEvent` to a `SnapcastEvent`.
    fn map_event(event: ServerEvent) -> Option<SnapcastEvent> {
        match event {
            ServerEvent::ClientConnected { id, name } => Some(SnapcastEvent::ClientConnected {
                id,
                name: name.clone(),
                mac: String::new(), // embedded server doesn't expose MAC yet
            }),
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
            | ServerEvent::StreamStatus { .. } => Some(SnapcastEvent::ServerUpdated),
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
        _zone_index: usize,
        samples: &[f32],
        sample_rate: u32,
        channels: u16,
    ) -> BoxFuture<'_, Result<()>> {
        let frame = AudioFrame {
            samples: samples.to_vec(),
            sample_rate,
            channels,
            timestamp_usec: snapcast_server::time::now_usec(),
        };
        Box::pin(async move {
            self.audio_tx
                .send(frame)
                .await
                .map_err(|_| anyhow::anyhow!("Audio channel closed"))
        })
    }

    fn execute(&self, cmd: SnapcastCmd) -> BoxFuture<'_, Result<()>> {
        let commands = Self::map_command(cmd);
        Box::pin(async move {
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
            rx.await.context("Status response channel closed")
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
