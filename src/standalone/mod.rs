pub mod play;
pub mod play_editor;
pub mod record;
pub mod record_editor;

use crate::machines::shared::{get_global_registry, CHANNEL_COUNT};
use std::path::PathBuf;

pub use play::StandalonePlay;
pub use record::StandaloneRecord;

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

    get_global_registry().replace_buffer(channel, mono_buffer.clone());
    Ok(Some(mono_buffer.len()))
}
