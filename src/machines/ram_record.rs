use crate::dsp::sample_engine::param_to_normalized;
use crate::machines::shared::{
    get_global_registry, BufferRegistry, CHANNEL_COUNT, MAX_BUFFER_SAMPLES,
};
use std::sync::Arc;

pub struct RamRecordParams {
    pub mlev: i32,
    pub mbal: i32,
    pub ilev: i32,
    pub ibal: i32,
    pub rec_len: i32,
    pub rec_rate: i32,
    pub tempo_bpm: f64,
}

impl Default for RamRecordParams {
    fn default() -> Self {
        Self {
            mlev: 64,
            mbal: 64,
            ilev: 64,
            ibal: 64,
            rec_len: 48,
            rec_rate: 127,
            tempo_bpm: 120.0,
        }
    }
}

impl RamRecordParams {
    /// Stereo-to-mono gains for the main and external buses. Center is exactly 50/50.
    pub fn input_gains(&self) -> [f32; 4] {
        let balance = |value: i32| {
            if value <= 64 {
                value as f32 / 128.0
            } else {
                0.5 + (value - 64) as f32 / 126.0
            }
        };
        let main = param_to_normalized(self.mlev);
        let input = param_to_normalized(self.ilev);
        let mb = balance(self.mbal);
        let ib = balance(self.ibal);
        [(1.0 - mb) * main, mb * main, (1.0 - ib) * input, ib * input]
    }
}

/// Convert a rec_len parameter value (0-127) to a beat count.
/// Returns `None` for INF mode (record until buffer full).
pub fn rec_len_to_beats(rec_len: i32) -> Option<f64> {
    match rec_len {
        0..=15 => Some(0.5),    // 1/8 note
        16..=31 => Some(1.0),   // 1/4 note
        32..=47 => Some(2.0),   // 1/2 note
        48..=63 => Some(4.0),   // 1 BAR
        64..=79 => Some(8.0),   // 2 BAR
        80..=95 => Some(16.0),  // 4 BAR
        96..=111 => Some(32.0), // 8 BAR
        _ => None,              // INF
    }
}

pub struct RamRecord {
    registry: Arc<BufferRegistry>,
    sample_rate: f32,
    max_length: usize,
    target_lengths: [usize; CHANNEL_COUNT],
    samples_recorded: [usize; CHANNEL_COUNT],
    elapsed_samples: [usize; CHANNEL_COUNT],
    is_recording: [bool; CHANNEL_COUNT],
    rec_rate_counters: [f32; CHANNEL_COUNT],
    rec_rate_divisors: [f32; CHANNEL_COUNT],
    monitor: [f32; CHANNEL_COUNT],
    owner_tokens: [u64; CHANNEL_COUNT],
}

impl RamRecord {
    const MAX_RECORD_SECONDS: f32 = 10.0;

    pub fn new(sample_rate: f32) -> Self {
        Self::with_registry(sample_rate, get_global_registry())
    }

    pub fn with_registry(sample_rate: f32, registry: Arc<BufferRegistry>) -> Self {
        let max_length = (sample_rate * Self::MAX_RECORD_SECONDS) as usize;

        Self {
            registry,
            sample_rate,
            max_length,
            target_lengths: [MAX_BUFFER_SAMPLES; CHANNEL_COUNT],
            samples_recorded: [0; CHANNEL_COUNT],
            elapsed_samples: [0; CHANNEL_COUNT],
            is_recording: [false; CHANNEL_COUNT],
            rec_rate_counters: [0.0; CHANNEL_COUNT],
            rec_rate_divisors: [1.0; CHANNEL_COUNT],
            monitor: [0.0; CHANNEL_COUNT],
            owner_tokens: [0; CHANNEL_COUNT],
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.max_length = (sample_rate * Self::MAX_RECORD_SECONDS) as usize;
        for length in &mut self.target_lengths {
            *length = (*length).min(self.max_length);
        }
    }

    /// Stop all writers before changing RAM ownership. The old Arc is returned
    /// so its final drop can be retired away from the audio thread.
    pub fn rebind_registry(&mut self, registry: Arc<BufferRegistry>) -> Arc<BufferRegistry> {
        self.stop_all_recording();
        std::mem::replace(&mut self.registry, registry)
    }

    fn clamp_channel(channel: usize) -> usize {
        channel.min(CHANNEL_COUNT - 1)
    }

    /// Starts capturing audio into the requested shared buffer channel.
    /// Retriggering while recording restarts from the beginning.
    pub fn start_recording(&mut self, params: &RamRecordParams, clear: bool, channel: usize) {
        let channel = Self::clamp_channel(channel);
        self.stop_recording(channel);

        let target_length = match rec_len_to_beats(params.rec_len) {
            Some(beats) => {
                let seconds = beats * (60.0 / params.tempo_bpm);
                (seconds * self.sample_rate as f64) as usize
            }
            None => self.max_length, // INF mode
        };
        let target_length = target_length.min(self.max_length).max(1);

        self.target_lengths[channel] = target_length;
        self.samples_recorded[channel] = 0;
        self.elapsed_samples[channel] = 0;

        let rate_normalized = param_to_normalized(params.rec_rate);
        self.rec_rate_divisors[channel] = 1.0 + (1.0 - rate_normalized) * 15.0;
        self.rec_rate_counters[channel] = 0.0;
        self.monitor[channel] = 0.0;

        let stored_rate = self.sample_rate / self.rec_rate_divisors[channel].ceil();
        // Registration happens after construction. Allocate the take's identity
        // here using only atomics, once a live session exists.
        self.owner_tokens[channel] = self.registry.writer_token();
        self.is_recording[channel] =
            self.registry
                .claim_writer(channel, self.owner_tokens[channel], clear, stored_rate);
    }

    pub fn stop_recording(&mut self, channel: usize) {
        let channel = Self::clamp_channel(channel);
        self.is_recording[channel] = false;
        self.registry
            .release_writer(channel, self.owner_tokens[channel]);
    }

    pub fn stop_all_recording(&mut self) {
        for channel in 0..CHANNEL_COUNT {
            self.stop_recording(channel);
        }
    }

    pub fn is_recording(&self, channel: usize) -> bool {
        let channel = Self::clamp_channel(channel);
        self.is_recording[channel]
    }

    pub fn is_any_recording(&self) -> bool {
        self.is_recording.iter().any(|&flag| flag)
    }

    pub fn record_sample(
        &mut self,
        main_l: f32,
        main_r: f32,
        input_l: f32,
        input_r: f32,
        params: &RamRecordParams,
        channel: usize,
    ) {
        let gains = params.input_gains();
        let mono = main_l * gains[0] + main_r * gains[1] + input_l * gains[2] + input_r * gains[3];
        self.record_mono(mono, channel);
    }

    /// Capture an already mixed/processed sample without repeating input mixing.
    pub fn record_mono(&mut self, mono: f32, channel: usize) {
        let channel = Self::clamp_channel(channel);

        if !self.is_recording[channel] {
            return;
        }

        self.rec_rate_counters[channel] += 1.0;
        if self.rec_rate_counters[channel] < self.rec_rate_divisors[channel] {
            self.elapsed_samples[channel] += 1;
            if self.elapsed_samples[channel] >= self.target_lengths[channel] {
                self.is_recording[channel] = false;
                self.registry
                    .release_writer(channel, self.owner_tokens[channel]);
            }
            return;
        }
        self.rec_rate_counters[channel] = 0.0;

        let sample_12bit = (mono * 2048.0).round().clamp(-2048.0, 2047.0) as i16;

        if !self
            .registry
            .write_sample_owned(channel, self.owner_tokens[channel], sample_12bit)
        {
            self.is_recording[channel] = false;
            self.registry
                .release_writer(channel, self.owner_tokens[channel]);
            return;
        }

        self.monitor[channel] = sample_12bit as f32 / 2048.0;
        self.samples_recorded[channel] += 1;
        self.elapsed_samples[channel] += 1;

        if self.elapsed_samples[channel] >= self.target_lengths[channel] {
            self.is_recording[channel] = false;
            self.registry
                .release_writer(channel, self.owner_tokens[channel]);
        }
    }

    pub fn monitor_sample(&self, channel: usize) -> f32 {
        self.monitor[Self::clamp_channel(channel)]
    }

    pub fn get_buffer(&self, channel: usize) -> Vec<i16> {
        self.registry.read_buffer(channel).unwrap_or_default()
    }

    pub fn buffer_len(&self, channel: usize) -> usize {
        self.registry.buffer_len(channel)
    }

    pub fn target_length(&self, channel: usize) -> usize {
        let channel = Self::clamp_channel(channel);
        self.target_lengths[channel]
    }

    pub fn recorded_samples(&self, channel: usize) -> usize {
        let channel = Self::clamp_channel(channel);
        self.samples_recorded[channel]
    }

    pub fn samples_remaining(&self, channel: usize) -> usize {
        let channel = Self::clamp_channel(channel);
        self.target_lengths[channel].saturating_sub(self.elapsed_samples[channel])
    }

    pub fn clear(&mut self, channel: usize) {
        let channel = Self::clamp_channel(channel);
        self.registry
            .release_writer(channel, self.owner_tokens[channel]);
        self.registry.clear_buffer(channel);
        self.samples_recorded[channel] = 0;
        self.elapsed_samples[channel] = 0;
        self.is_recording[channel] = false;
        self.rec_rate_counters[channel] = 0.0;
        self.rec_rate_divisors[channel] = 1.0;
    }

    pub fn clear_all(&mut self) {
        self.stop_all_recording();
        self.registry.clear_all();
        for channel in 0..CHANNEL_COUNT {
            self.samples_recorded[channel] = 0;
            self.elapsed_samples[channel] = 0;
            self.is_recording[channel] = false;
            self.rec_rate_counters[channel] = 0.0;
            self.rec_rate_divisors[channel] = 1.0;
        }
    }

    pub fn recording_progress(&self, channel: usize) -> f32 {
        let channel = Self::clamp_channel(channel);
        let target = self.target_lengths[channel];
        if target == 0 {
            return 0.0;
        }
        self.elapsed_samples[channel] as f32 / target as f32
    }
}

impl Drop for RamRecord {
    fn drop(&mut self) {
        self.stop_all_recording();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machines::shared::BufferRegistry;
    use std::sync::Arc;

    const SAMPLE_RATE: f32 = 100.0;

    fn basic_params() -> RamRecordParams {
        // rec_len 48 = 1 BAR = 4 beats. At 120 BPM that's 2 seconds = 200 samples @ 100Hz.
        RamRecordParams {
            rec_len: 48,
            rec_rate: 127,
            tempo_bpm: 120.0,
            ..Default::default()
        }
    }

    fn record_one_sample(rec: &mut RamRecord, params: &RamRecordParams, channel: usize) {
        rec.record_sample(0.5, 0.5, 0.5, 0.5, params, channel);
    }

    #[test]
    fn note_on_starts_recording_and_writes_to_registry() {
        let registry = Arc::new(BufferRegistry::new());
        let mut rec = RamRecord::with_registry(SAMPLE_RATE, registry.clone());
        let params = basic_params();

        rec.start_recording(&params, true, 0);
        assert!(rec.is_recording(0));

        record_one_sample(&mut rec, &params, 0);
        assert!(registry.buffer_len(0) > 0);
    }

    #[test]
    fn recording_stops_at_beat_synced_length() {
        let registry = Arc::new(BufferRegistry::new());
        let mut rec = RamRecord::with_registry(SAMPLE_RATE, registry.clone());
        // 1 BAR at 120 BPM = 4 beats * 0.5s/beat = 2s = 200 samples @ 100Hz
        let params = basic_params();

        rec.start_recording(&params, true, 0);
        let mut count = 0;
        while rec.is_recording(0) {
            record_one_sample(&mut rec, &params, 0);
            count += 1;
            assert!(count < 1000, "recording loop did not end");
        }

        // At 120 BPM, 1 BAR = 4 beats = 2 seconds = 200 samples @ 100Hz sample rate.
        // Recording stops after 200 elapsed samples (target_length = 200).
        assert_eq!(rec.target_length(0), 200);
        assert_eq!(registry.buffer_len(0), 200);
    }

    #[test]
    fn retrigger_restarts_recording() {
        let registry = Arc::new(BufferRegistry::new());
        let mut rec = RamRecord::with_registry(SAMPLE_RATE, registry.clone());
        let params = basic_params();

        rec.start_recording(&params, true, 2);
        // Record a few samples
        for _ in 0..5 {
            record_one_sample(&mut rec, &params, 2);
        }
        assert!(rec.is_recording(2));
        assert_eq!(registry.buffer_len(2), 5);

        // Retrigger should restart recording from scratch
        rec.start_recording(&params, true, 2);
        assert!(rec.is_recording(2));
        // Buffer was cleared by retrigger
        assert_eq!(registry.buffer_len(2), 0);
        assert_eq!(rec.samples_remaining(2), rec.target_length(2));
    }

    #[test]
    fn recording_length_is_locked_at_trigger_time() {
        let registry = Arc::new(BufferRegistry::new());
        let mut rec = RamRecord::with_registry(SAMPLE_RATE, registry.clone());
        let start_params = RamRecordParams {
            rec_len: 0, // 1/8 note = 0.5 beats
            tempo_bpm: 120.0,
            ..Default::default()
        };
        let changed_params = RamRecordParams {
            rec_len: 112, // INF
            tempo_bpm: 120.0,
            ..start_params
        };

        rec.start_recording(&start_params, true, 1);
        let locked_target = rec.target_length(1);
        // 0.5 beats at 120 BPM = 0.25s = 25 samples
        assert_eq!(locked_target, 25);

        let mut count = 0;
        while rec.is_recording(1) {
            rec.record_sample(0.5, 0.5, 0.5, 0.5, &changed_params, 1);
            count += 1;
            assert!(count < 500, "recording loop did not end");
        }

        // Should have stopped at the original target, not INF
        assert_eq!(registry.buffer_len(1), locked_target);
    }

    #[test]
    fn recording_writes_only_to_selected_channel() {
        let registry = Arc::new(BufferRegistry::new());
        let mut rec = RamRecord::with_registry(SAMPLE_RATE, registry.clone());
        let params = basic_params();

        rec.start_recording(&params, true, 2);
        record_one_sample(&mut rec, &params, 2);

        assert_eq!(registry.buffer_len(0), 0);
        assert_eq!(registry.buffer_len(1), 0);
        assert!(registry.buffer_len(2) > 0);
        assert_eq!(registry.buffer_len(3), 0);
    }

    #[test]
    fn beat_to_sample_at_various_tempos() {
        // 1 BAR = 4 beats
        // At 60 BPM: 4 beats * 1s/beat = 4s = 400 samples @ 100Hz
        let registry = Arc::new(BufferRegistry::new());
        let mut rec = RamRecord::with_registry(SAMPLE_RATE, registry);
        let params_60 = RamRecordParams {
            rec_len: 48, // 1 BAR
            tempo_bpm: 60.0,
            ..Default::default()
        };
        rec.start_recording(&params_60, true, 0);
        assert_eq!(rec.target_length(0), 400);

        // At 180 BPM: 4 beats * (60/180)s/beat = 4/3s ≈ 133 samples
        let params_180 = RamRecordParams {
            rec_len: 48,
            tempo_bpm: 180.0,
            ..Default::default()
        };
        rec.start_recording(&params_180, true, 1);
        assert_eq!(rec.target_length(1), 133);

        // 1/8 note at 120 BPM: 0.5 beats * 0.5s/beat = 0.25s = 25 samples
        let params_eighth = RamRecordParams {
            rec_len: 0,
            tempo_bpm: 120.0,
            ..Default::default()
        };
        rec.start_recording(&params_eighth, true, 2);
        assert_eq!(rec.target_length(2), 25);

        // 2 BAR at 120 BPM: 8 beats * 0.5s/beat = 4s = 400 samples
        let params_2bar = RamRecordParams {
            rec_len: 64,
            tempo_bpm: 120.0,
            ..Default::default()
        };
        rec.start_recording(&params_2bar, true, 3);
        assert_eq!(rec.target_length(3), 400);
    }

    #[test]
    fn inf_mode_records_until_buffer_full() {
        let registry = Arc::new(BufferRegistry::new());
        let mut rec = RamRecord::with_registry(SAMPLE_RATE, registry.clone());
        let params = RamRecordParams {
            rec_len: 127, // INF
            tempo_bpm: 120.0,
            ..Default::default()
        };

        rec.start_recording(&params, true, 0);
        let max_length = (SAMPLE_RATE * RamRecord::MAX_RECORD_SECONDS) as usize;
        assert_eq!(rec.target_length(0), max_length);
    }

    #[test]
    fn second_recorder_cannot_take_active_channel() {
        let registry = Arc::new(BufferRegistry::new());
        let mut rec = RamRecord::with_registry(SAMPLE_RATE, registry.clone());
        let mut contender = RamRecord::with_registry(SAMPLE_RATE, registry.clone());
        let params = basic_params();

        rec.start_recording(&params, true, 0);
        record_one_sample(&mut rec, &params, 0);
        assert!(rec.is_recording(0));
        contender.start_recording(&params, true, 0);
        assert!(!contender.is_recording(0));
        assert_eq!(registry.buffer_len(0), 1);
        record_one_sample(&mut rec, &params, 0);
        assert_eq!(registry.buffer_len(0), 2);
    }

    #[test]
    fn samples_remaining_counts_down_to_zero() {
        let registry = Arc::new(BufferRegistry::new());
        let mut rec = RamRecord::with_registry(SAMPLE_RATE, registry);
        let params = RamRecordParams {
            rec_len: 0, // 1/8 note = 0.5 beats at 120 BPM = 0.25s = 25 samples
            tempo_bpm: 120.0,
            ..Default::default()
        };

        rec.start_recording(&params, true, 0);
        let initial_remaining = rec.samples_remaining(0);
        assert_eq!(initial_remaining, rec.target_length(0));
        assert_eq!(initial_remaining, 25);

        while rec.samples_remaining(0) > 0 {
            rec.record_sample(0.5, 0.5, 0.5, 0.5, &params, 0);
        }

        assert_eq!(rec.samples_remaining(0), 0);
        assert!(!rec.is_recording(0));
    }

    #[test]
    fn rec_len_to_beats_mapping() {
        assert_eq!(rec_len_to_beats(0), Some(0.5));
        assert_eq!(rec_len_to_beats(15), Some(0.5));
        assert_eq!(rec_len_to_beats(16), Some(1.0));
        assert_eq!(rec_len_to_beats(31), Some(1.0));
        assert_eq!(rec_len_to_beats(32), Some(2.0));
        assert_eq!(rec_len_to_beats(48), Some(4.0));
        assert_eq!(rec_len_to_beats(64), Some(8.0));
        assert_eq!(rec_len_to_beats(80), Some(16.0));
        assert_eq!(rec_len_to_beats(96), Some(32.0));
        assert_eq!(rec_len_to_beats(112), None);
        assert_eq!(rec_len_to_beats(127), None);
    }
}
