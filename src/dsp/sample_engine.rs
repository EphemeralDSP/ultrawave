pub struct SampleEngine {
    buffer: Vec<i16>,
    position: f64,
    sample_rate: f32,
    srr_counter: f32,
    srr_hold_sample: f32,
}

impl SampleEngine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            buffer: Vec::new(),
            position: 0.0,
            sample_rate,
            srr_counter: 0.0,
            srr_hold_sample: 0.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn load_buffer(&mut self, samples: Vec<i16>) {
        self.buffer = samples;
        self.position = 0.0;
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.position = 0.0;
        self.srr_counter = 0.0;
        self.srr_hold_sample = 0.0;
    }

    pub fn trigger(&mut self, start_position: f64) {
        self.position = start_position.clamp(0.0, self.buffer.len() as f64);
        self.srr_counter = 0.0;
        self.srr_hold_sample = 0.0;
    }

    pub fn read_sample_linear(&mut self, pitch_ratio: f64) -> f32 {
        if self.buffer.is_empty() {
            return 0.0;
        }

        let len = self.buffer.len();
        let idx = self.position as usize;

        if idx >= len - 1 {
            return 0.0;
        }

        let frac = (self.position - idx as f64) as f32;
        let s0: f32 = self.buffer[idx] as f32 / 32768.0;
        let s1: f32 = self.buffer[idx + 1] as f32 / 32768.0;
        let interpolated = s0 + (s1 - s0) * frac;

        self.position += pitch_ratio;

        interpolated
    }

    pub fn read_sample_with_processing(&mut self, pitch_ratio: f64, srr: i32) -> f32 {
        let raw_sample = self.read_sample_linear(pitch_ratio);
        let quantized = apply_12bit_quantization(raw_sample);
        apply_sample_rate_reduction(quantized, srr, &mut self.srr_counter, &mut self.srr_hold_sample)
    }

    pub fn is_finished(&self, end_position: f64) -> bool {
        self.position >= end_position || self.position >= self.buffer.len() as f64
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn position(&self) -> f64 {
        self.position
    }

    pub fn buffer(&self) -> &[i16] {
        &self.buffer
    }
}

/// Applies 12-bit quantization as per Machinedrum UW specs.
/// Converts from 16-bit to 12-bit range, adding the characteristic grit.
#[inline]
pub fn apply_12bit_quantization(sample: f32) -> f32 {
    let scaled = sample * 2048.0;
    let quantized = scaled.round();
    let clamped = quantized.clamp(-2048.0, 2047.0);
    clamped / 2048.0
}

/// Applies sample rate reduction (SRR) as per Machinedrum UW specs.
/// SRR 0-127: 0 = no reduction, 127 = extreme lo-fi (down to ~2-bit equivalent)
///
/// This implements a sample-and-hold effect where higher SRR values
/// cause fewer sample updates, creating the lo-fi aliasing characteristic.
#[inline]
pub fn apply_sample_rate_reduction(
    sample: f32,
    srr: i32,
    counter: &mut f32,
    hold_sample: &mut f32,
) -> f32 {
    if srr == 0 {
        return sample;
    }

    let srr_normalized = srr as f32 / 127.0;
    let hold_period = 1.0 + srr_normalized * 63.0;

    if *counter == 0.0 {
        *hold_sample = sample;
    }

    *counter += 1.0;
    if *counter >= hold_period {
        *counter = 0.0;
    }

    *hold_sample
}

/// Converts a pitch parameter (0-127, center=64) to a playback ratio.
/// 64 = 1.0x (original speed), 0 = 0.5x, 127 = ~2.0x
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
        assert!((apply_12bit_quantization(0.0) - 0.0).abs() < 0.001);
        assert!((apply_12bit_quantization(1.0) - (2047.0 / 2048.0)).abs() < 0.001);
        assert!((apply_12bit_quantization(-1.0) - (-2048.0 / 2048.0)).abs() < 0.001);
    }

    #[test]
    fn test_12bit_quantization_reduces_precision() {
        let precise = 0.12345678;
        let quantized = apply_12bit_quantization(precise);
        let step_size = 1.0 / 2048.0;
        assert!((quantized - precise).abs() <= step_size);
    }

    #[test]
    fn test_srr_zero_passes_through() {
        let mut counter = 0.0;
        let mut hold = 0.0;
        let result = apply_sample_rate_reduction(0.5, 0, &mut counter, &mut hold);
        assert!((result - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_srr_holds_sample() {
        let mut counter = 0.0;
        let mut hold = 0.0;
        let _ = apply_sample_rate_reduction(1.0, 127, &mut counter, &mut hold);
        let result = apply_sample_rate_reduction(0.5, 127, &mut counter, &mut hold);
        assert!((result - 1.0).abs() < 0.001);
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
        let mut engine = SampleEngine::new(44100.0);
        engine.load_buffer(vec![0, 16384, 32767]);
        engine.trigger(0.0);

        let s1 = engine.read_sample_linear(0.5);
        assert!(s1 >= 0.0 && s1 < 0.3);
    }
}
