use crate::dsp::sample_engine::SampleEngine;

pub struct RamPlay {
    engine: SampleEngine,
    envelope: f32,
    is_playing: bool,
    hold_time_samples: usize,
    hold_counter: usize,
    decay_rate: f32,
    sample_rate: f32,
}

impl RamPlay {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            engine: SampleEngine::new(sample_rate),
            envelope: 0.0,
            is_playing: false,
            hold_time_samples: 0,
            hold_counter: 0,
            decay_rate: 0.0,
            sample_rate,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.engine.set_sample_rate(sample_rate);
    }

    pub fn load_buffer(&mut self, samples: Vec<i16>) {
        self.engine.load_buffer(samples);
    }

    pub fn trigger(&mut self, start: f32, hold: f32, decay: f32) {
        let buffer_len = self.engine.buffer_len() as f64;
        let start_pos = (start as f64) * buffer_len;
        self.engine.trigger(start_pos);

        self.envelope = 1.0;
        self.is_playing = true;
        self.hold_time_samples = (hold * self.sample_rate) as usize;
        self.hold_counter = 0;
        self.decay_rate = if decay > 0.0 { 1.0 / (decay * self.sample_rate) } else { 1.0 };
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        self.envelope = 0.0;
    }

    pub fn process(&mut self, pitch_ratio: f64, end: f32) -> f32 {
        if !self.is_playing {
            return 0.0;
        }

        let buffer_len = self.engine.buffer_len() as f64;
        let end_pos = (end as f64) * buffer_len;

        if self.engine.is_finished(end_pos) {
            self.is_playing = false;
            return 0.0;
        }

        let sample = self.engine.read_sample_linear(pitch_ratio);

        if self.hold_counter < self.hold_time_samples {
            self.hold_counter += 1;
        } else {
            self.envelope = (self.envelope - self.decay_rate).max(0.0);
            if self.envelope <= 0.0 {
                self.is_playing = false;
            }
        }

        sample * self.envelope
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing
    }
}
