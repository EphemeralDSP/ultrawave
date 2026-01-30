use crate::dsp::sample_engine::{param_to_normalized, pitch_to_ratio, SampleEngine};

#[derive(Clone, Copy)]
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
    engines: [SampleEngine; 8],
    envelopes: [f32; 8],
    is_playing: [bool; 8],
    hold_time_samples: [usize; 8],
    hold_counters: [usize; 8],
    decay_rates: [f32; 8],
    sample_rate: f32,
    retrig_counters: [usize; 8],
    retrig_intervals: [usize; 8],
    retrigs_remaining: [usize; 8],
    current_params: [RamPlayParams; 8],
}

impl RamPlay {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            engines: [
                SampleEngine::new(sample_rate),
                SampleEngine::new(sample_rate),
                SampleEngine::new(sample_rate),
                SampleEngine::new(sample_rate),
                SampleEngine::new(sample_rate),
                SampleEngine::new(sample_rate),
                SampleEngine::new(sample_rate),
                SampleEngine::new(sample_rate),
            ],
            envelopes: [0.0; 8],
            is_playing: [false; 8],
            hold_time_samples: [0; 8],
            hold_counters: [0; 8],
            decay_rates: [0.0; 8],
            sample_rate,
            retrig_counters: [0; 8],
            retrig_intervals: [0; 8],
            retrigs_remaining: [0; 8],
            current_params: [RamPlayParams::default(); 8],
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        for engine in &mut self.engines {
            engine.set_sample_rate(sample_rate);
        }
    }

    pub fn load_buffer(&mut self, samples: Vec<i16>, channel: usize) {
        let channel = channel.min(7);
        self.engines[channel].load_buffer(samples);
    }

    pub fn load_all_buffers(&mut self, buffers: &[Vec<i16>; 8]) {
        for (i, buffer) in buffers.iter().enumerate() {
            self.engines[i].load_buffer(buffer.clone());
        }
    }

    pub fn trigger(&mut self, params: &RamPlayParams, channel: usize) {
        let channel = channel.min(7);
        self.current_params[channel] = *params;

        self.trigger_internal(channel);

        if params.rtrg > 0 {
            self.retrigs_remaining[channel] = params.rtrg as usize;
            let rtim_ms = param_to_normalized(params.rtim) * 500.0;
            self.retrig_intervals[channel] = (rtim_ms * self.sample_rate / 1000.0) as usize;
            self.retrig_counters[channel] = 0;
        } else {
            self.retrigs_remaining[channel] = 0;
        }
    }

    fn trigger_internal(&mut self, channel: usize) {
        let buffer_len = self.engines[channel].buffer_len() as f64;
        let start_norm = param_to_normalized(self.current_params[channel].strt);
        let start_pos = (start_norm as f64) * buffer_len;
        self.engines[channel].trigger(start_pos);

        self.envelopes[channel] = 1.0;
        self.is_playing[channel] = true;

        let hold_seconds = param_to_normalized(self.current_params[channel].hold) * 2.0;
        self.hold_time_samples[channel] = (hold_seconds * self.sample_rate) as usize;
        self.hold_counters[channel] = 0;

        let decay_seconds = param_to_normalized(self.current_params[channel].dec) * 4.0;
        self.decay_rates[channel] = if decay_seconds > 0.0 {
            1.0 / (decay_seconds * self.sample_rate)
        } else {
            1.0
        };
    }

    pub fn stop(&mut self, channel: usize) {
        let channel = channel.min(7);
        self.is_playing[channel] = false;
        self.envelopes[channel] = 0.0;
        self.retrigs_remaining[channel] = 0;
    }

    pub fn stop_all(&mut self) {
        self.is_playing = [false; 8];
        self.envelopes = [0.0; 8];
        self.retrigs_remaining = [0; 8];
    }

    pub fn process(&mut self, channel: usize) -> f32 {
        let channel = channel.min(7);

        if self.retrigs_remaining[channel] > 0 {
            self.retrig_counters[channel] += 1;
            if self.retrig_counters[channel] >= self.retrig_intervals[channel] {
                self.retrig_counters[channel] = 0;
                self.retrigs_remaining[channel] -= 1;
                self.trigger_internal(channel);
            }
        }

        if !self.is_playing[channel] {
            return 0.0;
        }

        let buffer_len = self.engines[channel].buffer_len() as f64;
        let end_norm = param_to_normalized(self.current_params[channel].end);
        let end_pos = (end_norm as f64) * buffer_len;

        if self.engines[channel].is_finished(end_pos) {
            self.is_playing[channel] = false;
            return 0.0;
        }

        let pitch_ratio = pitch_to_ratio(self.current_params[channel].pitch);
        let sample = self.engines[channel]
            .read_sample_with_processing(pitch_ratio, self.current_params[channel].srr);

        if self.hold_counters[channel] < self.hold_time_samples[channel] {
            self.hold_counters[channel] += 1;
        } else {
            self.envelopes[channel] =
                (self.envelopes[channel] - self.decay_rates[channel]).max(0.0);
            if self.envelopes[channel] <= 0.0 {
                self.is_playing[channel] = false;
            }
        }

        let vol = param_to_normalized(self.current_params[channel].vol);
        sample * self.envelopes[channel] * vol
    }

    pub fn process_mix(&mut self) -> f32 {
        let mut mix = 0.0;
        for ch in 0..8 {
            mix += self.process(ch);
        }
        mix / 8.0
    }

    pub fn is_playing(&self, channel: usize) -> bool {
        let channel = channel.min(7);
        self.is_playing[channel] || self.retrigs_remaining[channel] > 0
    }

    pub fn is_any_playing(&self) -> bool {
        for ch in 0..8 {
            if self.is_playing(ch) {
                return true;
            }
        }
        false
    }

    pub fn buffer_len(&self, channel: usize) -> usize {
        let channel = channel.min(7);
        self.engines[channel].buffer_len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_buffer() -> Vec<i16> {
        (0..1000)
            .map(|i| ((i as f32 / 1000.0 * 32767.0) as i16))
            .collect()
    }

    #[test]
    fn test_trigger_starts_playback() {
        let mut player = RamPlay::new(44100.0);
        player.load_buffer(make_test_buffer(), 0);
        player.trigger(&RamPlayParams::default(), 0);
        assert!(player.is_playing(0));
    }

    #[test]
    fn test_stop_halts_playback() {
        let mut player = RamPlay::new(44100.0);
        player.load_buffer(make_test_buffer(), 0);
        player.trigger(&RamPlayParams::default(), 0);
        player.stop(0);
        assert!(!player.is_playing(0));
    }

    #[test]
    fn test_process_outputs_audio() {
        let mut player = RamPlay::new(44100.0);
        player.load_buffer(make_test_buffer(), 0);
        player.trigger(&RamPlayParams::default(), 0);
        let sample = player.process(0);
        assert!(sample >= -1.0 && sample <= 1.0);
    }

    #[test]
    fn test_retrigger_restarts_playback() {
        let mut player = RamPlay::new(44100.0);
        player.load_buffer(make_test_buffer(), 0);
        let params = RamPlayParams {
            rtrg: 2,
            rtim: 10,
            ..Default::default()
        };
        player.trigger(&params, 0);
        assert_eq!(player.retrigs_remaining[0], 2);
    }

    #[test]
    fn test_multi_channel_playback() {
        let mut player = RamPlay::new(44100.0);
        player.load_buffer(make_test_buffer(), 0);
        player.load_buffer(make_test_buffer(), 3);

        player.trigger(&RamPlayParams::default(), 0);
        player.trigger(&RamPlayParams::default(), 3);

        assert!(player.is_playing(0));
        assert!(!player.is_playing(1));
        assert!(player.is_playing(3));

        let sample0 = player.process(0);
        let sample3 = player.process(3);

        assert!(sample0 >= -1.0 && sample0 <= 1.0);
        assert!(sample3 >= -1.0 && sample3 <= 1.0);
    }
}
