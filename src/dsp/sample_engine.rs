pub struct SampleEngine {
    buffer: Vec<i16>,
    position: f64,
    sample_rate: f32,
}

impl SampleEngine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            buffer: Vec::new(),
            position: 0.0,
            sample_rate,
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
    }

    pub fn trigger(&mut self, start_position: f64) {
        self.position = start_position.clamp(0.0, self.buffer.len() as f64);
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

    pub fn is_finished(&self, end_position: f64) -> bool {
        self.position >= end_position || self.position >= self.buffer.len() as f64
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }
}
