use crate::machines::memory::SampleMemory;
use crate::Decimator;
use std::sync::Arc;

enum SampleSource {
    Empty,
    Owned(Arc<[i16]>),
    Buffer {
        memory: Arc<SampleMemory>,
        revision: u64,
        len: usize,
    },
}

pub(crate) trait SampleRead {
    fn extent(&self) -> usize;
    fn pair(&self, index: usize) -> Option<(i16, i16)>;
}

impl SampleRead for SampleSource {
    fn extent(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Owned(buffer) => buffer.len(),
            Self::Buffer { len, .. } => *len,
        }
    }

    fn pair(&self, index: usize) -> Option<(i16, i16)> {
        match self {
            Self::Empty => None,
            Self::Owned(buffer) => Some((buffer[index], buffer[(index + 1).min(buffer.len() - 1)])),
            Self::Buffer {
                memory,
                revision,
                len,
            } => memory.read_sample_pair(index, *revision, *len),
        }
    }
}

pub struct SampleEngine {
    source: SampleSource,
    position: f64,
    decimator: Decimator,
}

impl SampleEngine {
    pub fn new() -> Self {
        Self {
            source: SampleSource::Empty,
            position: 0.0,
            decimator: Decimator::new(),
        }
    }

    pub fn load_buffer(&mut self, samples: Vec<i16>) {
        self.load_shared_buffer(samples.into());
    }

    /// Bind immutable signed 12-bit codes without copying them per voice.
    ///
    /// Unlike recorded RAM, this source has no ten-second capacity limit. For
    /// real-time rebinding, retain the old asset elsewhere so releasing this
    /// voice's reference cannot free its allocation on the audio thread.
    pub fn load_shared_buffer(&mut self, samples: Arc<[i16]>) {
        self.source = SampleSource::Owned(samples);
        self.position = 0.0;
    }

    pub fn latch_buffer(&mut self, memory: Arc<SampleMemory>) -> Option<f32> {
        let (len, rate, revision) = memory.snapshot()?;
        self.source = SampleSource::Buffer {
            memory,
            revision,
            len,
        };
        self.position = 0.0;
        (len != 0).then_some(rate)
    }

    /// Also usable between retriggers, when no samples are being read.
    pub fn source_is_current(&self) -> bool {
        match &self.source {
            SampleSource::Empty | SampleSource::Owned(_) => true,
            SampleSource::Buffer {
                memory, revision, ..
            } => memory
                .snapshot()
                .is_some_and(|(_, _, current)| current == *revision),
        }
    }

    pub fn trigger(&mut self, start_position: f64) {
        self.trigger_at(start_position.clamp(0.0, self.buffer_len() as f64));
    }

    pub(crate) fn trigger_at(&mut self, position: f64) {
        self.position = position;
        self.decimator = Decimator::new();
    }

    /// Position is a boundary in 0..=len. A negative ratio reads just below that
    /// boundary, then steps backwards. At unity rate, swapping integral region
    /// boundaries reverses its samples exactly, without skipping either endpoint.
    pub fn read_sample_linear(&mut self, pitch_ratio: f64) -> f32 {
        read_linear(
            &mut self.position,
            &mut self.decimator,
            &self.source,
            pitch_ratio,
        )
    }

    pub(crate) fn read_from(&mut self, source: &impl SampleRead, pitch_ratio: f64) -> f32 {
        read_linear(&mut self.position, &mut self.decimator, source, pitch_ratio)
    }

    pub(crate) fn position(&self) -> f64 {
        self.position
    }

    pub fn read_sample_with_processing(&mut self, pitch_ratio: f64, srr: f32, brr: i32) -> f32 {
        let raw_sample = self.read_sample_linear(pitch_ratio);
        let quantized = apply_bit_reduction(raw_sample, brr);
        self.decimator.process(quantized, srr)
    }

    pub fn is_finished(&self, end_position: f64, pitch_ratio: f64) -> bool {
        if pitch_ratio < 0.0 {
            self.position <= end_position || self.position <= 0.0
        } else {
            self.position >= end_position || self.position >= self.buffer_len() as f64
        }
    }

    pub fn buffer_len(&self) -> usize {
        self.source.extent()
    }

    pub fn playback_position(&self) -> usize {
        self.position.clamp(0.0, self.buffer_len() as f64) as usize
    }
}

impl Default for SampleEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn read_linear(
    position: &mut f64,
    decimator: &mut Decimator,
    source: &impl SampleRead,
    ratio: f64,
) -> f32 {
    let len = source.extent();
    let reverse = ratio < 0.0;
    if (reverse && (*position <= 0.0 || *position > len as f64))
        || (!reverse && *position >= len as f64)
    {
        return 0.0;
    }
    let read_position = if reverse {
        (*position - 1.0).max(0.0)
    } else {
        *position
    };
    let index = read_position as usize;
    let fraction = (read_position - index as f64) as f32;
    let Some((a, b)) = source.pair(index) else {
        *position = if reverse { 0.0 } else { len as f64 };
        // Invalidated staged takes must not leak SRR's held sample.
        *decimator = Decimator::new();
        return 0.0;
    };
    let a = a as f32 / 2048.0;
    let b = b as f32 / 2048.0;
    *position += ratio;
    a + (b - a) * fraction
}

/// Storage boundary shared by staged and progressive recording.
pub fn quantize_12bit(sample: f32) -> i16 {
    (sample * 2048.0).round().clamp(-2048.0, 2047.0) as i16
}

/// BRR progressively removes up to ten bits from the 12-bit playback signal.
/// This deliberately stepped mapping is an approximation, not an MD ROM table.
pub fn playback_bits(brr: i32) -> i32 {
    12 - brr.clamp(0, 127) * 10 / 127
}

#[inline]
pub fn apply_bit_reduction(sample: f32, brr: i32) -> f32 {
    let bits = playback_bits(brr);
    let scale = (1 << (bits - 1)) as f32;
    (sample * scale).round().clamp(-scale, scale - 1.0) / scale
}

/// Converts a pitch parameter (0-127, center=64) to a playback ratio.
/// 64 = 1.0x (original speed), 52 = 0.5x, 76 = 2.0x.
pub fn pitch_to_ratio(pitch: i32) -> f64 {
    let semitones = (pitch - 64) as f64;
    2.0_f64.powf(semitones / 12.0)
}

/// Converts a 0-127 parameter to normalized 0.0-1.0 range.
#[inline]
pub fn param_to_normalized(value: i32) -> f32 {
    (value as f32) / 127.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_12bit_quantization_clamps_properly() {
        assert!((apply_bit_reduction(0.0, 0) - 0.0).abs() < 0.001);
        assert!((apply_bit_reduction(1.0, 0) - (2047.0 / 2048.0)).abs() < 0.001);
        assert!((apply_bit_reduction(-1.0, 0) - (-2048.0 / 2048.0)).abs() < 0.001);
    }

    #[test]
    fn test_12bit_quantization_reduces_precision() {
        let precise = 0.12345678;
        let quantized = apply_bit_reduction(precise, 0);
        let step_size = 1.0 / 2048.0;
        assert!((quantized - precise).abs() <= step_size);
    }

    #[test]
    fn test_pitch_to_ratio_center() {
        let ratio = pitch_to_ratio(64);
        assert!((ratio - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_pitch_to_ratio_octave_up() {
        let ratio = pitch_to_ratio(64 + 12);
        assert!((ratio - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_pitch_to_ratio_octave_down() {
        let ratio = pitch_to_ratio(64 - 12);
        assert!((ratio - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_linear_interpolation() {
        let mut engine = SampleEngine::new();
        engine.load_buffer(vec![-512, 1024, 1536]);
        engine.trigger(0.0);

        assert_eq!(engine.read_sample_linear(0.5), -0.25);
        assert_eq!(engine.read_sample_linear(0.5), 0.125);
        assert_eq!(engine.read_sample_linear(0.5), 0.5);
    }
}
