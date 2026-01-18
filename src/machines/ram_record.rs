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
    buffers: [Vec<i16>; 8],
    write_positions: [usize; 8],
    max_length: usize,
    target_length: usize,
    is_recording: [bool; 8],
    sample_rate: f32,
    rec_rate_counter: f32,
    rec_rate_divisor: f32,
}

impl RamRecord {
    const MAX_RECORD_SECONDS: f32 = 10.0;

    pub fn new(sample_rate: f32) -> Self {
        let max_length = (sample_rate * Self::MAX_RECORD_SECONDS) as usize;
        Self {
            buffers: [
                Vec::with_capacity(max_length),
                Vec::with_capacity(max_length),
                Vec::with_capacity(max_length),
                Vec::with_capacity(max_length),
                Vec::with_capacity(max_length),
                Vec::with_capacity(max_length),
                Vec::with_capacity(max_length),
                Vec::with_capacity(max_length),
            ],
            write_positions: [0; 8],
            max_length,
            target_length: max_length,
            is_recording: [false; 8],
            sample_rate,
            rec_rate_counter: 0.0,
            rec_rate_divisor: 1.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.max_length = (sample_rate * Self::MAX_RECORD_SECONDS) as usize;
    }

    pub fn start_recording(&mut self, params: &RamRecordParams, clear: bool, channel: usize) {
        let channel = channel.min(7);

        if clear {
            self.buffers[channel].clear();
            self.write_positions[channel] = 0;
        }

        let len_normalized = param_to_normalized(params.rec_len);
        self.target_length = ((len_normalized * Self::MAX_RECORD_SECONDS * self.sample_rate)
            as usize)
            .min(self.max_length)
            .max(1);

        let rate_normalized = param_to_normalized(params.rec_rate);
        self.rec_rate_divisor = 1.0 + (1.0 - rate_normalized) * 15.0;
        self.rec_rate_counter = 0.0;

        self.is_recording[channel] = true;
    }

    pub fn stop_recording(&mut self, channel: usize) {
        let channel = channel.min(7);
        self.is_recording[channel] = false;
    }

    pub fn stop_all_recording(&mut self) {
        self.is_recording = [false; 8];
    }

    pub fn is_recording(&self, channel: usize) -> bool {
        let channel = channel.min(7);
        self.is_recording[channel]
    }

    pub fn is_any_recording(&self) -> bool {
        self.is_recording.iter().any(|&r| r)
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
        let channel = channel.min(7);

        if !self.is_recording[channel] {
            return;
        }

        if self.write_positions[channel] >= self.target_length {
            self.is_recording[channel] = false;
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

        if self.write_positions[channel] < self.buffers[channel].len() {
            self.buffers[channel][self.write_positions[channel]] = sample_12bit;
        } else {
            self.buffers[channel].push(sample_12bit);
        }

        self.write_positions[channel] += 1;
    }

    pub fn get_buffer(&self, channel: usize) -> Vec<i16> {
        let channel = channel.min(7);
        self.buffers[channel].clone()
    }

    pub fn get_all_buffers(&self) -> &[Vec<i16>; 8] {
        &self.buffers
    }

    pub fn buffer_len(&self, channel: usize) -> usize {
        let channel = channel.min(7);
        self.buffers[channel].len()
    }

    pub fn clear(&mut self, channel: usize) {
        let channel = channel.min(7);
        self.buffers[channel].clear();
        self.write_positions[channel] = 0;
        self.is_recording[channel] = false;
        self.rec_rate_counter = 0.0;
    }

    pub fn clear_all(&mut self) {
        for i in 0..8 {
            self.buffers[i].clear();
            self.write_positions[i] = 0;
            self.is_recording[i] = false;
        }
        self.rec_rate_counter = 0.0;
    }

    pub fn recording_progress(&self, channel: usize) -> f32 {
        let channel = channel.min(7);
        if self.target_length == 0 {
            return 0.0;
        }
        self.write_positions[channel] as f32 / self.target_length as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_empty_buffers() {
        let rec = RamRecord::new(44100.0);
        for ch in 0..8 {
            assert_eq!(rec.buffer_len(ch), 0);
            assert!(!rec.is_recording(ch));
        }
    }

    #[test]
    fn test_start_recording_enables_flag() {
        let mut rec = RamRecord::new(44100.0);
        rec.start_recording(&RamRecordParams::default(), true, 0);
        assert!(rec.is_recording(0));
        assert!(!rec.is_recording(1));
    }

    #[test]
    fn test_stop_recording_disables_flag() {
        let mut rec = RamRecord::new(44100.0);
        rec.start_recording(&RamRecordParams::default(), true, 0);
        rec.stop_recording(0);
        assert!(!rec.is_recording(0));
    }

    #[test]
    fn test_record_sample_adds_to_buffer() {
        let mut rec = RamRecord::new(44100.0);
        let params = RamRecordParams {
            rec_rate: 127,
            ..Default::default()
        };
        rec.start_recording(&params, true, 0);
        rec.record_sample(0.5, 0.5, 0.0, 0.0, &params, 0);
        assert!(rec.buffer_len(0) > 0);
        assert_eq!(rec.buffer_len(1), 0);
    }

    #[test]
    fn test_12bit_quantization() {
        let mut rec = RamRecord::new(44100.0);
        let params = RamRecordParams {
            rec_rate: 127,
            mlev: 127,
            ..Default::default()
        };
        rec.start_recording(&params, true, 0);
        rec.record_sample(1.0, 1.0, 0.0, 0.0, &params, 0);
        let buffer = rec.get_buffer(0);
        assert!(!buffer.is_empty());
        assert!(buffer[0] <= 2047 && buffer[0] >= -2048);
    }

    #[test]
    fn test_clear_resets_buffer() {
        let mut rec = RamRecord::new(44100.0);
        let params = RamRecordParams::default();
        rec.start_recording(&params, true, 0);
        rec.record_sample(0.5, 0.5, 0.0, 0.0, &params, 0);
        rec.clear(0);
        assert_eq!(rec.buffer_len(0), 0);
        assert!(!rec.is_recording(0));
    }

    #[test]
    fn test_multi_channel_recording() {
        let mut rec = RamRecord::new(44100.0);
        let params = RamRecordParams {
            rec_rate: 127,
            ..Default::default()
        };

        rec.start_recording(&params, true, 0);
        rec.start_recording(&params, true, 3);

        rec.record_sample(0.5, 0.5, 0.0, 0.0, &params, 0);
        rec.record_sample(0.3, 0.3, 0.0, 0.0, &params, 3);

        assert!(rec.buffer_len(0) > 0);
        assert_eq!(rec.buffer_len(1), 0);
        assert_eq!(rec.buffer_len(2), 0);
        assert!(rec.buffer_len(3) > 0);
    }
}
