//! Audio output — cpal callback reads from Stream directly.

use std::sync::{Arc, Mutex};

use snapcast_client::AudioFrame;
use snapcast_client::connection::now_usec;
use snapcast_client::stream::Stream;
use snapcast_client::time_provider::TimeProvider;
use tokio::sync::mpsc;

use crate::eq::ZoneEq;

/// Shared EQ processor, updated from the event loop, read from the audio thread.
pub type SharedEq = Arc<Mutex<ZoneEq>>;

/// Start audio output. Waits for the Stream to have audio, then starts cpal.
pub async fn play_audio(
    rx: mpsc::Receiver<AudioFrame>,
    stream: Arc<Mutex<Stream>>,
    time_provider: Arc<Mutex<TimeProvider>>,
    eq: SharedEq,
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
        if let Err(e) = run_cpal(stream, time_provider, format, eq) {
            tracing::error!(error = %e, "Audio output failed");
        }
    });
}

fn run_cpal(
    stream: Arc<Mutex<Stream>>,
    time_provider: Arc<Mutex<TimeProvider>>,
    format: snapcast_proto::SampleFormat,
    eq: SharedEq,
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
