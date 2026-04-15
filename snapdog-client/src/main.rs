mod cli;
mod eq;
mod logging;
mod player;

use clap::Parser;
use snapcast_client::{ClientCommand, ClientConfig, ClientEvent, SnapClient};

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();

    logging::init(&cli.logsink, &cli.logfilter)?;

    if cli.list {
        list_devices(&cli.player);
        return Ok(());
    }

    #[cfg(feature = "encryption")]
    let encryption_psk = cli.encryption_psk.clone();
    let settings = cli.into_settings()?;

    #[cfg(unix)]
    if let Some(ref daemon) = settings.daemon {
        daemonize(daemon)?;
    }

    tracing::info!(
        server = %format!(
            "{}://{}:{}",
            settings.server.scheme, settings.server.host, settings.server.port
        ),
        instance = settings.instance,
        "snapdog-client starting"
    );

    let config = ClientConfig {
        scheme: settings.server.scheme.clone(),
        host: settings.server.host.clone(),
        port: settings.server.port,
        auth: settings.server.auth.clone(),
        #[cfg(feature = "tls")]
        server_certificate: settings.server.server_certificate.clone(),
        #[cfg(feature = "tls")]
        certificate: settings.server.certificate.clone(),
        #[cfg(feature = "tls")]
        certificate_key: settings.server.certificate_key.clone(),
        #[cfg(feature = "tls")]
        key_password: settings.server.key_password.clone(),
        #[cfg(feature = "encryption")]
        encryption_psk: Some(
            encryption_psk.unwrap_or_else(|| snapcast_proto::DEFAULT_ENCRYPTION_PSK.into()),
        ),
        instance: settings.instance,
        host_id: settings.host_id.clone(),
        latency: settings.player.latency,
        client_name: "SnapDog".into(),
        ..ClientConfig::default()
    };
    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async {
        let (mut client, mut events, audio_rx) = SnapClient::new(config);
        let cmd = client.command_sender();

        // EQ processor — shared between event loop and audio thread
        let eq = std::sync::Arc::new(std::sync::Mutex::new(eq::ZoneEq::new(48000, 2)));

        // Audio output: cpal callback reads from Stream directly
        let player_stream = std::sync::Arc::clone(&client.stream);
        let player_tp = std::sync::Arc::clone(&client.time_provider);
        let player_eq = eq.clone();
        tokio::spawn(async move {
            player::play_audio(audio_rx, player_stream, player_tp, player_eq).await;
        });

        // Event handler
        let event_eq = eq.clone();
        tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                match event {
                    ClientEvent::Connected { host, port } => {
                        tracing::info!(host, port, "Connected");
                    }
                    ClientEvent::Disconnected { .. } => {}
                    ClientEvent::VolumeChanged { volume, muted } => {
                        tracing::info!(volume, muted, "Volume changed");
                    }
                    ClientEvent::TimeSyncComplete { diff_ms } => {
                        tracing::info!(diff_ms, "Time sync complete");
                    }
                    ClientEvent::StreamStarted { codec, format } => {
                        tracing::info!(%codec, %format, "Stream started");
                    }
                    #[cfg(feature = "custom-protocol")]
                    ClientEvent::CustomMessage(msg) if msg.type_id == eq::TYPE_EQ_CONFIG => {
                        match serde_json::from_slice::<eq::EqConfig>(&msg.payload) {
                            Ok(config) => {
                                tracing::info!(
                                    enabled = config.enabled,
                                    bands = config.bands.len(),
                                    "EQ config received"
                                );
                                event_eq
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .set_config(&config);
                            }
                            Err(e) => tracing::warn!(error = %e, "Invalid EQ config payload"),
                        }
                    }
                    _ => {}
                }
            }
        });

        // Ctrl-C
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            tracing::info!("Received Ctrl-C, shutting down");
            cmd.send(ClientCommand::Stop).await.ok();
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(2));
                std::process::exit(0);
            });
        });

        client.run().await
    })?;

    tracing::info!("snapdog-client terminated");
    Ok(())
}

fn list_devices(player: &str) {
    let player_name = player.split(':').next().unwrap_or("");
    match player_name {
        #[cfg(target_os = "macos")]
        "coreaudio" | "" => {
            println!("0: Default Output\nCoreAudio default output device\n");
        }
        _ => println!("No device listing available for '{player_name}'"),
    }
}

#[cfg(unix)]
fn daemonize(daemon: &snapcast_client::config::DaemonSettings) -> anyhow::Result<()> {
    if let Some(priority) = daemon.priority {
        let priority = priority.clamp(-20, 19);
        unsafe {
            libc::setpriority(libc::PRIO_PROCESS, 0, priority);
        }
        tracing::info!(priority, "Process priority set");
    }

    if let Some(ref user) = daemon.user {
        tracing::info!(user, "Would drop privileges to user (not yet implemented)");
    }

    unsafe {
        let pid = libc::fork();
        if pid < 0 {
            anyhow::bail!("fork failed");
        }
        if pid > 0 {
            std::process::exit(0);
        }
        libc::setsid();
    }

    tracing::info!("Daemonized");
    Ok(())
}
