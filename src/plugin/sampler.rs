use nice_plug::prelude::*;
use std::sync::{atomic::Ordering, Arc};

use super::{editor, UiState};
use crate::dsp::{effects::Distortion, mix::SmoothedGains};
use crate::machines::{
    memory::SampleMemory,
    ram_play::{RamPlay, RamPlayParams},
    ram_record::{RamRecord, RamRecordParams},
};
use crate::params::UltrawaveParams;

/// A MIDI-controlled insert effect. RAM belongs to this instance, never the host
/// process or project state. The editor only reads atomic memory snapshots.
pub struct UltrawavePlugin {
    params: Arc<UltrawaveParams>,
    editor_state: Arc<nice_plug_egui::EguiEditorState>,
    ui: Arc<UiState>,
    memory: Arc<SampleMemory>,
    recorder: RamRecord,
    player: RamPlay,
    record_channel: usize,
    play_channel: usize,
    sample_rate: f32,
    velocity: f32,
    capture_was_high: bool,
    playback_was_high: bool,
    rearm_was_high: bool,
    one_shot_spent: bool,
    transport_was_playing: bool,
    decimator: crate::Decimator,
    distortion: Distortion,
    mix_gains: SmoothedGains<4>,
    // Capture level, playback level, stereo dry-through.
    gains: SmoothedGains<3>,
    preload_attempted: bool,
}

impl Default for UltrawavePlugin {
    fn default() -> Self {
        let sample_rate = 44100.0;
        let memory = Arc::new(SampleMemory::new());
        Self {
            params: Arc::new(UltrawaveParams::default()),
            editor_state: editor::default_state(),
            ui: Arc::new(UiState::default()),
            recorder: RamRecord::new(sample_rate, memory.clone()),
            player: RamPlay::new(sample_rate, memory.clone()),
            memory,
            record_channel: 0,
            play_channel: 0,
            sample_rate,
            velocity: 1.0,
            capture_was_high: false,
            playback_was_high: false,
            rearm_was_high: false,
            one_shot_spent: false,
            transport_was_playing: false,
            decimator: crate::Decimator::new(),
            distortion: Distortion::new(),
            mix_gains: SmoothedGains::default(),
            gains: SmoothedGains::default(),
            preload_attempted: false,
        }
    }
}

impl UltrawavePlugin {
    fn capture(&mut self, params: &RamRecordParams) {
        if self.params.record.one_shot.value() && self.one_shot_spent {
            return;
        }
        self.recorder.stop_all_recording();
        self.record_channel = self.params.buffer.value() as usize;
        self.decimator = crate::Decimator::new();
        self.recorder
            .start_recording(params, true, self.record_channel);
        if self.params.record.one_shot.value() {
            self.one_shot_spent = true;
        }
    }

    fn play(&mut self, params: &RamPlayParams, velocity: f32) {
        self.player.stop_all();
        self.play_channel = self.params.buffer.value() as usize;
        self.velocity = velocity;
        self.player.trigger(params, self.play_channel);
    }
}

impl Plugin for UltrawavePlugin {
    const NAME: &'static str = "Ultrawave";
    const VENDOR: &'static str = "EphemeralDSP";
    const URL: &'static str = "https://github.com/EphemeralDSP/ultrawave";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[NonZeroU32::new(2).unwrap()],
        aux_output_ports: &[],
        names: PortNames::const_default(),
    }];
    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;
    type SysExMessage = ();
    type BackgroundTask = ();
    type Editor = editor::PluginEditor;

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        if crate::NO_GUI.load(Ordering::Relaxed) {
            return None;
        }
        editor::create(
            self.params.clone(),
            self.editor_state.clone(),
            self.ui.clone(),
            self.memory.clone(),
        )
    }

    fn activate(
        &mut self,
        _: &AudioIOLayout,
        config: &BufferConfig,
        context: &mut impl ActivateContext<Self>,
    ) -> bool {
        if context.plugin_api() == PluginApi::Standalone && !self.preload_attempted {
            if let Err(error) = super::maybe_preload_buffer_from_env(&self.memory) {
                eprintln!("standalone preload failed: {error}");
            }
            self.preload_attempted = true;
        }
        self.sample_rate = config.sample_rate;
        self.recorder.set_sample_rate(config.sample_rate);
        self.player.set_sample_rate(config.sample_rate);
        self.reset();
        true
    }

    fn reset(&mut self) {
        // Host transport resets stop voices/takes, not RAM or One Shot protection.
        self.recorder.stop_all_recording();
        self.player.stop_all();
        // Loading a saved high command or resetting while it is held is not a
        // new rising edge. Automation must go low then high to trigger again.
        self.capture_was_high = self.params.record.capture.value();
        self.playback_was_high = self.params.playback.value();
        self.rearm_was_high = self.params.rearm.value();
        self.transport_was_playing = false;
        self.decimator = crate::Decimator::new();
        self.mix_gains.reset();
        self.gains.reset();
        for flag in [
            &self.ui.recording,
            &self.ui.playing,
            &self.ui.record_trigger,
            &self.ui.play_trigger,
            &self.ui.rearm,
        ] {
            flag.store(false, Ordering::Relaxed);
        }
        self.ui.capture_peak.store(0, Ordering::Relaxed);
        self.ui.playback_peak.store(0, Ordering::Relaxed);
        self.ui.clear_playback();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let transport = context.transport();
        let tempo = transport.tempo.unwrap_or(120.0);
        self.player.set_tempo(tempo);
        if self.transport_was_playing && !transport.playing {
            self.player.stop_all();
            self.recorder.stop_all_recording();
        }
        self.transport_was_playing = transport.playing;
        if !self.params.record.one_shot.value() {
            self.one_shot_spent = false;
        } else if self.recorder.is_any_recording() {
            self.one_shot_spent = true;
        }
        let rearm = self.params.rearm.value();
        if (self.ui.rearm.swap(false, Ordering::Relaxed) || (rearm && !self.rearm_was_high))
            && !self.recorder.is_any_recording()
        {
            self.one_shot_spent = false;
        }
        self.rearm_was_high = rearm;
        let p = &self.params.record;
        let record = RamRecordParams {
            mlev: p.mlev.value(),
            mbal: p.mbal.value(),
            ilev: p.ilev.value(),
            ibal: p.ibal.value(),
            rec_len: p.rec_len.value(),
            rec_rate: p.rec_rate.value(),
            tempo_bpm: tempo,
        };
        let p = &self.params.play;
        let play = RamPlayParams {
            strt: p.strt.value(),
            end: p.end.value(),
            pitch: p.pitch.value(),
            hold: p.hold.value(),
            dec: p.dec.value(),
            rtrg: p.rtrg.value(),
            rtim: p.rtim.value(),
            srr: p.srr.value(),
            brr: p.brr.value(),
            dist: p.dist.value(),
            vol: 127,
        };
        self.player.update_params(&play, self.play_channel);
        let capture_high = self.params.record.capture.value();
        if self.ui.record_trigger.swap(false, Ordering::Relaxed)
            || (capture_high && !self.capture_was_high)
        {
            self.capture(&record);
        }
        self.capture_was_high = capture_high;
        let playback_high = self.params.playback.value();
        if self.ui.play_trigger.swap(false, Ordering::Relaxed)
            || (playback_high && !self.playback_was_high)
        {
            self.play(&play, 1.0);
        }
        self.playback_was_high = playback_high;
        self.mix_gains.set(record.input_gains(), self.sample_rate);
        self.gains.set(
            [
                util::db_to_gain(self.params.record.lev.value()),
                util::db_to_gain(self.params.play.lev.value()),
                if self.params.playback_only.value() {
                    0.0
                } else {
                    1.0
                },
            ],
            self.sample_rate,
        );
        self.distortion
            .set_drive(self.params.record.dist.value() as f32 / 127.0);
        let srr = self.params.record.srr.value();
        let external = aux.inputs.first_mut().map(|input| input.as_slice());
        let frames = buffer.samples();
        let main = buffer.as_slice();
        let mut mixed = [0.0; 64];
        let silence = [0.0; 64];
        let mut capture_peak = 0.0f32;
        let mut playback_peak = 0.0f32;
        let mut event = context.next_event();
        for start in (0..frames).step_by(mixed.len()) {
            let end = (start + mixed.len()).min(frames);
            let mono = &mut mixed[..end - start];
            let silent = &silence[..mono.len()];
            let left = main.first().map_or(silent, |s| &s[start..end]);
            let right = main.get(1).map_or(left, |s| &s[start..end]);
            let side = |channel| {
                external
                    .as_ref()
                    .and_then(|bus| bus.get(channel))
                    .and_then(|s: &&mut [f32]| s.get(start..end))
                    .unwrap_or(silent)
            };
            // SIMD source mixing reads input before playback touches the output.
            self.mix_gains.mix([left, right, side(0), side(1)], mono);
            for (offset, &input) in mono.iter().enumerate() {
                let frame = start + offset;
                while let Some(next) = event {
                    if next.timing() > frame as u32 {
                        break;
                    }
                    if let NoteEvent::NoteOn { key, velocity, .. } = next {
                        if velocity > 0.0 {
                            // Equal assignments deliberately give Record priority.
                            if key.number() == Some(self.params.record_note.value() as u8) {
                                self.capture(&record);
                            } else if key.number() == Some(self.params.play_note.value() as u8) {
                                self.play(&play, velocity);
                            }
                        }
                    }
                    event = context.next_event();
                }
                let [record_gain, play_gain, dry_gain] = self.gains.next_frame();
                let captured = self
                    .distortion
                    .process(self.decimator.process(input * record_gain, srr));
                self.recorder.record_mono(captured, self.record_channel);
                let played = self.player.process(self.play_channel) * self.velocity * play_gain;
                capture_peak = capture_peak.max(captured.abs());
                playback_peak = playback_peak.max(played.abs());
                for channel in main.iter_mut() {
                    channel[frame] = channel[frame] * dry_gain + played;
                }
            }
        }
        self.ui
            .recording
            .store(self.recorder.is_any_recording(), Ordering::Relaxed);
        self.ui
            .playing
            .store(self.player.is_playing(self.play_channel), Ordering::Relaxed);
        self.ui
            .capture_locked
            .store(self.one_shot_spent, Ordering::Relaxed);
        self.ui
            .capture_peak
            .fetch_max(capture_peak.to_bits(), Ordering::Relaxed);
        self.ui
            .playback_peak
            .fetch_max(playback_peak.to_bits(), Ordering::Relaxed);
        if let Some(position) = self.player.playback_position(self.play_channel) {
            self.ui.publish_playback(self.play_channel, position);
        } else {
            self.ui.clear_playback();
        }
        ProcessStatus::Normal
    }
}

impl ClapPlugin for UltrawavePlugin {
    const CLAP_ID: &'static str = "com.ephemeraldsp.ultrawave";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("MIDI-controlled 12-bit capture and playback effect");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Sampler];
}

impl Vst3Plugin for UltrawavePlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"EphemUltrawaveFx";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx];
}
