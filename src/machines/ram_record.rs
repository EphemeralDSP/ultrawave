use crate::dsp::sample_engine::param_to_normalized;
use crate::machines::memory::{SampleMemory, MAX_BUFFER_SAMPLES};
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
        0..=15 => Some(0.5),
        16..=31 => Some(1.0),
        32..=47 => Some(2.0),
        48..=63 => Some(4.0),
        64..=79 => Some(8.0),
        80..=95 => Some(16.0),
        96..=111 => Some(32.0),
        _ => None,
    }
}

pub struct RamRecord {
    memory: Arc<SampleMemory>,
    sample_rate: f32,
    max_length: usize,
    target_length: usize,
    samples_recorded: usize,
    elapsed_samples: usize,
    is_recording: bool,
    rec_rate_counter: f32,
    rec_rate_divisor: f32,
    monitor: f32,
}

impl RamRecord {
    const MAX_RECORD_SECONDS: f32 = 10.0;

    pub fn new(sample_rate: f32, memory: Arc<SampleMemory>) -> Self {
        let max_length = (sample_rate * Self::MAX_RECORD_SECONDS) as usize;
        Self {
            memory,
            sample_rate,
            max_length,
            target_length: MAX_BUFFER_SAMPLES,
            samples_recorded: 0,
            elapsed_samples: 0,
            is_recording: false,
            rec_rate_counter: 0.0,
            rec_rate_divisor: 1.0,
            monitor: 0.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.max_length = (sample_rate * Self::MAX_RECORD_SECONDS) as usize;
        self.target_length = self.target_length.min(self.max_length);
    }

    /// Starts a new capture, replacing the previous sample.
    pub fn start_recording(&mut self, params: &RamRecordParams) {
        self.start(params, false);
    }

    /// Retain the published take for playback until this capture ends. The new
    /// take (or a nonempty interrupted prefix) then replaces it atomically.
    pub fn start_resampling(&mut self, params: &RamRecordParams) {
        self.start(params, true);
    }

    fn start(&mut self, params: &RamRecordParams, staging: bool) {
        self.stop_recording();
        let target_length = match rec_len_to_beats(params.rec_len) {
            Some(beats) => {
                let seconds = beats * (60.0 / params.tempo_bpm);
                (seconds * self.sample_rate as f64) as usize
            }
            None => self.max_length,
        };
        self.target_length = target_length.min(self.max_length).max(1);
        self.samples_recorded = 0;
        self.elapsed_samples = 0;

        let rate_normalized = param_to_normalized(params.rec_rate);
        self.rec_rate_divisor = 1.0 + (1.0 - rate_normalized) * 15.0;
        self.rec_rate_counter = 0.0;
        self.monitor = 0.0;

        let stored_rate = self.sample_rate / self.rec_rate_divisor.ceil();
        self.is_recording = self.memory.begin_recording(stored_rate, staging);
    }

    pub fn stop_recording(&mut self) {
        if self.is_recording {
            self.memory.finish_recording();
        }
        self.is_recording = false;
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording
    }

    pub fn record_sample(
        &mut self,
        main_l: f32,
        main_r: f32,
        input_l: f32,
        input_r: f32,
        params: &RamRecordParams,
    ) {
        let gains = params.input_gains();
        let mono = main_l * gains[0] + main_r * gains[1] + input_l * gains[2] + input_r * gains[3];
        self.record_mono(mono);
    }

    /// Capture an already mixed/processed sample without repeating input mixing.
    pub fn record_mono(&mut self, mono: f32) {
        if !self.is_recording {
            return;
        }

        self.rec_rate_counter += 1.0;
        if self.rec_rate_counter < self.rec_rate_divisor {
            self.elapsed_samples += 1;
            if self.elapsed_samples >= self.target_length {
                self.stop_recording();
            }
            return;
        }
        self.rec_rate_counter = 0.0;

        let sample_12bit = (mono * 2048.0).round().clamp(-2048.0, 2047.0) as i16;
        if !self.memory.write_sample(sample_12bit) {
            self.stop_recording();
            return;
        }

        self.monitor = sample_12bit as f32 / 2048.0;
        self.samples_recorded += 1;
        self.elapsed_samples += 1;
        if self.elapsed_samples >= self.target_length || self.samples_recorded == MAX_BUFFER_SAMPLES
        {
            self.stop_recording();
        }
    }

    pub fn monitor_sample(&self) -> f32 {
        self.monitor
    }

    pub fn get_buffer(&self) -> Vec<i16> {
        self.memory.read_buffer().unwrap_or_default()
    }

    pub fn buffer_len(&self) -> usize {
        self.memory.buffer_len()
    }

    pub fn target_length(&self) -> usize {
        self.target_length
    }

    pub fn recorded_samples(&self) -> usize {
        self.samples_recorded
    }

    pub fn samples_remaining(&self) -> usize {
        self.target_length.saturating_sub(self.elapsed_samples)
    }

    pub fn clear(&mut self) {
        self.memory.clear_buffer();
        self.samples_recorded = 0;
        self.elapsed_samples = 0;
        self.is_recording = false;
        self.rec_rate_counter = 0.0;
        self.rec_rate_divisor = 1.0;
    }

    pub fn recording_progress(&self) -> f32 {
        if self.target_length == 0 {
            return 0.0;
        }
        self.elapsed_samples as f32 / self.target_length as f32
    }
}

impl Drop for RamRecord {
    fn drop(&mut self) {
        self.stop_recording();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 100.0;

    fn basic_params() -> RamRecordParams {
        RamRecordParams {
            rec_len: 48,
            rec_rate: 127,
            tempo_bpm: 120.0,
            ..Default::default()
        }
    }

    fn recorder() -> (RamRecord, Arc<SampleMemory>) {
        let memory = Arc::new(SampleMemory::new());
        (RamRecord::new(SAMPLE_RATE, memory.clone()), memory)
    }

    fn record_one_sample(recorder: &mut RamRecord, params: &RamRecordParams) {
        recorder.record_sample(0.5, 0.5, 0.5, 0.5, params);
    }

    #[test]
    fn recording_starts_writes_and_stops_at_beat_synced_length() {
        let (mut recorder, memory) = recorder();
        let params = basic_params();
        recorder.start_recording(&params);
        assert!(recorder.is_recording());
        while recorder.is_recording() {
            record_one_sample(&mut recorder, &params);
        }
        assert_eq!(recorder.target_length(), 200);
        assert_eq!(memory.buffer_len(), 200);
    }

    #[test]
    fn retrigger_replaces_the_previous_sample() {
        let (mut recorder, memory) = recorder();
        let params = basic_params();
        recorder.start_recording(&params);
        for _ in 0..5 {
            record_one_sample(&mut recorder, &params);
        }
        assert_eq!(memory.buffer_len(), 5);
        recorder.start_recording(&params);
        assert!(recorder.is_recording());
        assert_eq!(memory.buffer_len(), 0);
        assert_eq!(recorder.samples_remaining(), recorder.target_length());
    }

    #[test]
    fn recording_length_is_locked_at_trigger_time() {
        let (mut recorder, memory) = recorder();
        let start = RamRecordParams {
            rec_len: 0,
            tempo_bpm: 120.0,
            ..Default::default()
        };
        let changed = RamRecordParams {
            rec_len: 112,
            ..start
        };
        recorder.start_recording(&start);
        while recorder.is_recording() {
            recorder.record_sample(0.5, 0.5, 0.5, 0.5, &changed);
        }
        assert_eq!(recorder.target_length(), 25);
        assert_eq!(memory.buffer_len(), 25);
    }

    #[test]
    fn beat_to_sample_at_various_tempos() {
        let (mut recorder, _) = recorder();
        for (tempo_bpm, rec_len, expected) in [
            (60.0, 48, 400),
            (180.0, 48, 133),
            (120.0, 0, 25),
            (120.0, 64, 400),
        ] {
            recorder.start_recording(&RamRecordParams {
                rec_len,
                tempo_bpm,
                ..Default::default()
            });
            assert_eq!(recorder.target_length(), expected);
        }
    }

    #[test]
    fn inf_mode_and_remaining_count_use_capacity_bound() {
        let (mut recorder, _) = recorder();
        recorder.start_recording(&RamRecordParams {
            rec_len: 127,
            ..Default::default()
        });
        assert_eq!(recorder.target_length(), 1000);

        let short = RamRecordParams {
            rec_len: 0,
            tempo_bpm: 120.0,
            ..Default::default()
        };
        recorder.start_recording(&short);
        assert_eq!(recorder.samples_remaining(), 25);
        while recorder.samples_remaining() > 0 {
            record_one_sample(&mut recorder, &short);
        }
        assert_eq!(recorder.samples_remaining(), 0);
        assert!(!recorder.is_recording());
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
