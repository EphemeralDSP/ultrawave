use crate::dsp::sample_engine::{pitch_to_ratio, param_to_normalized, SampleEngine};

pub struct RamPlayParams {
    pub strt: i32,
    pub end: i32,
    pub pitch: i32,
    pub hold: i32,
    pub dec: i32,
    pub rtrg: i32,
    pub rtim: i32,
    pub srr: i32,
    pub vol: i32,
}

impl Default for RamPlayParams {
    fn default() -> Self {
        Self {
            strt: 0,
            end: 127,
            pitch: 64,
            hold: 0,
            dec: 0,
            rtrg: 0,
            rtim: 0,
            srr: 0,
            vol: 100,
        }
    }
}

pub struct RamPlay {
    engine: SampleEngine,
    envelope: f32,
    is_playing: bool,
    hold_time_samples: usize,
    hold_counter: usize,
    decay_rate: f32,
    sample_rate: f32,
    retrig_counter: usize,
    retrig_interval: usize,
    retrigs_remaining: usize,
    current_params: RamPlayParams,
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
            retrig_counter: 0,
            retrig_interval: 0,
            retrigs_remaining: 0,
            current_params: RamPlayParams::default(),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.engine.set_sample_rate(sample_rate);
    }

    pub fn load_buffer(&mut self, samples: Vec<i16>) {
        self.engine.load_buffer(samples);
    }

    pub fn trigger(&mut self, params: &RamPlayParams) {
        self.current_params = RamPlayParams {
            strt: params.strt,
            end: params.end,
            pitch: params.pitch,
            hold: params.hold,
            dec: params.dec,
            rtrg: params.rtrg,
            rtim: params.rtim,
            srr: params.srr,
            vol: params.vol,
        };

        self.trigger_internal();

        if params.rtrg > 0 {
            self.retrigs_remaining = params.rtrg as usize;
            let rtim_ms = param_to_normalized(params.rtim) * 500.0;
            self.retrig_interval = (rtim_ms * self.sample_rate / 1000.0) as usize;
            self.retrig_counter = 0;
        } else {
            self.retrigs_remaining = 0;
        }
    }

    fn trigger_internal(&mut self) {
        let buffer_len = self.engine.buffer_len() as f64;
        let start_norm = param_to_normalized(self.current_params.strt);
        let start_pos = (start_norm as f64) * buffer_len;
        self.engine.trigger(start_pos);

        self.envelope = 1.0;
        self.is_playing = true;

        let hold_seconds = param_to_normalized(self.current_params.hold) * 2.0;
        self.hold_time_samples = (hold_seconds * self.sample_rate) as usize;
        self.hold_counter = 0;

        let decay_seconds = param_to_normalized(self.current_params.dec) * 4.0;
        self.decay_rate = if decay_seconds > 0.0 {
            1.0 / (decay_seconds * self.sample_rate)
        } else {
            1.0
        };
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        self.envelope = 0.0;
        self.retrigs_remaining = 0;
    }

    pub fn process(&mut self) -> f32 {
        if self.retrigs_remaining > 0 {
            self.retrig_counter += 1;
            if self.retrig_counter >= self.retrig_interval {
                self.retrig_counter = 0;
                self.retrigs_remaining -= 1;
                self.trigger_internal();
            }
        }

        if !self.is_playing {
            return 0.0;
        }

        let buffer_len = self.engine.buffer_len() as f64;
        let end_norm = param_to_normalized(self.current_params.end);
        let end_pos = (end_norm as f64) * buffer_len;

        if self.engine.is_finished(end_pos) {
            self.is_playing = false;
            return 0.0;
        }

        let pitch_ratio = pitch_to_ratio(self.current_params.pitch);
        let sample = self.engine.read_sample_with_processing(pitch_ratio, self.current_params.srr);

        if self.hold_counter < self.hold_time_samples {
            self.hold_counter += 1;
        } else {
            self.envelope = (self.envelope - self.decay_rate).max(0.0);
            if self.envelope <= 0.0 {
                self.is_playing = false;
            }
        }

        let vol = param_to_normalized(self.current_params.vol);
        sample * self.envelope * vol
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing || self.retrigs_remaining > 0
    }

    pub fn buffer_len(&self) -> usize {
        self.engine.buffer_len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_buffer() -> Vec<i16> {
        (0..1000).map(|i| ((i as f32 / 1000.0 * 32767.0) as i16)).collect()
    }

    #[test]
    fn test_trigger_starts_playback() {
        let mut player = RamPlay::new(44100.0);
        player.load_buffer(make_test_buffer());
        player.trigger(&RamPlayParams::default());
        assert!(player.is_playing());
    }

    #[test]
    fn test_stop_halts_playback() {
        let mut player = RamPlay::new(44100.0);
        player.load_buffer(make_test_buffer());
        player.trigger(&RamPlayParams::default());
        player.stop();
        assert!(!player.is_playing());
    }

    #[test]
    fn test_process_outputs_audio() {
        let mut player = RamPlay::new(44100.0);
        player.load_buffer(make_test_buffer());
        player.trigger(&RamPlayParams::default());
        let sample = player.process();
        assert!(sample >= -1.0 && sample <= 1.0);
    }

    #[test]
    fn test_retrigger_restarts_playback() {
        let mut player = RamPlay::new(44100.0);
        player.load_buffer(make_test_buffer());
        let params = RamPlayParams {
            rtrg: 2,
            rtim: 10,
            ..Default::default()
        };
        player.trigger(&params);
        assert_eq!(player.retrigs_remaining, 2);
    }
}
