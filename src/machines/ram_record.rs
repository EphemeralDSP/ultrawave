use crate::dsp::sample_engine::param_to_normalized;

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
    buffer: Vec<i16>,
    write_position: usize,
    max_length: usize,
    target_length: usize,
    is_recording: bool,
    sample_rate: f32,
    rec_rate_counter: f32,
    rec_rate_divisor: f32,
}

impl RamRecord {
    const MAX_RECORD_SECONDS: f32 = 10.0;

    pub fn new(sample_rate: f32) -> Self {
        let max_length = (sample_rate * Self::MAX_RECORD_SECONDS) as usize;
        Self {
            buffer: Vec::with_capacity(max_length),
            write_position: 0,
            max_length,
            target_length: max_length,
            is_recording: false,
            sample_rate,
            rec_rate_counter: 0.0,
            rec_rate_divisor: 1.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.max_length = (sample_rate * Self::MAX_RECORD_SECONDS) as usize;
    }

    pub fn start_recording(&mut self, params: &RamRecordParams, clear: bool) {
        if clear {
            self.buffer.clear();
            self.write_position = 0;
        }

        let len_normalized = param_to_normalized(params.rec_len);
        self.target_length = ((len_normalized * Self::MAX_RECORD_SECONDS * self.sample_rate)
            as usize)
            .min(self.max_length)
            .max(1);

        let rate_normalized = param_to_normalized(params.rec_rate);
        self.rec_rate_divisor = 1.0 + (1.0 - rate_normalized) * 15.0;
        self.rec_rate_counter = 0.0;

        self.is_recording = true;
    }

    pub fn stop_recording(&mut self) {
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
        if !self.is_recording {
            return;
        }

        if self.write_position >= self.target_length {
            self.is_recording = false;
            return;
        }

        self.rec_rate_counter += 1.0;
        if self.rec_rate_counter < self.rec_rate_divisor {
            return;
        }
        self.rec_rate_counter = 0.0;

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

        if self.write_position < self.buffer.len() {
            self.buffer[self.write_position] = sample_12bit;
        } else {
            self.buffer.push(sample_12bit);
        }

        self.write_position += 1;
    }

    pub fn get_buffer(&self) -> Vec<i16> {
        self.buffer.clone()
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.write_position = 0;
        self.is_recording = false;
        self.rec_rate_counter = 0.0;
    }

    pub fn recording_progress(&self) -> f32 {
        if self.target_length == 0 {
            return 0.0;
        }
        self.write_position as f32 / self.target_length as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_empty_buffer() {
        let rec = RamRecord::new(44100.0);
        assert_eq!(rec.buffer_len(), 0);
        assert!(!rec.is_recording());
    }

    #[test]
    fn test_start_recording_enables_flag() {
        let mut rec = RamRecord::new(44100.0);
        rec.start_recording(&RamRecordParams::default(), true);
        assert!(rec.is_recording());
    }

    #[test]
    fn test_stop_recording_disables_flag() {
        let mut rec = RamRecord::new(44100.0);
        rec.start_recording(&RamRecordParams::default(), true);
        rec.stop_recording();
        assert!(!rec.is_recording());
    }

    #[test]
    fn test_record_sample_adds_to_buffer() {
        let mut rec = RamRecord::new(44100.0);
        let params = RamRecordParams {
            rec_rate: 127,
            ..Default::default()
        };
        rec.start_recording(&params, true);
        rec.record_sample(0.5, 0.5, 0.0, 0.0, &params);
        assert!(rec.buffer_len() > 0);
    }

    #[test]
    fn test_12bit_quantization() {
        let mut rec = RamRecord::new(44100.0);
        let params = RamRecordParams {
            rec_rate: 127,
            mlev: 127,
            ..Default::default()
        };
        rec.start_recording(&params, true);
        rec.record_sample(1.0, 1.0, 0.0, 0.0, &params);
        let buffer = rec.get_buffer();
        assert!(!buffer.is_empty());
        assert!(buffer[0] <= 2047 && buffer[0] >= -2048);
    }

    #[test]
    fn test_clear_resets_buffer() {
        let mut rec = RamRecord::new(44100.0);
        let params = RamRecordParams::default();
        rec.start_recording(&params, true);
        rec.record_sample(0.5, 0.5, 0.0, 0.0, &params);
        rec.clear();
        assert_eq!(rec.buffer_len(), 0);
        assert!(!rec.is_recording());
    }
}
