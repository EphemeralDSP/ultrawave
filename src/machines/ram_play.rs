use crate::dsp::effects::Distortion;
use crate::dsp::sample_engine::{param_to_normalized, pitch_to_ratio, SampleEngine};
use crate::machines::memory::SampleMemory;
use std::sync::Arc;

/// Total hits in a fresh MD RAM-P trigger; None is the continuous endpoint.
/// X.08/1.63 MAIN 0x204abc..0x204af8, confirmed by hardware captures.
pub fn retrig_total_hits(value: i32) -> Option<usize> {
    match value {
        127 => None,
        0 | 1 => Some(1),
        2..=63 => Some(value as usize),
        _ => Some((value * value / 128 + 32) as usize),
    }
}

/// Canonical MD interval, from MAIN 0x204af8..0x204b96 at direct Q7 inputs.
/// The analytic logarithm reproduces all 128 original table entries exactly.
/// The DSP allows one restart per 32-frame render (four timer units per frame).
/// Match that measured steady rate; do not emulate its startup/block jitter or
/// history-dependent CPU smoothing. See docs/testing/machinedrum-measurement.md.
pub fn retrig_interval_seconds(value: i32, tempo_bpm: f64) -> f64 {
    let tempo = tempo_bpm * 24.0;
    let units = if value < 64 {
        let scale = ((270_950_400.0 / tempo).floor() * 60.0 / 4096.0).floor();
        (scale * f64::from(value * 128) / 2048.0).floor()
    } else {
        let step = (value * 512 / 127) as u32;
        let table = (2048.0 * 2.0_f64.powf(f64::from(step & 127) / 127.5)).round() as u32;
        let scaled = table >> (5 - (step >> 7));
        ((f64::from(scaled) * 1_058_400.0 / tempo).floor() * 60.0 / 1024.0).floor()
    };
    units.max(128.0) / 176_400.0
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
    memory: Arc<SampleMemory>,
    engine: SampleEngine,
    envelope: f32,
    is_playing: bool,
    hold_time_samples: usize,
    hold_counter: usize,
    decay_rate: f32,
    sample_rate: f32,
    tempo_bpm: f64,
    retrig_counter: f64,
    retrig_interval: f64,
    retrigs_remaining: usize,
    current_params: RamPlayParams,
    distortion: Distortion,
    playback_ratio: f64,
    reverse: bool,
}

#[allow(dead_code)]
impl RamPlay {
    pub fn new(sample_rate: f32, memory: Arc<SampleMemory>) -> Self {
        Self {
            memory,
            engine: SampleEngine::new(),
            envelope: 0.0,
            is_playing: false,
            hold_time_samples: 0,
            hold_counter: 0,
            decay_rate: 0.0,
            sample_rate,
            tempo_bpm: 120.0,
            retrig_counter: 0.0,
            retrig_interval: 0.0,
            retrigs_remaining: 0,
            current_params: RamPlayParams::default(),
            distortion: Distortion::new(),
            playback_ratio: 1.0,
            reverse: false,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
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
            let interval = (retrig_interval_seconds(self.current_params.rtim, tempo_bpm)
                * self.sample_rate as f64)
                .max(1.0);
            if self.retrig_interval > 0.0 {
                self.retrig_counter *= interval / self.retrig_interval;
            }
            self.retrig_interval = interval;
            self.tempo_bpm = tempo_bpm;
        }
    }

    pub fn load_buffer(&mut self, samples: Vec<i16>) {
        let _ = self.memory.replace_buffer(samples);
    }

    pub fn trigger(&mut self, params: &RamPlayParams) {
        self.current_params = *params;
        self.update_params(params);
        let source_rate = self.engine.latch_buffer(Arc::clone(&self.memory));
        if source_rate.is_none() {
            self.is_playing = false;
            self.retrigs_remaining = 0;
            return;
        }
        self.playback_ratio =
            pitch_to_ratio(params.pitch) * source_rate.unwrap() as f64 / self.sample_rate as f64;

        self.reverse = params.strt > params.end;
        self.trigger_internal();

        self.retrigs_remaining = retrig_total_hits(params.rtrg).map_or(usize::MAX, |n| n - 1);
        self.retrig_interval = (retrig_interval_seconds(params.rtim, self.tempo_bpm)
            * self.sample_rate as f64)
            .max(1.0);
        self.retrig_counter = 0.0;
    }

    /// Live playback controls. Start, direction, envelope and retrigger timing
    /// are latched on trigger; End remains live within the selected direction.
    pub fn update_params(&mut self, params: &RamPlayParams) {
        // Both single-hit settings cancel pending repeats, even an infinite run.
        // Other count/time changes latch on the next note; RTIM=0 is not OFF.
        if params.rtrg <= 1 {
            self.retrigs_remaining = 0;
        }
        let current = &mut self.current_params;
        current.pitch = params.pitch;
        current.end = params.end;
        current.srr = params.srr;
        current.brr = params.brr;
        current.dist = params.dist;
        current.vol = params.vol;
        self.distortion.set_drive(param_to_normalized(params.dist));
        self.playback_ratio = pitch_to_ratio(params.pitch);
        if let Some((_, source_rate, _)) = self.memory.snapshot() {
            self.playback_ratio *= source_rate as f64 / self.sample_rate as f64;
        }
    }

    fn trigger_internal(&mut self) {
        let buffer_len = self.engine.buffer_len() as f64;
        let start = self.current_params.strt;
        // Multiply before dividing so exact sample boundaries stay exact.
        let start_pos = start as f64 * buffer_len / 127.0;
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
            if !self.engine.source_is_current() {
                self.stop();
                return 0.0;
            }
            // Check elapsed frames BEFORE counting this frame. Carry fractional
            // overshoot so non-integral intervals never accumulate rounding drift.
            if self.retrig_counter + 0.5 > self.retrig_interval {
                self.retrig_counter -= self.retrig_interval;
                if self.retrigs_remaining != usize::MAX {
                    self.retrigs_remaining -= 1;
                }
                self.trigger_internal();
            }
            self.retrig_counter += 1.0;
        }

        if !self.is_playing {
            return 0.0;
        }

        let buffer_len = self.engine.buffer_len();
        let end_pos = self.current_params.end as f64 * buffer_len as f64 / 127.0;
        let ratio = if self.reverse {
            -self.playback_ratio
        } else {
            self.playback_ratio
        };

        if self.engine.is_finished(end_pos, ratio) {
            self.is_playing = false;
            return 0.0;
        }

        let sample = self.engine.read_sample_with_processing(
            ratio,
            self.current_params.srr,
            self.current_params.brr,
        );
        let sample = self.distortion.process(sample);
        if self.engine.is_finished(end_pos, ratio) {
            self.is_playing = false;
        }

        let is_minimal_one_shot = self.current_params.hold == 0 && self.current_params.dec == 0;
        if !is_minimal_one_shot {
            if self.hold_counter < self.hold_time_samples {
                self.hold_counter += 1;
            } else {
                self.envelope = (self.envelope - self.decay_rate).max(0.0);
                if self.envelope <= 0.0 {
                    self.is_playing = false;
                }
            }
        }

        let vol = param_to_normalized(self.current_params.vol);
        sample * self.envelope * vol
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing || self.retrigs_remaining > 0
    }

    /// The currently audible voice, excluding silent waits between retriggers.
    pub fn playback_position(&self) -> Option<usize> {
        if !self.is_playing {
            return None;
        }
        Some(self.engine.playback_position())
    }

    pub fn buffer_len(&self) -> usize {
        self.memory.buffer_len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machines::memory::SampleMemory;
    use std::sync::Arc;

    fn make_test_buffer() -> Vec<i16> {
        (0..1000)
            .map(|i| (i as f32 / 1000.0 * 2047.0) as i16)
            .collect()
    }

    fn test_player() -> RamPlay {
        RamPlay::new(44100.0, Arc::new(SampleMemory::new()))
    }

    #[test]
    fn test_trigger_starts_playback() {
        let mut player = test_player();
        player.load_buffer(make_test_buffer());
        player.trigger(&RamPlayParams::default());
        assert!(player.is_playing());
    }

    #[test]
    fn test_stop_halts_playback() {
        let mut player = test_player();
        player.load_buffer(make_test_buffer());
        player.trigger(&RamPlayParams::default());
        player.stop();
        assert!(!player.is_playing());
    }

    #[test]
    fn test_process_outputs_audio() {
        let mut player = test_player();
        player.load_buffer(make_test_buffer());
        player.trigger(&RamPlayParams::default());
        let sample = player.process();
        assert!((-1.0..=1.0).contains(&sample));
    }

    #[test]
    fn retrigger_timeline_has_exact_count_off_and_infinite_boundaries() {
        // At 120 BPM, sixteenth notes are 125 ms = 128 frames at 1024 Hz.
        // Fresh RAM-P1 hardware captures establish TOTAL hits, not extra repeats.
        // A finite count above 127 must not accidentally become the INF sentinel.
        let registry = Arc::new(SampleMemory::new());
        registry.replace_buffer_at_rate(vec![1024], 1024.0);
        for (raw, total) in [(0, 1), (1, 1), (2, 2), (96, 104), (126, 156), (127, 160)] {
            let mut player = RamPlay::new(1024.0, registry.clone());
            player.trigger(&RamPlayParams {
                rtrg: raw,
                rtim: 127,
                vol: 127,
                ..Default::default()
            });
            for frame in 0..(160 * 128) {
                let hit = frame % 128 == 0 && frame / 128 < total;
                assert_eq!(
                    player.process(),
                    if hit { 0.5 } else { 0.0 },
                    "RTRG={raw} frame={frame}"
                );
            }
            assert_eq!(player.is_playing(), raw == 127);
            player.stop();
            assert_eq!(player.process(), 0.0);
            assert!(!player.is_playing());
        }
    }

    #[test]
    fn minimum_interval_repeats_and_stays_at_the_hardware_rate_limit() {
        // The hardware's steady fastest train is 44100/32 Hz, not OFF.
        // Tempo changes cannot accelerate that floor. Changing RTRG to 1 stops it.
        let memory = Arc::new(SampleMemory::new());
        memory.replace_buffer_at_rate(vec![1024], 44100.0);
        let mut player = RamPlay::new(44100.0, memory);
        let params = RamPlayParams {
            rtrg: 127,
            rtim: 0,
            vol: 127,
            ..Default::default()
        };
        player.trigger(&params);
        for frame in 0..160 {
            if frame == 48 {
                player.set_tempo(240.0);
            }
            assert_eq!(
                player.process(),
                if frame % 32 == 0 { 0.5 } else { 0.0 },
                "frame {frame}"
            );
        }
        player.update_params(&RamPlayParams { rtrg: 1, ..params });
        for _ in 0..128 {
            assert_eq!(player.process(), 0.0);
        }
        assert!(!player.is_playing());
    }

    #[test]
    fn retrigger_cancels_on_buffer_replacement_even_between_hits() {
        for reverse in [false, true] {
            for frames_before_replace in [1, 8] {
                let registry = Arc::new(SampleMemory::new());
                registry.replace_buffer_at_rate(vec![1024; 4], 1024.0);
                let mut player = RamPlay::new(1024.0, registry.clone());
                player.trigger(&RamPlayParams {
                    strt: if reverse { 127 } else { 0 },
                    end: if reverse { 0 } else { 127 },
                    rtrg: 127,
                    rtim: 127,
                    vol: 127,
                    ..Default::default()
                });
                for _ in 0..frames_before_replace {
                    player.process();
                }
                registry.replace_buffer(vec![-1024; 4]);
                assert_eq!(player.process(), 0.0);
                assert!(
                    !player.is_playing(),
                    "dead source must cancel infinite repeats"
                );
                for _ in 0..300 {
                    assert_eq!(player.process(), 0.0);
                }
            }
        }
    }

    #[test]
    fn retrigger_fractional_intervals_do_not_drift_at_different_tempos_and_rates() {
        // Original formatter operands at these tempos, in four-per-44.1k-frame
        // timer units: 22050, 14929, 14857. No expectation uses the Rust mapping.
        for (rate, tempo, time, interval) in [
            (44_100.0, 120.0, 127, 5512.5),
            (48_000.0, 90.0, 96, 14929.0 * 48000.0 / 176400.0),
            (96_000.0, 120.0, 109, 14857.0 * 96000.0 / 176400.0),
        ] {
            let registry = Arc::new(SampleMemory::new());
            registry.replace_buffer_at_rate(vec![1024], rate);
            let mut player = RamPlay::new(rate, registry);
            player.set_tempo(tempo);
            player.trigger(&RamPlayParams {
                rtrg: 127,
                rtim: time,
                vol: 127,
                ..Default::default()
            });
            let hits: Vec<_> = (0..(200.0 * interval) as usize)
                .filter(|_| player.process() == 0.5)
                .collect();
            let expected: Vec<_> = (0..200)
                .map(|hit| (hit as f64 * interval).round() as usize)
                .collect();
            assert_eq!(hits, expected, "{rate} Hz, {tempo} BPM, RTIM={time}");
        }
    }

    #[test]
    fn retrigger_tempo_change_preserves_phase_and_off_cancels_pending_hits() {
        let registry = Arc::new(SampleMemory::new());
        registry.replace_buffer_at_rate(vec![1024], 1024.0);
        let mut player = RamPlay::new(1024.0, registry);
        let params = RamPlayParams {
            rtrg: 127,
            rtim: 127,
            vol: 127,
            ..Default::default()
        };
        player.trigger(&params);
        for frame in 0..64 {
            assert_eq!(player.process(), if frame == 0 { 0.5 } else { 0.0 });
        }
        // Halfway through a 128-frame interval, double tempo. The remaining
        // half now takes 32 frames, not 64 and not a freshly restarted interval.
        player.set_tempo(240.0);
        for _ in 0..32 {
            assert_eq!(player.process(), 0.0);
        }
        assert_eq!(player.process(), 0.5);
        for off in [
            RamPlayParams { rtrg: 0, ..params },
            RamPlayParams { rtrg: 1, ..params },
        ] {
            player.trigger(&params);
            assert_eq!(player.process(), 0.5);
            player.update_params(&off);
            assert!(!player.is_playing());
            for _ in 0..256 {
                assert_eq!(player.process(), 0.0);
            }
        }
        // Invalid host data must not poison timing, and a new note resets phase.
        for tempo in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            player.set_tempo(tempo);
            player.trigger(&params);
            assert_eq!(player.process(), 0.5);
            for _ in 1..128 {
                assert_eq!(player.process(), 0.0);
            }
            assert_eq!(player.process(), 0.5);
        }
    }

    #[test]
    fn retrigger_mapping_matches_firmware_operands_and_measured_rate_floor() {
        for (tempo, value, ticks) in [
            (120.0, 0, 128),
            (120.0, 1, 128),
            (120.0, 2, 172),
            (120.0, 32, 2756),
            (120.0, 63, 5425),
            (120.0, 64, 5555),
            (150.0, 64, 4444),
            (120.0, 127, 22050),
        ] {
            assert!(
                (retrig_interval_seconds(value, tempo) - ticks as f64 / 176400.0).abs() < 1e-12
            );
        }
        for tempo in [60.0, 120.0, 150.0, 300.0] {
            for value in 1..=127 {
                assert!(
                    retrig_interval_seconds(value, tempo)
                        >= retrig_interval_seconds(value - 1, tempo)
                );
            }
        }
    }

    #[test]
    fn test_trigger_on_empty_buffer_is_deterministic_silence() {
        let registry = Arc::new(SampleMemory::new());
        let mut player = RamPlay::new(44100.0, registry);

        player.trigger(&RamPlayParams::default());

        assert!(!player.is_playing());
        assert!(player.process().abs() < 1.0e-6);
    }

    #[test]
    fn reversed_markers_play_the_selected_region_last_sample_to_first() {
        let mut player = test_player();
        for codes in [vec![512, -1024, 1536, 256], vec![-768]] {
            player.load_buffer(codes.clone());
            player.trigger(&RamPlayParams {
                strt: 127,
                end: 0,
                vol: 127,
                ..Default::default()
            });
            for code in codes.into_iter().rev() {
                assert_eq!(player.process(), code as f32 / 2048.0);
            }
            assert!(!player.is_playing());
            assert_eq!(player.process(), 0.0);
        }

        // Exact interior boundaries: [31, 95) traversed in either direction.
        let codes: Vec<_> = (0..127).map(|i| i * 16 - 1000).collect();
        player.load_buffer(codes.clone());
        for (start, end, indices) in [
            (31, 95, (31..95).collect::<Vec<_>>()),
            (95, 31, (31..95).rev().collect()),
        ] {
            player.trigger(&RamPlayParams {
                strt: start,
                end,
                vol: 127,
                ..Default::default()
            });
            for i in indices {
                assert_eq!(player.process(), codes[i] as f32 / 2048.0);
            }
            assert!(!player.is_playing());
            assert_eq!(player.process(), 0.0);
        }
        for point in [0, 63, 127] {
            player.trigger(&RamPlayParams {
                strt: point,
                end: point,
                ..Default::default()
            });
            assert_eq!(player.process(), 0.0);
            assert!(!player.is_playing());
        }
    }

    #[test]
    fn reverse_pitch_and_sample_rate_conversion_keep_interpolation_and_duration() {
        let registry = Arc::new(SampleMemory::new());
        assert!(registry.replace_buffer_at_rate(vec![-512, 1024, 1536], 44_100.0));
        for (output_rate, pitch, expected) in [
            (44_100.0, 52, vec![0.75, 0.625, 0.5, 0.125, -0.25, -0.25]),
            (88_200.0, 64, vec![0.75, 0.625, 0.5, 0.125, -0.25, -0.25]),
            (44_100.0, 76, vec![0.75, -0.25]),
        ] {
            let mut player = RamPlay::new(output_rate, registry.clone());
            player.trigger(&RamPlayParams {
                strt: 127,
                end: 0,
                pitch,
                vol: 127,
                ..Default::default()
            });
            for sample in expected {
                assert_eq!(player.process(), sample);
            }
            assert!(!player.is_playing());
            assert_eq!(player.process(), 0.0);
        }
    }

    #[test]
    fn crossing_end_does_not_reverse_an_existing_retrigger_train() {
        let memory = Arc::new(SampleMemory::new());
        // At 1024 Hz and 120 BPM, RTIM=127 repeats every 128 frames.
        let codes: Vec<i16> = (0..127).map(|i| i * 8 + 256).collect();
        assert!(memory.replace_buffer_at_rate(codes.clone(), 1024.0));
        for (end, crossed_end, first_index, new_index) in [(95, 31, 63, 62), (31, 95, 62, 63)] {
            let mut player = RamPlay::new(1024.0, memory.clone());
            let params = RamPlayParams {
                strt: 63,
                end,
                rtrg: 3,
                rtim: 127,
                vol: 127,
                ..Default::default()
            };
            player.trigger(&params);
            assert_eq!(player.process(), codes[first_index] as f32 / 2048.0);
            let crossed = RamPlayParams {
                end: crossed_end,
                ..params
            };
            player.update_params(&crossed);
            for frame in 1..384 {
                assert_eq!(player.process(), 0.0, "end={end}, frame={frame}");
            }
            assert!(!player.is_playing());
            // A fresh external trigger, unlike a repeat, latches the new direction.
            player.trigger(&crossed);
            assert_eq!(player.process(), codes[new_index] as f32 / 2048.0);
        }
    }

    #[test]
    fn reverse_retriggers_restart_at_start_and_replacement_stops_held_audio() {
        let registry = Arc::new(SampleMemory::new());
        // RTIM=32 rounds to four frames at this deliberately low output rate.
        assert!(registry.replace_buffer_at_rate(vec![256, 512, 768, 1024], 256.0));
        let mut player = RamPlay::new(256.0, registry.clone());
        let params = RamPlayParams {
            strt: 127,
            end: 0,
            vol: 127,
            rtrg: 2,
            rtim: 32,
            ..Default::default()
        };
        player.trigger(&params);
        let rendered: Vec<_> = (0..9).map(|_| player.process()).collect();
        assert_eq!(
            rendered,
            [0.5, 0.375, 0.25, 0.125, 0.5, 0.375, 0.25, 0.125, 0.0]
        );
        assert!(!player.is_playing());

        player.trigger(&RamPlayParams {
            rtrg: 0,
            srr: 0.5,
            ..params
        });
        assert_eq!(player.process(), 0.0);
        assert_eq!(player.process(), 0.375);
        assert!(registry.replace_buffer(vec![-1024; 4]));
        assert_eq!(player.process(), 0.0);
        assert!(
            !player.is_playing(),
            "invalidated reverse voice must stop, not hold or loop"
        );
    }

    #[test]
    fn test_minimal_one_shot_reaches_end_and_stops() {
        let registry = Arc::new(SampleMemory::new());
        let mut player = RamPlay::new(44100.0, registry);
        player.load_buffer(vec![2047; 64]);

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
        player.trigger(&params);
        let mut produced_non_zero = false;
        for _ in 0..200 {
            let sample = player.process();
            if sample.abs() > 1.0e-6 {
                produced_non_zero = true;
            }
            if !player.is_playing() {
                break;
            }
        }

        assert!(produced_non_zero);
        assert!(!player.is_playing());
        assert!(player.process().abs() < 1.0e-6);
    }
}
