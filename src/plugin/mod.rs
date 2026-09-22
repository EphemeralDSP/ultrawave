pub mod editor;
mod sampler;

use crate::machines::memory::SampleMemory;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

pub use sampler::UltrawavePlugin;

/// Per-instance UI communication. No process-global LEDs or audio-thread locks.
#[derive(Default)]
pub struct UiState {
    pub record_trigger: AtomicBool,
    pub play_trigger: AtomicBool,
    pub rearm: AtomicBool,
    pub capture_locked: AtomicBool,
    pub recording: AtomicBool,
    pub playing: AtomicBool,
    pub capture_peak: AtomicU32,
    pub playback_peak: AtomicU32,
    /// One coherent snapshot: valid bit and absolute sample position.
    playback_cursor: AtomicU64,
}

impl UiState {
    pub fn publish_playback(&self, position: usize) {
        self.playback_cursor
            .store((1 << 63) | position as u64, Ordering::Relaxed);
    }

    pub fn playback_position(&self, displayed_len: usize) -> Option<f32> {
        let packed = self.playback_cursor.load(Ordering::Relaxed);
        (displayed_len != 0 && packed >> 63 != 0)
            .then(|| (packed as u32 as f32 / displayed_len as f32).clamp(0.0, 1.0))
    }

    pub fn clear_playback(&self) {
        self.playback_cursor.store(0, Ordering::Relaxed);
    }
}

pub fn maybe_preload_buffer_from_env(memory: &SampleMemory) -> Result<Option<usize>, String> {
    let wav_path = match std::env::var("ULTRAWAVE_SCREENSHOT_WAV") {
        Ok(path) if !path.is_empty() => path,
        _ => return Ok(None),
    };

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
    memory.replace_buffer_at_rate(mono_buffer, spec.sample_rate as f32);
    Ok(Some(len))
}
