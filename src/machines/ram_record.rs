pub struct RamRecord {
    buffer: Vec<i16>,
    write_position: usize,
    max_length: usize,
    is_recording: bool,
    sample_rate: f32,
}

impl RamRecord {
    pub fn new(sample_rate: f32, max_seconds: f32) -> Self {
        let max_length = (sample_rate * max_seconds) as usize;
        Self {
            buffer: Vec::with_capacity(max_length),
            write_position: 0,
            max_length,
            is_recording: false,
            sample_rate,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32, max_seconds: f32) {
        self.sample_rate = sample_rate;
        self.max_length = (sample_rate * max_seconds) as usize;
    }

    pub fn start_recording(&mut self, clear: bool) {
        if clear {
            self.buffer.clear();
            self.write_position = 0;
        }
        self.is_recording = true;
    }

    pub fn stop_recording(&mut self) {
        self.is_recording = false;
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording
    }

    pub fn record_sample(&mut self, main_l: f32, main_r: f32, input_l: f32, input_r: f32, mlev: f32, ilev: f32) {
        if !self.is_recording || self.write_position >= self.max_length {
            return;
        }

        let main_mix = (main_l + main_r) * 0.5 * mlev;
        let input_mix = (input_l + input_r) * 0.5 * ilev;
        let combined = main_mix + input_mix;

        let sample_12bit = (combined * 2048.0).round().clamp(-2048.0, 2047.0) as i16;

        if self.write_position < self.buffer.len() {
            self.buffer[self.write_position] = sample_12bit;
        } else {
            self.buffer.push(sample_12bit);
        }

        self.write_position += 1;
    }

    pub fn get_buffer(&self) -> &[i16] {
        &self.buffer
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.write_position = 0;
        self.is_recording = false;
    }
}
