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
}

impl Default for RamRecordParams {
    fn default() -> Self {
        Self {
            mlev: 64,
            mbal: 64,
            ilev: 64,
            ibal: 64,
            rec_len: 64,
            rec_rate: 127,
        }
    }
}

pub struct RamRecord {
    registry: Arc<BufferRegistry>,
    sample_rate: f32,
    max_length: usize,
    target_lengths: [usize; CHANNEL_COUNT],
    samples_recorded: [usize; CHANNEL_COUNT],
    is_recording: [bool; CHANNEL_COUNT],
    rec_rate_counters: [f32; CHANNEL_COUNT],
    rec_rate_divisors: [f32; CHANNEL_COUNT],
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
            is_recording: [false; CHANNEL_COUNT],
            rec_rate_counters: [0.0; CHANNEL_COUNT],
            rec_rate_divisors: [1.0; CHANNEL_COUNT],
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.max_length = (sample_rate * Self::MAX_RECORD_SECONDS) as usize;
        for length in &mut self.target_lengths {
            *length = (*length).min(self.max_length);
        }
    }

    fn clamp_channel(channel: usize) -> usize {
        channel.min(CHANNEL_COUNT - 1)
    }

    /// Starts capturing audio into the requested shared buffer channel.
    /// Repeated triggers while the channel is already recording are ignored to keep the behaviour deterministic.
    pub fn start_recording(&mut self, params: &RamRecordParams, clear: bool, channel: usize) {
        let channel = Self::clamp_channel(channel);

        if self.is_recording[channel] {
            return;
        }

        if clear {
            self.registry.clear_buffer(channel);
        } else {
            self.registry.reset_write_position(channel);
        }

        let len_normalized = param_to_normalized(params.rec_len);
        let target_length = ((len_normalized * Self::MAX_RECORD_SECONDS * self.sample_rate)
            as usize)
            .min(self.max_length)
            .max(1);

        self.target_lengths[channel] = target_length;
        self.samples_recorded[channel] = 0;

        let rate_normalized = param_to_normalized(params.rec_rate);
        self.rec_rate_divisors[channel] = 1.0 + (1.0 - rate_normalized) * 15.0;
        self.rec_rate_counters[channel] = 0.0;

        self.is_recording[channel] = true;
    }

    pub fn stop_recording(&mut self, channel: usize) {
        let channel = Self::clamp_channel(channel);
        self.is_recording[channel] = false;
    }

    pub fn stop_all_recording(&mut self) {
        for channel in 0..CHANNEL_COUNT {
            self.is_recording[channel] = false;
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
        let channel = Self::clamp_channel(channel);

        if !self.is_recording[channel] {
            return;
        }

        if self.samples_recorded[channel] >= self.target_lengths[channel] {
            self.is_recording[channel] = false;
            return;
        }

        self.rec_rate_counters[channel] += 1.0;
        if self.rec_rate_counters[channel] < self.rec_rate_divisors[channel] {
            return;
        }
        self.rec_rate_counters[channel] = 0.0;

        let mlev = param_to_normalized(params.mlev);
        let ilev = param_to_normalized(params.ilev);

        let mbal = param_to_normalized(params.mbal);
        let main_l_gain = (1.0 - mbal).min(1.0);
        let main_r_gain = mbal.min(1.0);

        let ibal = param_to_normalized(params.ibal);
        let input_l_gain = (1.0 - ibal).min(1.0);
        let input_r_gain = ibal.min(1.0);

        let main_mix = (main_l * main_l_gain + main_r * main_r_gain) * mlev;
        let input_mix = (input_l * input_l_gain + input_r * input_r_gain) * ilev;
        let combined = (main_mix + input_mix).clamp(-1.0, 1.0);

        let sample_12bit = (combined * 2048.0).round().clamp(-2048.0, 2047.0) as i16;

        if !self.registry.write_sample(channel, sample_12bit) {
            self.is_recording[channel] = false;
            return;
        }

        self.samples_recorded[channel] += 1;

        if self.samples_recorded[channel] >= self.target_lengths[channel] {
            self.is_recording[channel] = false;
        }
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

    pub fn clear(&mut self, channel: usize) {
        let channel = Self::clamp_channel(channel);
        self.registry.clear_buffer(channel);
        self.samples_recorded[channel] = 0;
        self.is_recording[channel] = false;
        self.rec_rate_counters[channel] = 0.0;
        self.rec_rate_divisors[channel] = 1.0;
    }

    pub fn clear_all(&mut self) {
        self.registry.clear_all();
        for channel in 0..CHANNEL_COUNT {
            self.samples_recorded[channel] = 0;
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
        self.samples_recorded[channel] as f32 / target as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machines::shared::BufferRegistry;
    use std::sync::Arc;

    const SAMPLE_RATE: f32 = 100.0;

    fn basic_params() -> RamRecordParams {
        RamRecordParams {
            rec_len: 10,
            rec_rate: 127,
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
    fn recording_stops_at_configured_length() {
        let registry = Arc::new(BufferRegistry::new());
        let mut rec = RamRecord::with_registry(SAMPLE_RATE, registry.clone());
        let params = basic_params();

        rec.start_recording(&params, true, 0);
        let mut loop_guard = 0;
        while rec.is_recording(0) {
            record_one_sample(&mut rec, &params, 0);
            loop_guard += 1;
            assert!(loop_guard < 500, "recording loop did not end");
        }

        let normalized_len = param_to_normalized(params.rec_len);
        let max_length = (SAMPLE_RATE * RamRecord::MAX_RECORD_SECONDS) as usize;
        let expected = ((normalized_len * RamRecord::MAX_RECORD_SECONDS * SAMPLE_RATE) as usize)
            .min(max_length)
            .max(1);

        assert_eq!(registry.buffer_len(0), expected);
        assert_eq!(rec.recording_progress(0), 1.0);
    }

    #[test]
    fn retriggers_are_ignored_while_channel_is_recording() {
        let registry = Arc::new(BufferRegistry::new());
        let mut rec = RamRecord::with_registry(SAMPLE_RATE, registry.clone());
        let params = basic_params();

        rec.start_recording(&params, true, 2);
        record_one_sample(&mut rec, &params, 2);
        let initial_len = registry.buffer_len(2);
        assert!(initial_len > 0);
        assert!(rec.is_recording(2));

        rec.start_recording(&params, true, 2);
        assert_eq!(registry.buffer_len(2), initial_len);
        record_one_sample(&mut rec, &params, 2);
        assert_eq!(registry.buffer_len(2), initial_len + 1);
    }

    #[test]
    fn recording_length_is_locked_at_trigger_time() {
        let registry = Arc::new(BufferRegistry::new());
        let mut rec = RamRecord::with_registry(SAMPLE_RATE, registry.clone());
        let start_params = RamRecordParams {
            rec_len: 10,
            ..Default::default()
        };
        let changed_params = RamRecordParams {
            rec_len: 120,
            ..start_params
        };

        rec.start_recording(&start_params, true, 1);
        let mut loop_guard = 0;
        while rec.is_recording(1) {
            rec.record_sample(0.5, 0.5, 0.5, 0.5, &changed_params, 1);
            loop_guard += 1;
            assert!(loop_guard < 500, "recording loop did not end");
        }

        let normalized_len = param_to_normalized(start_params.rec_len);
        let max_length = (SAMPLE_RATE * RamRecord::MAX_RECORD_SECONDS) as usize;
        let expected = ((normalized_len * RamRecord::MAX_RECORD_SECONDS * SAMPLE_RATE) as usize)
            .min(max_length)
            .max(1);

        assert_eq!(registry.buffer_len(1), expected);
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
    fn target_length_matches_len_param() {
        let registry = Arc::new(BufferRegistry::new());
        let mut rec = RamRecord::with_registry(SAMPLE_RATE, registry);
        let params = RamRecordParams {
            rec_len: 64,
            ..Default::default()
        };

        rec.start_recording(&params, true, 0);
        let normalized_len = param_to_normalized(params.rec_len);
        let max_length = (SAMPLE_RATE * RamRecord::MAX_RECORD_SECONDS) as usize;
        let expected = ((normalized_len * RamRecord::MAX_RECORD_SECONDS * SAMPLE_RATE) as usize)
            .min(max_length)
            .max(1);

        assert_eq!(rec.target_length(0), expected);
    }

    #[test]
    fn target_length_is_locked_against_retriggers() {
        let registry = Arc::new(BufferRegistry::new());
        let mut rec = RamRecord::with_registry(SAMPLE_RATE, registry.clone());
        let first_params = RamRecordParams {
            rec_len: 10,
            ..Default::default()
        };
        let second_params = RamRecordParams {
            rec_len: 120,
            ..Default::default()
        };

        rec.start_recording(&first_params, true, 1);
        let initial_target = rec.target_length(1);
        rec.start_recording(&second_params, true, 1);
        assert_eq!(rec.target_length(1), initial_target);
    }
}
