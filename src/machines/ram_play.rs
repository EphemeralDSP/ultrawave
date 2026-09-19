use crate::dsp::effects::Distortion;
use crate::dsp::sample_engine::{param_to_normalized, pitch_to_ratio, SampleEngine};
use crate::machines::shared::{get_global_registry, BufferRegistry, CHANNEL_COUNT};
use std::sync::Arc;

/// Quarter-note beats between retriggers. Zero disables retriggering.
/// Landmarks are Legofriendly's reported MD timings in Tarekith's tips, not
/// firmware-derived measurements: https://tarekith.com/assets/machinedrum_tipsandtricks.htm#Reference
/// Interpolate logarithmically between landmarks; below 32, extrapolate the
/// doubling-per-32 trend. Those intermediate values still need hardware checks.
pub fn retrig_interval_beats(value: i32) -> f64 {
    if value == 0 {
        return 0.0;
    }
    const RATES: [(i32, f64); 14] = [
        (0, 64.0),
        (32, 32.0),
        (44, 24.0),
        (52, 20.0),
        (64, 16.0),
        (77, 12.0),
        (81, 11.0),
        (85, 10.0),
        (90, 9.0),
        (96, 8.0),
        (101, 7.0),
        (109, 6.0),
        (117, 5.0),
        (127, 4.0),
    ];
    for pair in RATES.windows(2) {
        let [(lo, lo_rate), (hi, hi_rate)] = [pair[0], pair[1]];
        if value <= hi {
            let fraction = (value - lo) as f64 / (hi - lo) as f64;
            return (lo_rate / hi_rate).powf(fraction) / lo_rate;
        }
    }
    0.25
}

#[derive(Clone, Copy)]
pub struct RamPlayParams {
    pub strt: i32,
    pub end: i32,
    pub pitch: i32,
    pub hold: i32,
    pub dec: i32,
    pub rtrg: i32,
    pub rtim: i32,
    pub srr: f32,
    pub brr: i32,
    pub dist: i32,
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
            srr: 0.0,
            brr: 0,
            dist: 0,
            vol: 100,
        }
    }
}

pub struct RamPlay {
    registry: Arc<BufferRegistry>,
    engines: [SampleEngine; CHANNEL_COUNT],
    envelopes: [f32; CHANNEL_COUNT],
    is_playing: [bool; CHANNEL_COUNT],
    hold_time_samples: [usize; CHANNEL_COUNT],
    hold_counters: [usize; CHANNEL_COUNT],
    decay_rates: [f32; CHANNEL_COUNT],
    sample_rate: f32,
    tempo_bpm: f64,
    retrig_counters: [f64; CHANNEL_COUNT],
    retrig_intervals: [f64; CHANNEL_COUNT],
    retrigs_remaining: [usize; CHANNEL_COUNT],
    current_params: [RamPlayParams; CHANNEL_COUNT],
    distortion: [Distortion; CHANNEL_COUNT],
    playback_ratios: [f64; CHANNEL_COUNT],
    reverse: [bool; CHANNEL_COUNT],
}

#[allow(dead_code)]
impl RamPlay {
    pub fn new(sample_rate: f32) -> Self {
        Self::with_registry(sample_rate, get_global_registry())
    }

    pub fn with_registry(sample_rate: f32, registry: Arc<BufferRegistry>) -> Self {
        Self {
            registry,
            engines: [
                SampleEngine::new(sample_rate),
                SampleEngine::new(sample_rate),
                SampleEngine::new(sample_rate),
                SampleEngine::new(sample_rate),
            ],
            envelopes: [0.0; CHANNEL_COUNT],
            is_playing: [false; CHANNEL_COUNT],
            hold_time_samples: [0; CHANNEL_COUNT],
            hold_counters: [0; CHANNEL_COUNT],
            decay_rates: [0.0; CHANNEL_COUNT],
            sample_rate,
            tempo_bpm: 120.0,
            retrig_counters: [0.0; CHANNEL_COUNT],
            retrig_intervals: [0.0; CHANNEL_COUNT],
            retrigs_remaining: [0; CHANNEL_COUNT],
            current_params: [RamPlayParams::default(); CHANNEL_COUNT],
            distortion: std::array::from_fn(|_| Distortion::new()),
            playback_ratios: [1.0; CHANNEL_COUNT],
            reverse: [false; CHANNEL_COUNT],
        }
    }

    fn clamp_channel(channel: usize) -> usize {
        channel.min(CHANNEL_COUNT - 1)
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        for engine in &mut self.engines {
            engine.set_sample_rate(sample_rate);
        }
    }

    /// Follow host tempo changes without restarting the current beat interval.
    /// Invalid or unavailable host tempo falls back to 120 BPM.
    pub fn set_tempo(&mut self, tempo_bpm: f64) {
        let tempo_bpm = if tempo_bpm.is_finite() && tempo_bpm > 0.0 {
            tempo_bpm
        } else {
            120.0
        };
        if tempo_bpm != self.tempo_bpm {
            let scale = self.tempo_bpm / tempo_bpm;
            for channel in 0..CHANNEL_COUNT {
                self.retrig_counters[channel] *= scale;
                self.retrig_intervals[channel] *= scale;
            }
            self.tempo_bpm = tempo_bpm;
        }
    }

    pub fn load_buffer(&mut self, samples: Vec<i16>, channel: usize) {
        let channel = Self::clamp_channel(channel);
        let _ = self.registry.replace_buffer(channel, samples);
    }

    pub fn load_all_buffers(&mut self, buffers: &[Vec<i16>; CHANNEL_COUNT]) {
        for (i, buffer) in buffers.iter().enumerate() {
            let _ = self.registry.replace_buffer(i, buffer.clone());
        }
    }

    pub fn trigger(&mut self, params: &RamPlayParams, channel: usize) {
        let channel = Self::clamp_channel(channel);
        self.current_params[channel] = *params;
        self.update_params(params, channel);
        let source_rate = self.engines[channel].latch_shared(Arc::clone(&self.registry), channel);
        if source_rate.is_none() {
            self.is_playing[channel] = false;
            self.retrigs_remaining[channel] = 0;
            return;
        }
        self.playback_ratios[channel] =
            pitch_to_ratio(params.pitch) * source_rate.unwrap() as f64 / self.sample_rate as f64;

        self.trigger_internal(channel);

        if params.rtrg > 0 && params.rtim > 0 {
            self.retrigs_remaining[channel] = params.rtrg as usize;
            self.retrig_intervals[channel] =
                (retrig_interval_beats(params.rtim) * self.sample_rate as f64 * 60.0
                    / self.tempo_bpm)
                    .max(1.0);
            self.retrig_counters[channel] = 0.0;
        } else {
            self.retrigs_remaining[channel] = 0;
        }
    }

    /// Live playback controls. Start, direction, envelope and retrigger timing
    /// are latched on trigger; End remains live within the selected direction.
    pub fn update_params(&mut self, params: &RamPlayParams, channel: usize) {
        let channel = Self::clamp_channel(channel);
        // Turning either retrigger control off cancels pending repeats, even
        // an infinite run. Other count/time changes latch on the next note.
        if params.rtrg == 0 || params.rtim == 0 {
            self.retrigs_remaining[channel] = 0;
        }
        let current = &mut self.current_params[channel];
        current.pitch = params.pitch;
        current.end = params.end;
        current.srr = params.srr;
        current.brr = params.brr;
        current.dist = params.dist;
        current.vol = params.vol;
        self.distortion[channel].set_drive(param_to_normalized(params.dist));
        self.playback_ratios[channel] = pitch_to_ratio(params.pitch);
        if let Some((_, source_rate, _)) = self.registry.snapshot(channel) {
            self.playback_ratios[channel] *= source_rate as f64 / self.sample_rate as f64;
        }
    }

    fn trigger_internal(&mut self, channel: usize) {
        let buffer_len = self.engines[channel].buffer_len() as f64;
        let start = self.current_params[channel].strt;
        // Multiply before dividing so exact sample boundaries stay exact.
        let start_pos = start as f64 * buffer_len / 127.0;
        self.reverse[channel] = start > self.current_params[channel].end;
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
        let channel = Self::clamp_channel(channel);
        self.is_playing[channel] = false;
        self.envelopes[channel] = 0.0;
        self.retrigs_remaining[channel] = 0;
    }

    pub fn stop_all(&mut self) {
        self.is_playing = [false; CHANNEL_COUNT];
        self.envelopes = [0.0; CHANNEL_COUNT];
        self.retrigs_remaining = [0; CHANNEL_COUNT];
    }

    pub fn process(&mut self, channel: usize) -> f32 {
        let channel = Self::clamp_channel(channel);

        if self.retrigs_remaining[channel] > 0 {
            if !self.engines[channel].source_is_current() {
                self.stop(channel);
                return 0.0;
            }
            // Check elapsed frames BEFORE counting this frame. Carry fractional
            // overshoot so non-integral intervals never accumulate rounding drift.
            if self.retrig_counters[channel] + 0.5 > self.retrig_intervals[channel] {
                self.retrig_counters[channel] -= self.retrig_intervals[channel];
                if self.retrigs_remaining[channel] != 127 {
                    self.retrigs_remaining[channel] -= 1;
                }
                self.trigger_internal(channel);
            }
            self.retrig_counters[channel] += 1.0;
        }

        if !self.is_playing[channel] {
            return 0.0;
        }

        let buffer_len = self.engines[channel].buffer_len();
        let end_pos = self.current_params[channel].end as f64 * buffer_len as f64 / 127.0;
        let ratio = if self.reverse[channel] {
            -self.playback_ratios[channel]
        } else {
            self.playback_ratios[channel]
        };

        if self.engines[channel].is_finished(end_pos, ratio) {
            self.is_playing[channel] = false;
            return 0.0;
        }

        let sample = self.engines[channel].read_sample_with_processing(
            ratio,
            self.current_params[channel].srr,
            self.current_params[channel].brr,
        );
        let sample = self.distortion[channel].process(sample);
        if self.engines[channel].is_finished(end_pos, ratio) {
            self.is_playing[channel] = false;
        }

        let is_minimal_one_shot =
            self.current_params[channel].hold == 0 && self.current_params[channel].dec == 0;
        if !is_minimal_one_shot {
            if self.hold_counters[channel] < self.hold_time_samples[channel] {
                self.hold_counters[channel] += 1;
            } else {
                self.envelopes[channel] =
                    (self.envelopes[channel] - self.decay_rates[channel]).max(0.0);
                if self.envelopes[channel] <= 0.0 {
                    self.is_playing[channel] = false;
                }
            }
        }

        let vol = param_to_normalized(self.current_params[channel].vol);
        sample * self.envelopes[channel] * vol
    }

    pub fn process_mix(&mut self) -> f32 {
        let mut mix = 0.0;
        for ch in 0..CHANNEL_COUNT {
            mix += self.process(ch);
        }
        mix / CHANNEL_COUNT as f32
    }

    pub fn is_playing(&self, channel: usize) -> bool {
        let channel = Self::clamp_channel(channel);
        self.is_playing[channel] || self.retrigs_remaining[channel] > 0
    }

    pub fn is_any_playing(&self) -> bool {
        for ch in 0..CHANNEL_COUNT {
            if self.is_playing(ch) {
                return true;
            }
        }
        false
    }

    pub fn buffer_len(&self, channel: usize) -> usize {
        let channel = Self::clamp_channel(channel);
        self.registry.buffer_len(channel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machines::shared::BufferRegistry;
    use std::sync::Arc;

    fn make_test_buffer() -> Vec<i16> {
        (0..1000)
            .map(|i| (i as f32 / 1000.0 * 2047.0) as i16)
            .collect()
    }

    fn test_player() -> RamPlay {
        RamPlay::with_registry(44100.0, Arc::new(BufferRegistry::new()))
    }

    #[test]
    fn test_trigger_starts_playback() {
        let mut player = test_player();
        player.load_buffer(make_test_buffer(), 0);
        player.trigger(&RamPlayParams::default(), 0);
        assert!(player.is_playing(0));
    }

    #[test]
    fn test_stop_halts_playback() {
        let mut player = test_player();
        player.load_buffer(make_test_buffer(), 0);
        player.trigger(&RamPlayParams::default(), 0);
        player.stop(0);
        assert!(!player.is_playing(0));
    }

    #[test]
    fn test_process_outputs_audio() {
        let mut player = test_player();
        player.load_buffer(make_test_buffer(), 0);
        player.trigger(&RamPlayParams::default(), 0);
        let sample = player.process(0);
        assert!((-1.0..=1.0).contains(&sample));
    }

    #[test]
    fn retrigger_timeline_has_exact_count_off_and_infinite_boundaries() {
        // At 120 BPM, sixteenth notes are 125 ms = 128 frames at 1024 Hz.
        // A one-frame impulse also checks restarting after playback has ended.
        let registry = Arc::new(BufferRegistry::new());
        registry.replace_buffer_at_rate(0, vec![1024], 1024.0);
        for count in [0, 1, 2, 126, 127] {
            for time in [0, 127] {
                let mut player = RamPlay::with_registry(1024.0, registry.clone());
                player.trigger(
                    &RamPlayParams {
                        rtrg: count,
                        rtim: time,
                        vol: 127,
                        ..Default::default()
                    },
                    0,
                );
                for frame in 0..(140 * 128) {
                    let hit = frame == 0
                        || (time != 0
                            && frame % 128 == 0
                            && (count == 127 || frame / 128 <= count as usize));
                    assert_eq!(
                        player.process(0),
                        if hit { 0.5 } else { 0.0 },
                        "count={count} time={time} frame={frame}"
                    );
                }
                assert_eq!(player.is_playing(0), count == 127 && time != 0);
                player.stop_all();
                assert_eq!(player.process(0), 0.0);
                assert!(!player.is_playing(0));
            }
        }
    }

    #[test]
    fn retrigger_cancels_on_buffer_replacement_even_between_hits() {
        for reverse in [false, true] {
            for frames_before_replace in [1, 8] {
                let registry = Arc::new(BufferRegistry::new());
                registry.replace_buffer_at_rate(0, vec![1024; 4], 1024.0);
                let mut player = RamPlay::with_registry(1024.0, registry.clone());
                player.trigger(
                    &RamPlayParams {
                        strt: if reverse { 127 } else { 0 },
                        end: if reverse { 0 } else { 127 },
                        rtrg: 127,
                        rtim: 127,
                        vol: 127,
                        ..Default::default()
                    },
                    0,
                );
                for _ in 0..frames_before_replace {
                    player.process(0);
                }
                registry.replace_buffer(0, vec![-1024; 4]);
                assert_eq!(player.process(0), 0.0);
                assert!(
                    !player.is_playing(0),
                    "dead source must cancel infinite repeats"
                );
                for _ in 0..300 {
                    assert_eq!(player.process(0), 0.0);
                }
            }
        }
    }

    #[test]
    fn retrigger_fractional_intervals_do_not_drift_at_different_tempos_and_rates() {
        // Independently calculated sixteenth/triplet durations in output frames.
        for (rate, tempo, time, interval) in [
            (44_100.0, 120.0, 127, 5512.5),
            (48_000.0, 90.0, 96, 4000.0),
            (96_000.0, 120.0, 109, 8000.0),
        ] {
            let registry = Arc::new(BufferRegistry::new());
            registry.replace_buffer_at_rate(0, vec![1024], rate);
            let mut player = RamPlay::with_registry(rate, registry);
            player.set_tempo(tempo);
            player.trigger(
                &RamPlayParams {
                    rtrg: 127,
                    rtim: time,
                    vol: 127,
                    ..Default::default()
                },
                0,
            );
            let hits: Vec<_> = (0..(200.0 * interval) as usize)
                .filter(|_| player.process(0) == 0.5)
                .collect();
            let expected: Vec<_> = (0..200)
                .map(|hit| (hit as f64 * interval).round() as usize)
                .collect();
            assert_eq!(hits, expected, "{rate} Hz, {tempo} BPM, RTIM={time}");
        }
    }

    #[test]
    fn retrigger_tempo_change_preserves_phase_and_off_cancels_pending_hits() {
        let registry = Arc::new(BufferRegistry::new());
        registry.replace_buffer_at_rate(0, vec![1024], 1024.0);
        let mut player = RamPlay::with_registry(1024.0, registry);
        let params = RamPlayParams {
            rtrg: 127,
            rtim: 127,
            vol: 127,
            ..Default::default()
        };
        player.trigger(&params, 0);
        for frame in 0..64 {
            assert_eq!(player.process(0), if frame == 0 { 0.5 } else { 0.0 });
        }
        // Halfway through a 128-frame interval, double tempo. The remaining
        // half now takes 32 frames, not 64 and not a freshly restarted interval.
        player.set_tempo(240.0);
        for _ in 0..32 {
            assert_eq!(player.process(0), 0.0);
        }
        assert_eq!(player.process(0), 0.5);
        for off in [
            RamPlayParams { rtrg: 0, ..params },
            RamPlayParams { rtim: 0, ..params },
        ] {
            player.trigger(&params, 0);
            assert_eq!(player.process(0), 0.5);
            player.update_params(&off, 0);
            assert!(!player.is_playing(0));
            for _ in 0..256 {
                assert_eq!(player.process(0), 0.0);
            }
        }
        // Invalid host data must not poison timing, and a new note resets phase.
        for tempo in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            player.set_tempo(tempo);
            player.trigger(&params, 0);
            assert_eq!(player.process(0), 0.5);
            for _ in 1..128 {
                assert_eq!(player.process(0), 0.0);
            }
            assert_eq!(player.process(0), 0.5);
        }
    }

    #[test]
    fn retrigger_mapping_preserves_reported_landmarks_and_monotonicity() {
        for (value, per_beat) in [
            (32, 32),
            (44, 24),
            (52, 20),
            (64, 16),
            (77, 12),
            (81, 11),
            (85, 10),
            (90, 9),
            (96, 8),
            (101, 7),
            (109, 6),
            (117, 5),
            (127, 4),
        ] {
            assert!((retrig_interval_beats(value) - 1.0 / per_beat as f64).abs() < 1e-12);
        }
        assert_eq!(retrig_interval_beats(0), 0.0);
        for value in 1..=127 {
            assert!(retrig_interval_beats(value) > retrig_interval_beats(value - 1));
        }
    }

    #[test]
    fn test_multi_channel_playback() {
        let mut player = test_player();
        player.load_buffer(make_test_buffer(), 0);
        player.load_buffer(make_test_buffer(), 3);

        player.trigger(&RamPlayParams::default(), 0);
        player.trigger(&RamPlayParams::default(), 3);

        assert!(player.is_playing(0));
        assert!(!player.is_playing(1));
        assert!(player.is_playing(3));

        let sample0 = player.process(0);
        let sample3 = player.process(3);

        assert!((-1.0..=1.0).contains(&sample0));
        assert!((-1.0..=1.0).contains(&sample3));
    }

    #[test]
    fn test_cross_channel_isolation_from_shared_registry() {
        let registry = Arc::new(BufferRegistry::new());
        assert!(registry.replace_buffer(0, vec![2047; 64]));

        let mut player = RamPlay::with_registry(44100.0, registry);
        player.trigger(&RamPlayParams::default(), 1);
        let sample1 = player.process(1);
        assert!(sample1.abs() < 1.0e-6);
    }

    #[test]
    fn test_trigger_on_empty_buffer_is_deterministic_silence() {
        let registry = Arc::new(BufferRegistry::new());
        let mut player = RamPlay::with_registry(44100.0, registry);

        player.trigger(&RamPlayParams::default(), 2);

        assert!(!player.is_playing(2));
        assert!(player.process(2).abs() < 1.0e-6);
    }

    #[test]
    fn reversed_markers_play_the_selected_region_last_sample_to_first() {
        let mut player = test_player();
        for codes in [vec![512, -1024, 1536, 256], vec![-768]] {
            player.load_buffer(codes.clone(), 0);
            player.trigger(
                &RamPlayParams {
                    strt: 127,
                    end: 0,
                    vol: 127,
                    ..Default::default()
                },
                0,
            );
            for code in codes.into_iter().rev() {
                assert_eq!(player.process(0), code as f32 / 2048.0);
            }
            assert!(!player.is_playing(0));
            assert_eq!(player.process(0), 0.0);
        }

        // Exact interior boundaries: [31, 95) traversed in either direction.
        let codes: Vec<_> = (0..127).map(|i| i * 16 - 1000).collect();
        player.load_buffer(codes.clone(), 0);
        for (start, end, indices) in [
            (31, 95, (31..95).collect::<Vec<_>>()),
            (95, 31, (31..95).rev().collect()),
        ] {
            player.trigger(
                &RamPlayParams {
                    strt: start,
                    end,
                    vol: 127,
                    ..Default::default()
                },
                0,
            );
            for i in indices {
                assert_eq!(player.process(0), codes[i] as f32 / 2048.0);
            }
            assert!(!player.is_playing(0));
            assert_eq!(player.process(0), 0.0);
        }
        for point in [0, 63, 127] {
            player.trigger(
                &RamPlayParams {
                    strt: point,
                    end: point,
                    ..Default::default()
                },
                0,
            );
            assert_eq!(player.process(0), 0.0);
            assert!(!player.is_playing(0));
        }
    }

    #[test]
    fn reverse_pitch_and_sample_rate_conversion_keep_interpolation_and_duration() {
        let registry = Arc::new(BufferRegistry::new());
        assert!(registry.replace_buffer_at_rate(0, vec![-512, 1024, 1536], 44_100.0));
        for (output_rate, pitch, expected) in [
            (44_100.0, 52, vec![0.75, 0.625, 0.5, 0.125, -0.25, -0.25]),
            (88_200.0, 64, vec![0.75, 0.625, 0.5, 0.125, -0.25, -0.25]),
            (44_100.0, 76, vec![0.75, -0.25]),
        ] {
            let mut player = RamPlay::with_registry(output_rate, registry.clone());
            player.trigger(
                &RamPlayParams {
                    strt: 127,
                    end: 0,
                    pitch,
                    vol: 127,
                    ..Default::default()
                },
                0,
            );
            for sample in expected {
                assert_eq!(player.process(0), sample);
            }
            assert!(!player.is_playing(0));
            assert_eq!(player.process(0), 0.0);
        }
    }

    #[test]
    fn reverse_retriggers_restart_at_start_and_replacement_stops_held_audio() {
        let registry = Arc::new(BufferRegistry::new());
        // A 1/128 note at 120 BPM is four frames at 256 Hz.
        assert!(registry.replace_buffer_at_rate(0, vec![256, 512, 768, 1024], 256.0));
        let mut player = RamPlay::with_registry(256.0, registry.clone());
        let params = RamPlayParams {
            strt: 127,
            end: 0,
            vol: 127,
            rtrg: 1,
            rtim: 32,
            ..Default::default()
        };
        player.trigger(&params, 0);
        let rendered: Vec<_> = (0..9).map(|_| player.process(0)).collect();
        assert_eq!(
            rendered,
            [0.5, 0.375, 0.25, 0.125, 0.5, 0.375, 0.25, 0.125, 0.0]
        );
        assert!(!player.is_playing(0));

        player.trigger(
            &RamPlayParams {
                rtrg: 0,
                srr: 0.5,
                ..params
            },
            0,
        );
        assert_eq!(player.process(0), 0.0);
        assert_eq!(player.process(0), 0.375);
        assert!(registry.replace_buffer(0, vec![-1024; 4]));
        assert_eq!(player.process(0), 0.0);
        assert!(
            !player.is_playing(0),
            "invalidated reverse voice must stop, not hold or loop"
        );
    }

    #[test]
    fn test_minimal_one_shot_reaches_end_and_stops() {
        let registry = Arc::new(BufferRegistry::new());
        let mut player = RamPlay::with_registry(44100.0, registry);
        player.load_buffer(vec![2047; 64], 0);

        let params = RamPlayParams {
            strt: 0,
            end: 127,
            pitch: 64,
            hold: 0,
            dec: 0,
            rtrg: 0,
            rtim: 0,
            srr: 0.0,
            brr: 0,
            dist: 0,
            vol: 127,
        };
        player.trigger(&params, 0);
        let mut produced_non_zero = false;
        for _ in 0..200 {
            let sample = player.process(0);
            if sample.abs() > 1.0e-6 {
                produced_non_zero = true;
            }
            if !player.is_playing(0) {
                break;
            }
        }

        assert!(produced_non_zero);
        assert!(!player.is_playing(0));
        assert!(player.process(0).abs() < 1.0e-6);
    }
}
