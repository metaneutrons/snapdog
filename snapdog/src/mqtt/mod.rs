// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! MQTT integration via rumqttc.
//!
//! Bidirectional: subscribes to command topics (`*/set`), publishes status updates.
//! All incoming commands are routed through ZonePlayer command channels.

use std::collections::HashMap;

use anyhow::{Context, Result};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};

use crate::config::MqttConfig;
use crate::player::{ZoneCommand, ZoneCommandSender};
use crate::state;

/// MQTT bridge: receives commands, publishes status.
pub struct MqttBridge {
    client: AsyncClient,
    eventloop: rumqttc::EventLoop,
    base_topic: String,
}

impl MqttBridge {
    /// Connect to MQTT broker.
    #[tracing::instrument(skip_all, fields(broker = %config.broker))]
    pub async fn connect(config: &MqttConfig) -> Result<Self> {
        let mut opts = MqttOptions::new("snapdog", &config.broker, parse_port(&config.broker)?);
        opts.set_keep_alive(std::time::Duration::from_secs(60));
        if !config.username.is_empty() {
            opts.set_credentials(&config.username, &config.password);
        }

        let (client, eventloop) = AsyncClient::new(opts, 64);
        tracing::info!("MQTT connected");

        Ok(Self {
            client,
            eventloop,
            base_topic: config.base_topic.trim_end_matches('/').to_string(),
        })
    }

    /// Subscribe to all command topics.
    pub async fn subscribe_commands(&self) -> Result<()> {
        let topics = [
            "zones/+/volume/set",
            "zones/+/mute/set",
            "zones/+/control/set",
            "zones/+/track/set",
            "zones/+/track/position/set",
            "zones/+/playlist/set",
            "clients/+/volume/set",
            "clients/+/mute/set",
            "clients/+/latency/set",
            "clients/+/zone/set",
        ];

        for topic in &topics {
            self.client
                .subscribe(format!("{}/{topic}", self.base_topic), QoS::AtLeastOnce)
                .await
                .with_context(|| format!("Failed to subscribe to {topic}"))?;
        }

        tracing::info!(count = topics.len(), "Subscribed to command topics");
        Ok(())
    }

    /// Publish a status value (retained).
    pub async fn publish(&self, topic: &str, payload: &str) -> Result<()> {
        self.client
            .publish(
                format!("{}/{topic}", self.base_topic),
                QoS::AtLeastOnce,
                true,
                payload.as_bytes(),
            )
            .await
            .with_context(|| format!("Failed to publish to {topic}"))
    }

    /// Publish zone status updates.
    pub async fn publish_zone_state(&self, index: usize, zone: &state::ZoneState) -> Result<()> {
        let base = format!("zones/{index}");
        self.publish(&format!("{base}/volume"), &zone.volume.to_string())
            .await?;
        self.publish(&format!("{base}/mute"), &zone.muted.to_string())
            .await?;
        self.publish(&format!("{base}/shuffle"), &zone.shuffle.to_string())
            .await?;
        self.publish(&format!("{base}/repeat"), &zone.repeat.to_string())
            .await?;
        if let Some(track) = &zone.track {
            self.publish(&format!("{base}/track/title"), &track.title)
                .await?;
            self.publish(&format!("{base}/track/artist"), &track.artist)
                .await?;
            self.publish(&format!("{base}/track/album"), &track.album)
                .await?;
            self.publish(
                &format!("{base}/track/duration"),
                &track.duration_ms.to_string(),
            )
            .await?;
            self.publish(
                &format!("{base}/track/position"),
                &track.position_ms.to_string(),
            )
            .await?;
            self.publish(
                &format!("{base}/track/cover"),
                &format!("/api/v1/zones/{index}/cover"),
            )
            .await?;
        }
        Ok(())
    }

    /// Publish client status updates.
    pub async fn publish_client_state(
        &self,
        index: usize,
        client: &state::ClientState,
    ) -> Result<()> {
        let base = format!("clients/{index}");
        self.publish(&format!("{base}/volume"), &client.volume.to_string())
            .await?;
        self.publish(&format!("{base}/mute"), &client.muted.to_string())
            .await?;
        self.publish(&format!("{base}/latency"), &client.latency_ms.to_string())
            .await?;
        self.publish(&format!("{base}/zone"), &client.zone_index.to_string())
            .await?;
        self.publish(&format!("{base}/connected"), &client.connected.to_string())
            .await?;
        Ok(())
    }

    /// Run the event loop, dispatching incoming commands via ZonePlayer channels.
    pub async fn run(
        &mut self,
        zone_commands: HashMap<usize, ZoneCommandSender>,
        state: state::SharedState,
    ) -> Result<()> {
        loop {
            self.poll_once(&zone_commands, &state).await;
        }
    }

    /// Poll for a single MQTT event. Returns when one event is processed.
    pub async fn poll_once(
        &mut self,
        zone_commands: &HashMap<usize, ZoneCommandSender>,
        state: &state::SharedState,
    ) {
        match self.eventloop.poll().await {
            Ok(Event::Incoming(Packet::Publish(msg))) => {
                let topic = msg.topic.clone();
                let payload = String::from_utf8_lossy(&msg.payload).to_string();
                tracing::debug!(topic = %topic, payload = %payload, "MQTT message received");
                if let Err(e) = self
                    .handle_command(&topic, &payload, zone_commands, state)
                    .await
                {
                    tracing::warn!(error = %e, topic = %topic, "Failed to handle MQTT command");
                }
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "MQTT connection error, retrying");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }

    async fn handle_command(
        &self,
        topic: &str,
        payload: &str,
        zone_commands: &HashMap<usize, ZoneCommandSender>,
        state: &state::SharedState,
    ) -> Result<()> {
        let stripped = topic
            .strip_prefix(&self.base_topic)
            .and_then(|t| t.strip_prefix('/'))
            .context("Topic doesn't match base")?;

        let parts: Vec<&str> = stripped.split('/').collect();

        match parts.as_slice() {
            // Zone commands → routed through ZonePlayer
            ["zones", idx, "volume", "set"] => {
                let index: usize = idx.parse()?;
                let volume: i32 = payload.parse()?;
                send_zone_cmd(zone_commands, index, ZoneCommand::SetVolume(volume)).await;
            }
            ["zones", idx, "mute", "set"] => {
                let index: usize = idx.parse()?;
                let muted: bool = payload.parse()?;
                send_zone_cmd(zone_commands, index, ZoneCommand::SetMute(muted)).await;
            }
            ["zones", idx, "control", "set"] => {
                let index: usize = idx.parse()?;
                let cmd = match payload.to_lowercase().as_str() {
                    "play" => Some(ZoneCommand::Play),
                    "pause" => Some(ZoneCommand::Pause),
                    "stop" => Some(ZoneCommand::Stop),
                    "next" => Some(ZoneCommand::Next),
                    "previous" => Some(ZoneCommand::Previous),
                    _ => {
                        tracing::debug!(zone = index, command = payload, "Unknown control command");
                        None
                    }
                };
                if let Some(cmd) = cmd {
                    send_zone_cmd(zone_commands, index, cmd).await;
                }
            }
            ["zones", idx, "playlist", "set"] => {
                let index: usize = idx.parse()?;
                let playlist: usize = payload.parse()?;
                send_zone_cmd(zone_commands, index, ZoneCommand::SetPlaylist(playlist)).await;
            }
            ["zones", idx, "track", "set"] => {
                let index: usize = idx.parse()?;
                let track: usize = payload.parse()?;
                send_zone_cmd(zone_commands, index, ZoneCommand::SetTrack(track)).await;
            }
            ["zones", idx, "track", "position", "set"] => {
                let index: usize = idx.parse()?;
                let pos: i64 = payload.parse()?;
                send_zone_cmd(zone_commands, index, ZoneCommand::Seek(pos)).await;
            }

            // Client commands → direct state mutation (no ZonePlayer involvement)
            ["clients", idx, "volume", "set"] => {
                let index: usize = idx.parse()?;
                let volume: i32 = payload.parse()?;
                let mut store = state.write().await;
                if let Some(client) = store.clients.get_mut(&index) {
                    client.volume = volume.clamp(0, 100);
                }
            }
            ["clients", idx, "mute", "set"] => {
                let index: usize = idx.parse()?;
                let muted: bool = payload.parse()?;
                let mut store = state.write().await;
                if let Some(client) = store.clients.get_mut(&index) {
                    client.muted = muted;
                }
            }
            ["clients", idx, "latency", "set"] => {
                let index: usize = idx.parse()?;
                let latency: i32 = payload.parse()?;
                let mut store = state.write().await;
                if let Some(client) = store.clients.get_mut(&index) {
                    client.latency_ms = latency;
                }
            }
            ["clients", idx, "zone", "set"] => {
                let index: usize = idx.parse()?;
                let zone: usize = payload.parse()?;
                let mut store = state.write().await;
                if let Some(client) = store.clients.get_mut(&index) {
                    client.zone_index = zone;
                }
            }
            _ => {
                tracing::debug!(topic = stripped, "Unhandled MQTT command topic");
            }
        }

        Ok(())
    }
}

async fn send_zone_cmd(
    zone_commands: &HashMap<usize, ZoneCommandSender>,
    index: usize,
    cmd: ZoneCommand,
) {
    if let Some(tx) = zone_commands.get(&index) {
        if let Err(e) = tx.send(cmd).await {
            tracing::warn!(zone = index, error = %e, "Failed to send zone command from MQTT");
        }
    } else {
        tracing::warn!(zone = index, "No ZonePlayer for MQTT command");
    }
}

fn parse_port(broker: &str) -> Result<u16> {
    broker
        .rsplit_once(':')
        .and_then(|(_, p)| p.parse().ok())
        .context("Invalid broker address — expected host:port")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_broker_port() {
        assert_eq!(parse_port("mqtt:1883").unwrap(), 1883);
        assert_eq!(parse_port("localhost:1883").unwrap(), 1883);
        assert!(parse_port("no-port").is_err());
    }
}
