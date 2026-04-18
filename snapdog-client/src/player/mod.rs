//! Audio output — cpal callback reads from Stream directly.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use snapcast_client::AudioFrame;
use snapcast_client::connection::now_usec;
use snapcast_client::stream::Stream;
use snapcast_client::time_provider::TimeProvider;
use tokio::sync::mpsc;

use crate::eq::ZoneEq;

/// Shared EQ processor, updated from the event loop, read from the audio thread.
pub type SharedEq = Arc<Mutex<ZoneEq>>;

/// Shared volume state, updated from the event loop, read from the audio thread.
pub struct VolumeState {
    pub percent: AtomicU8,
    pub muted: AtomicBool,
}

impl VolumeState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            percent: AtomicU8::new(100),
            muted: AtomicBool::new(false),
        })
    }

    pub fn gain(&self) -> f32 {
        if self.muted.load(Ordering::Relaxed) {
            return 0.0;
        }
        let pct = self.percent.load(Ordering::Relaxed) as f32 / 100.0;
        // Perceptual volume curve (quadratic)
        pct * pct
    }
}

/// Mixer implementation dispatched by mode.
pub enum Mixer {
    /// PCM amplitude scaling in the audio callback.
    Software(Arc<VolumeState>),
    /// ALSA hardware mixer control (Linux only).
    #[cfg(target_os = "linux")]
    Hardware {
        control: String,
        volume: Arc<VolumeState>,
    },
    /// MIDI CC volume control.
    Midi {
        conn: Mutex<midir::MidiOutputConnection>,
        channel: u8,
        cc: u8,
    },
    /// No volume control.
    None,
}

impl Mixer {
    pub fn from_cli(raw: &str, volume: Arc<VolumeState>) -> Self {
        let (mode, param) = raw.split_once(':').unwrap_or((raw, ""));
        match mode {
            "software" | "" => Mixer::Software(volume),
            #[cfg(target_os = "linux")]
            "hardware" => {
                let control = if param.is_empty() {
                    "Master".to_string()
                } else {
                    param.to_string()
                };
                Mixer::Hardware { control, volume }
            }
            #[cfg(not(target_os = "linux"))]
            "hardware" => {
                tracing::warn!(
                    "Hardware mixer not supported on this platform, falling back to software"
                );
                Mixer::Software(volume)
            }
            "midi" => match parse_midi_param(param) {
                Ok((conn, channel, cc)) => {
                    tracing::info!(channel = channel + 1, cc, "MIDI mixer connected");
                    Mixer::Midi {
                        conn: Mutex::new(conn),
                        channel,
                        cc,
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "MIDI mixer init failed, falling back to software");
                    Mixer::Software(volume)
                }
            },
            "none" => Mixer::None,
            other => {
                tracing::warn!(mode = other, "Unknown mixer mode, using software");
                Mixer::Software(volume)
            }
        }
    }

    /// Apply a volume change.
    pub fn set_volume(&self, percent: u8, muted: bool) {
        match self {
            Mixer::Software(vol) => {
                vol.percent.store(percent, Ordering::Relaxed);
                vol.muted.store(muted, Ordering::Relaxed);
            }
            #[cfg(target_os = "linux")]
            Mixer::Hardware {
                control, volume, ..
            } => {
                volume.percent.store(percent, Ordering::Relaxed);
                volume.muted.store(muted, Ordering::Relaxed);
                set_alsa_volume(control, percent, muted);
            }
            Mixer::Midi { conn, channel, cc } => {
                let value = if muted {
                    0
                } else {
                    (percent as u16 * 127 / 100).min(127) as u8
                };
                if let Ok(mut conn) = conn.lock() {
                    // CC message: 0xB0 | channel, cc, value
                    if let Err(e) = conn.send(&[0xB0 | channel, *cc, value]) {
                        tracing::warn!(error = %e, "MIDI send failed");
                    }
                }
            }
            Mixer::None => {}
        }
    }

    /// Get the software gain to apply in the audio callback.
    /// Returns 1.0 for hardware/midi/none (volume is handled elsewhere).
    pub fn software_gain(&self) -> f32 {
        match self {
            Mixer::Software(vol) => vol.gain(),
            _ => 1.0,
        }
    }
}

/// Parse `interface:ch[:cc]` → (MidiOutputConnection, channel_0based, cc)
fn parse_midi_param(param: &str) -> anyhow::Result<(midir::MidiOutputConnection, u8, u8)> {
    let parts: Vec<&str> = param.rsplitn(3, ':').collect();
    // rsplitn reverses: "IAC Driver:1:7" → ["7", "1", "IAC Driver"]
    let (interface, channel_1, cc) = match parts.len() {
        3 => (parts[2], parts[1], parts[0].parse::<u8>().unwrap_or(7)),
        2 => (parts[1], parts[0], 7u8),
        _ => anyhow::bail!("expected midi:interface:ch[:cc]"),
    };
    let channel: u8 = channel_1
        .parse::<u8>()
        .ok()
        .filter(|&c| (1..=16).contains(&c))
        .map(|c| c - 1)
        .ok_or_else(|| anyhow::anyhow!("MIDI channel must be 1-16, got '{channel_1}'"))?;

    let midi_out = midir::MidiOutput::new("snapdog-client")?;
    let port = midi_out
        .ports()
        .into_iter()
        .find(|p| {
            midi_out
                .port_name(p)
                .map(|n| n.to_lowercase().contains(&interface.to_lowercase()))
                .unwrap_or(false)
        })
        .ok_or_else(|| {
            let available: Vec<_> = midir::MidiOutput::new("probe")
                .ok()
                .map(|m| {
                    m.ports()
                        .iter()
                        .filter_map(|p| m.port_name(p).ok())
                        .collect()
                })
                .unwrap_or_default();
            anyhow::anyhow!("MIDI interface '{interface}' not found. Available: {available:?}")
        })?;
    let conn = midi_out.connect(&port, "snapdog-mixer")?;
    Ok((conn, channel, cc))
}

#[cfg(target_os = "linux")]
fn set_alsa_volume(control: &str, percent: u8, muted: bool) {
    use std::process::Command;
    // Use amixer for simplicity — avoids alsa-sys dependency
    let vol_arg = if muted {
        "0%".to_string()
    } else {
        format!("{percent}%")
    };
    match Command::new("amixer")
        .args(["sset", control, &vol_arg])
        .output()
    {
        Ok(output) if !output.status.success() => {
            tracing::warn!(
                control,
                "amixer failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Err(e) => tracing::warn!(control, error = %e, "Failed to run amixer"),
        _ => tracing::debug!(control, percent, muted, "Hardware volume set"),
    }
}

/// Start audio output. Waits for the Stream to have audio, then starts cpal.
pub async fn play_audio(
    rx: mpsc::Receiver<AudioFrame>,
    stream: Arc<Mutex<Stream>>,
    time_provider: Arc<Mutex<TimeProvider>>,
    eq: SharedEq,
    mixer: Arc<Mixer>,
) {
    // Drain audio_rx in background
    tokio::spawn(async move {
        let mut rx = rx;
        while rx.recv().await.is_some() {}
    });

    // Wait for the Stream to have a valid format
    let format = loop {
        {
            let s = stream.lock().unwrap_or_else(|e| e.into_inner());
            let f = s.format();
            if f.rate() > 0 && f.channels() > 0 {
                break f;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    };

    tracing::info!(
        rate = format.rate(),
        bits = format.bits(),
        channels = format.channels(),
        "Audio format detected"
    );

    std::thread::spawn(move || {
        if let Err(e) = run_cpal(stream, time_provider, format, eq, mixer) {
            tracing::error!(error = %e, "Audio output failed");
        }
    });
}

fn run_cpal(
    stream: Arc<Mutex<Stream>>,
    time_provider: Arc<Mutex<TimeProvider>>,
    format: snapcast_proto::SampleFormat,
    eq: SharedEq,
    mixer: Arc<Mixer>,
) -> anyhow::Result<()> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("no output device"))?;

    tracing::info!(device = %device.description().map(|d| format!("{d}")).unwrap_or_default(), "Using audio device");

    let config = cpal::StreamConfig {
        channels: format.channels(),
        sample_rate: format.rate(),
        buffer_size: cpal::BufferSize::Default,
    };

    let channels = format.channels() as usize;

    let cpal_stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
            let num_frames = data.len() / channels;

            let buffer_dac_usec = info
                .timestamp()
                .playback
                .duration_since(&info.timestamp().callback)
                .map(|d| d.as_micros() as i64)
                .unwrap_or(0)
                + (num_frames as i64 * 1_000_000) / format.rate() as i64;

            let server_now = {
                let tp = time_provider.lock().unwrap_or_else(|e| e.into_inner());
                now_usec() + tp.diff_to_server_usec()
            };

            let mut s = stream.lock().unwrap_or_else(|e| e.into_inner());
            let current_format = s.format();
            let current_frame_size = current_format.frame_size() as usize;
            let current_sample_size = current_format.sample_size() as usize;

            if current_frame_size == 0 {
                data.fill(0.0);
                return;
            }

            let mut pcm_buf = vec![0u8; num_frames * current_frame_size];
            s.get_player_chunk_or_silence(
                server_now,
                buffer_dac_usec,
                &mut pcm_buf,
                num_frames as u32,
            );
            drop(s);

            match current_sample_size {
                2 => {
                    for (i, chunk) in pcm_buf.chunks_exact(2).enumerate() {
                        if i < data.len() {
                            data[i] =
                                i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / i16::MAX as f32;
                        }
                    }
                }
                4 => {
                    for (i, chunk) in pcm_buf.chunks_exact(4).enumerate() {
                        if i < data.len() {
                            data[i] = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        }
                    }
                }
                _ => data.fill(0.0),
            }

            // Apply EQ after PCM decode
            if let Ok(mut eq) = eq.try_lock() {
                eq.process(data);
            }

            // Apply software volume (no-op for hardware/none mixer)
            let gain = mixer.software_gain();
            if gain < 1.0 {
                for sample in data.iter_mut() {
                    *sample *= gain;
                }
            }
        },
        |err| tracing::error!(error = %err, "Audio stream error"),
        None,
    )?;

    cpal_stream.play()?;
    tracing::info!("Audio playback started");

    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
