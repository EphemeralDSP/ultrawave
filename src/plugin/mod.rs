pub mod editor;
pub mod play;
pub mod record;

use crate::machines::shared::{get_global_registry, CHANNEL_COUNT};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub use play::PlayPlugin;
pub use record::RecordPlugin;

/// Per-instance UI communication. No process-global LEDs or audio-thread locks.
#[derive(Default)]
pub struct UiState {
    pub trigger: AtomicBool,
    pub active: AtomicBool,
    pub peak: AtomicU32,
}

impl UiState {
    pub fn publish(&self, active: bool, peak: f32) {
        self.active.store(active, Ordering::Relaxed);
        // The UI consumes the maximum since its last poll, not one tiny block.
        self.peak.fetch_max(peak.abs().to_bits(), Ordering::Relaxed);
    }
}

pub fn maybe_preload_buffer_from_env() -> Result<Option<usize>, String> {
    let wav_path = match std::env::var("ULTRAWAVE_SCREENSHOT_WAV") {
        Ok(path) if !path.is_empty() => path,
        _ => return Ok(None),
    };

    let channel = std::env::var("ULTRAWAVE_SCREENSHOT_CHANNEL")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|channel| *channel < CHANNEL_COUNT)
        .unwrap_or(0);

    let path = PathBuf::from(wav_path);
    let mut reader = hound::WavReader::open(&path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;

    let raw_samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max = (1u32 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / max))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?
        }
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?,
    };

    let mono_buffer: Vec<i16> = raw_samples
        .chunks(channels)
        .map(|frame| {
            let left = frame.first().copied().unwrap_or(0.0);
            let right = frame.get(1).copied().unwrap_or(left);
            let mono = (left + right) * 0.5;
            (mono * 2048.0).round().clamp(-2048.0, 2047.0) as i16
        })
        .collect();

    let len = mono_buffer.len();
    get_global_registry().replace_buffer_at_rate(channel, mono_buffer, spec.sample_rate as f32);
    Ok(Some(len))
}
