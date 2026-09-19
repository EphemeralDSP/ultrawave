use nice_plug::prelude::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::dsp::effects::Distortion;
use crate::dsp::mix::mix_inputs;
use crate::machines::ram_record::{RamRecord, RamRecordParams as RamRecordMachineParams};
use crate::params::RamRecordParams;
use crate::plugin::{
    editor::{self, EditorParams},
    pairing::Pairing,
    UiState,
};

pub struct RecordPlugin {
    params: Arc<RamRecordParams>,
    editor_state: Arc<nice_plug_egui::EguiEditorState>,
    sample_rate: f32,
    ram_record: RamRecord,
    active_channel: usize,
    ui: Arc<UiState>,
    capture_was_high: bool,
    one_shot_spent: bool,
    decimator: crate::Decimator,
    distortion: Distortion,
    pairing: Pairing,
    preload_attempted: bool,
}

impl Default for RecordPlugin {
    fn default() -> Self {
        let sample_rate = 44100.0;
        let pairing = Pairing::new();
        Self {
            params: Arc::new(RamRecordParams::default()),
            editor_state: editor::default_state(),
            sample_rate,
            ram_record: RamRecord::with_registry(sample_rate, pairing.current_registry()),
            active_channel: 0,
            ui: Arc::new(UiState::default()),
            capture_was_high: false,
            one_shot_spent: false,
            decimator: crate::Decimator::new(),
            distortion: Distortion::new(),
            pairing,
            preload_attempted: false,
        }
    }
}

impl Plugin for RecordPlugin {
    const NAME: &'static str = "Ultrawave-Record";
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

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        if crate::NO_GUI.load(Ordering::Relaxed) {
            return None;
        }
        editor::create(
            EditorParams::Record(self.params.clone()),
            self.editor_state.clone(),
            self.ui.clone(),
            self.pairing.ui_registry(),
        )
    }

    fn activate(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl ActivateContext<Self>,
    ) -> bool {
        self.pairing
            .prepare_initial(self.params.bank.value() as u8, |registry| {
                self.ram_record.rebind_registry(registry)
            });
        if !self.preload_attempted {
            if let Err(error) =
                crate::plugin::maybe_preload_buffer_from_env(&self.pairing.current_registry())
            {
                eprintln!("standalone preload failed: {error}");
            }
            self.preload_attempted = true;
        }

        self.sample_rate = buffer_config.sample_rate;
        self.ram_record.set_sample_rate(buffer_config.sample_rate);
        self.reset();
        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let tempo_bpm = context.transport().tempo.unwrap_or(120.0);
        self.process_audio(buffer, aux, context, tempo_bpm)
    }

    fn reset(&mut self) {
        self.ram_record.stop_all_recording();
        self.capture_was_high = false;
        self.decimator = crate::Decimator::new();
        self.ui.active.store(false, Ordering::Relaxed);
        self.ui.peak.store(0, Ordering::Relaxed);
        self.ui.trigger.store(false, Ordering::Relaxed);
    }
}

impl RecordPlugin {
    fn start_capture(&mut self, params: &RamRecordMachineParams) {
        if self.params.one_shot.value() && self.one_shot_spent {
            return;
        }
        self.ram_record.stop_all_recording();
        self.active_channel = self.params.buffer.value() as usize;
        self.decimator = crate::Decimator::new();
        self.ram_record
            .start_recording(params, true, self.active_channel);
        if self.params.one_shot.value() && self.ram_record.is_recording(self.active_channel) {
            self.one_shot_spent = true;
        }
    }

    fn process_audio(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
        tempo_bpm: f64,
    ) -> ProcessStatus {
        let bank = self.params.bank.value() as u8;
        if self.pairing.request(bank) {
            self.ram_record.stop_all_recording();
            self.capture_was_high = self.params.capture.value();
        }
        self.pairing
            .adopt(|registry| self.ram_record.rebind_registry(registry));
        self.ui
            .bank_status
            .store(self.pairing.status() as u32, Ordering::Relaxed);
        if matches!(
            self.pairing.status(),
            crate::plugin::pairing::CONNECTING | crate::plugin::pairing::FAILED
        ) {
            self.ui.trigger.store(false, Ordering::Relaxed);
            self.capture_was_high = self.params.capture.value();
            for channel in buffer.as_slice() {
                channel.fill(0.0);
            }
            self.ui.publish(false, 0.0);
            return ProcessStatus::Normal;
        }
        if !self.params.one_shot.value() {
            self.one_shot_spent = false;
        } else if self.ram_record.is_any_recording() {
            // Enabling One Shot mid-take protects that take, not the next one.
            self.one_shot_spent = true;
        }
        // Re-arming preserves the buffer and waits for the next trigger.
        // Ignore requests during a take instead of truncating or queuing one.
        if self.ui.rearm.swap(false, Ordering::Relaxed) && !self.ram_record.is_any_recording() {
            self.one_shot_spent = false;
        }
        let machine_params = RamRecordMachineParams {
            mlev: self.params.mlev.value(),
            mbal: self.params.mbal.value(),
            ilev: self.params.ilev.value(),
            ibal: self.params.ibal.value(),
            rec_len: self.params.rec_len.value(),
            rec_rate: self.params.rec_rate.value(),
            tempo_bpm,
        };
        let capture_high = self.params.capture.value();
        if self.ui.trigger.swap(false, Ordering::Relaxed)
            || (capture_high && !self.capture_was_high)
        {
            self.start_capture(&machine_params);
        }
        self.capture_was_high = capture_high;
        let gains = machine_params.input_gains();
        let input_gain = util::db_to_gain(self.params.lev.value());
        self.distortion
            .set_drive(self.params.dist.value() as f32 / 127.0);
        let srr = self.params.srr.value();
        let cue1 = self.params.cue1.value() as f32 / 127.0;
        let cue2 = self.params.cue2.value() as f32 / 127.0;
        let external = aux.inputs.first_mut().map(|input| input.as_slice());
        let mut peak = 0.0f32;
        let mut next_event = context.next_event();
        let frames = buffer.samples();
        let main = buffer.as_slice();
        let mut mixed = [0.0; 64];
        let silence = [0.0; 64];
        for start in (0..frames).step_by(mixed.len()) {
            let end = (start + mixed.len()).min(frames);
            let dry = &mut mixed[..end - start];
            let silent = &silence[..dry.len()];
            let left = main.first().map_or(silent, |s| &s[start..end]);
            let right = main.get(1).map_or(left, |s| &s[start..end]);
            let side = |channel| {
                external
                    .as_ref()
                    .and_then(|bus| bus.get(channel))
                    .and_then(|s: &&mut [f32]| s.get(start..end))
                    .unwrap_or(silent)
            };
            // Read each chunk before overwriting the in-place main bus.
            mix_inputs([left, right, side(0), side(1)], gains, dry);
            for (offset, &dry) in dry.iter().enumerate() {
                let sample_id = start + offset;
                while let Some(event) = next_event {
                    if event.timing() > sample_id as u32 {
                        break;
                    }
                    if let NoteEvent::NoteOn { velocity, .. } = event {
                        if velocity > 0.0 {
                            self.start_capture(&machine_params);
                        }
                    }
                    next_event = context.next_event();
                }
                let chan = self.active_channel;
                let wet = self
                    .distortion
                    .process(self.decimator.process(dry * input_gain, srr));
                peak = peak.max(wet.abs());
                let capturing = self.ram_record.is_recording(chan);
                self.ram_record.record_mono(wet, chan);
                let monitor = dry * cue1
                    + if capturing {
                        self.ram_record.monitor_sample(chan) * cue2
                    } else {
                        0.0
                    };
                // Monitoring is opt-in. Zero cue gains preserve silent output.
                for channel in main.iter_mut() {
                    channel[sample_id] = monitor;
                }
            }
        }
        self.ui
            .capture_locked
            .store(self.one_shot_spent, Ordering::Relaxed);
        self.ui
            .publish(self.ram_record.is_recording(self.active_channel), peak);
        ProcessStatus::Normal
    }
}

impl ClapPlugin for RecordPlugin {
    const CLAP_ID: &'static str = "com.ephemeraldsp.ultrawave-record";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("12-bit RAM capture effect with main and sidechain inputs");
    const CLAP_MANUAL_URL: Option<&'static str> = Some("https://github.com/EphemeralDSP/ultrawave");
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Sampler];
}

impl Vst3Plugin for RecordPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"EphemUWaveRecSt1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx];
}

impl Drop for RecordPlugin {
    fn drop(&mut self) {
        // Stop every writer before the instance lease is released.
        self.ram_record.stop_all_recording();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machines::shared::BufferRegistry;
    use nice_plug::context::{process::Transport, PluginApi};
    use std::collections::VecDeque;

    struct Context(VecDeque<PluginNoteEvent<RecordPlugin>>);
    impl ProcessContext<RecordPlugin> for Context {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Standalone
        }
        fn execute_background(&self, _: ()) {}
        fn execute_gui(&self, _: ()) {}
        fn transport(&self) -> &Transport {
            unreachable!("tempo supplied to process_audio")
        }
        fn next_event(&mut self) -> Option<PluginNoteEvent<RecordPlugin>> {
            self.0.pop_front()
        }
        fn try_send_event(
            &mut self,
            event: PluginNoteEvent<RecordPlugin>,
        ) -> Result<
            (),
            (
                PluginNoteEvent<RecordPlugin>,
                nice_plug::context::process::SendEventError,
            ),
        > {
            Err((
                event,
                nice_plug::context::process::SendEventError::NoOutputBuffer,
            ))
        }
        fn set_latency_samples(&self, _: u32) {}
        fn request_restart(&self) {}
        fn set_current_voice_capacity(&self, _: u32) {}
    }

    fn int(value: i32) -> IntParam {
        IntParam::new("test", value, IntRange::Linear { min: 0, max: 127 })
    }

    fn render(plugin: &mut RecordPlugin, frames: &[[f32; 4]], note: Option<u32>) -> Vec<f32> {
        let mut left: Vec<_> = frames.iter().map(|f| f[0]).collect();
        let mut right: Vec<_> = frames.iter().map(|f| f[1]).collect();
        let mut side_left: Vec<_> = frames.iter().map(|f| f[2]).collect();
        let mut side_right: Vec<_> = frames.iter().map(|f| f[3]).collect();
        let mut main = Buffer::default();
        let mut side = Buffer::default();
        // SAFETY: Every channel has frames.len() elements; the slices remain
        // alive and exclusively borrowed until processing has finished.
        unsafe {
            main.set_slices(frames.len(), |s| *s = vec![&mut left, &mut right]);
            side.set_slices(frames.len(), |s| *s = vec![&mut side_left, &mut side_right]);
        }
        let mut inputs = [side];
        let mut aux = AuxiliaryBuffers {
            inputs: &mut inputs,
            outputs: &mut [],
        };
        let mut context = Context(
            note.into_iter()
                .map(|timing| NoteEvent::NoteOn {
                    timing,
                    voice_id: nice_plug::midi::VoiceID::Wildcard,
                    channel: nice_plug::midi::Channel::Number(0),
                    key: nice_plug::midi::Key::MIDDLE_C,
                    velocity: 1.0,
                })
                .collect(),
        );
        assert_eq!(
            plugin.process_audio(&mut main, &mut aux, &mut context, 120.0),
            ProcessStatus::Normal
        );
        assert_eq!(left, right, "monitor output is dual mono");
        left
    }

    #[test]
    fn main_sidechain_balance_and_monitors_are_independent() {
        let registry = Arc::new(BufferRegistry::new());
        let mut plugin = RecordPlugin::default();
        plugin.ram_record = RamRecord::with_registry(44100.0, registry.clone());
        let frames = [[0.25, -0.75, 0.5, -0.125]; 4];
        assert_eq!(render(&mut plugin, &frames, Some(2)), [0.0; 4]);
        assert_eq!(registry.read_buffer(0).unwrap(), [-512, -512]);
        assert_eq!(registry.buffer_len(1), 0);

        let p = Arc::get_mut(&mut plugin.params).unwrap();
        p.buffer = int(2);
        p.mlev = int(0);
        p.ilev = int(127);
        p.ibal = int(127);
        p.cue2 = int(127);
        // The old R1 take still captures the new live input mix until the
        // note at frame 1 switches capture to R3.
        assert_eq!(render(&mut plugin, &frames, Some(1)), [-0.125; 4]);
        assert_eq!(registry.read_buffer(2).unwrap(), [-256; 3]);
        assert_eq!(registry.read_buffer(0).unwrap(), [-512, -512, -256]);
        plugin.reset();
        assert_eq!(render(&mut plugin, &frames, None), [0.0; 4]);
        let p = Arc::get_mut(&mut plugin.params).unwrap();
        p.cue2 = int(0);
        p.cue1 = int(127);
        p.ibal = int(0);
        assert_eq!(render(&mut plugin, &frames, None), [0.5; 4]);
        assert_eq!(registry.read_buffer(2).unwrap(), [-256; 3]);
    }

    #[test]
    fn capture_edge_does_not_retrigger_each_block_and_effects_reach_ram() {
        let registry = Arc::new(BufferRegistry::new());
        let mut plugin = RecordPlugin::default();
        plugin.ram_record = RamRecord::with_registry(44100.0, registry.clone());
        Arc::get_mut(&mut plugin.params).unwrap().capture = BoolParam::new("Capture", true);
        render(&mut plugin, &[[0.25; 4]; 2], None);
        render(&mut plugin, &[[0.5; 4]; 2], None);
        assert_eq!(registry.read_buffer(0).unwrap(), [512, 512, 1024, 1024]);
        Arc::get_mut(&mut plugin.params).unwrap().capture = BoolParam::new("Capture", false);
        render(&mut plugin, &[[0.75; 4]], None);
        assert_eq!(registry.buffer_len(0), 5, "falling edge does not truncate");
        let p = Arc::get_mut(&mut plugin.params).unwrap();
        p.capture = BoolParam::new("Capture", true);
        p.srr = FloatParam::new(
            "SRR",
            0.5,
            FloatRange::Linear {
                min: 0.0,
                max: 0.9629,
            },
        );
        p.lev = FloatParam::new(
            "Level",
            -6.0206,
            FloatRange::Linear {
                min: -60.0,
                max: 12.0,
            },
        );
        render(
            &mut plugin,
            &[[0.25; 4], [0.5; 4], [-0.25; 4], [-0.5; 4]],
            None,
        );
        assert_eq!(registry.read_buffer(0).unwrap(), [0, 512, 512, -512]);
        let p = Arc::get_mut(&mut plugin.params).unwrap();
        p.dist = int(127);
        p.srr = FloatParam::new(
            "SRR",
            0.0,
            FloatRange::Linear {
                min: 0.0,
                max: 0.9629,
            },
        );
        render(&mut plugin, &[[0.5; 4]], Some(0));
        assert_eq!(registry.read_buffer(0).unwrap(), [2047]);
        assert!(f32::from_bits(plugin.ui.peak.load(Ordering::Relaxed)) > 0.9);
    }

    #[test]
    fn one_shot_protects_the_first_take_during_capture_and_after_transport_reset() {
        let registry = Arc::new(BufferRegistry::new());
        let mut plugin = RecordPlugin::default();
        // At 32 Hz, one bar at 120 BPM is exactly 64 samples.
        plugin.ram_record = RamRecord::with_registry(32.0, registry.clone());
        Arc::get_mut(&mut plugin.params).unwrap().one_shot = BoolParam::new("One Shot", true);
        render(&mut plugin, &[[0.25; 4]; 32], Some(0));
        render(&mut plugin, &[[-0.5; 4]; 40], Some(0));
        let expected = [vec![512; 32], vec![-1024; 32]].concat();
        assert_eq!(registry.read_buffer(0).unwrap(), expected);

        // Neither another MIDI trigger, a Capture automation edge, nor the
        // UI Capture button may replace this take after stop/start.
        plugin.reset();
        Arc::get_mut(&mut plugin.params).unwrap().capture = BoolParam::new("Capture", true);
        plugin.ui.trigger.store(true, Ordering::Relaxed);
        render(&mut plugin, &[[0.75; 4]; 8], Some(3));
        assert_eq!(registry.read_buffer(0).unwrap(), expected);
    }

    #[test]
    fn one_shot_rearm_waits_for_a_trigger_and_does_not_interrupt_a_take() {
        let registry = Arc::new(BufferRegistry::new());
        let mut plugin = RecordPlugin::default();
        plugin.ram_record = RamRecord::with_registry(32.0, registry.clone());
        Arc::get_mut(&mut plugin.params).unwrap().one_shot = BoolParam::new("One Shot", true);
        render(&mut plugin, &[[0.25; 4]; 32], Some(0));
        plugin.ui.rearm.store(true, Ordering::Relaxed);
        render(&mut plugin, &[[-0.5; 4]; 32], Some(0));
        let first_take = [vec![512; 32], vec![-1024; 32]].concat();
        assert_eq!(registry.read_buffer(0).unwrap(), first_take);

        plugin.ui.rearm.store(true, Ordering::Relaxed);
        render(&mut plugin, &[[0.75; 4]; 8], None);
        assert_eq!(
            registry.read_buffer(0).unwrap(),
            first_take,
            "arming keeps RAM"
        );
        render(&mut plugin, &[[0.75; 4]; 68], Some(4));
        assert_eq!(registry.read_buffer(0).unwrap(), [1536; 64]);
        render(&mut plugin, &[[-0.25; 4]; 8], Some(0));
        assert_eq!(registry.read_buffer(0).unwrap(), [1536; 64]);
    }

    #[test]
    fn one_shot_toggle_rearms_and_adopts_an_in_progress_take() {
        let registry = Arc::new(BufferRegistry::new());
        let mut plugin = RecordPlugin::default();
        plugin.ram_record = RamRecord::with_registry(32.0, registry.clone());
        Arc::get_mut(&mut plugin.params).unwrap().one_shot = BoolParam::new("One Shot", true);
        render(&mut plugin, &[[0.25; 4]; 64], Some(0));
        Arc::get_mut(&mut plugin.params).unwrap().one_shot = BoolParam::new("One Shot", false);
        render(&mut plugin, &[[0.0; 4]; 8], None);
        Arc::get_mut(&mut plugin.params).unwrap().one_shot = BoolParam::new("One Shot", true);
        render(&mut plugin, &[[-0.5; 4]; 8], Some(0));
        assert_eq!(registry.read_buffer(0).unwrap(), [-1024; 8]);

        Arc::get_mut(&mut plugin.params).unwrap().one_shot = BoolParam::new("One Shot", false);
        render(&mut plugin, &[[0.25; 4]; 8], None);
        Arc::get_mut(&mut plugin.params).unwrap().one_shot = BoolParam::new("One Shot", true);
        render(&mut plugin, &[[0.75; 4]; 56], Some(0));
        assert_eq!(
            registry.read_buffer(0).unwrap(),
            [vec![-1024; 8], vec![512; 8], vec![1536; 48]].concat()
        );
        Arc::get_mut(&mut plugin.params).unwrap().one_shot = BoolParam::new("One Shot", false);
        render(&mut plugin, &[[-0.25; 4]; 8], Some(0));
        render(&mut plugin, &[[0.5; 4]; 5], Some(0));
        assert_eq!(registry.read_buffer(0).unwrap(), [1024; 5]);
    }

    #[test]
    fn rejected_one_shot_stays_armed_and_lock_is_per_instance_not_per_buffer() {
        let registry = Arc::new(BufferRegistry::new());
        let mut owner = RamRecord::with_registry(32.0, registry.clone());
        owner.start_recording(&RamRecordMachineParams::default(), true, 0);
        owner.record_mono(-0.25, 0);
        let mut plugin = RecordPlugin::default();
        plugin.ram_record = RamRecord::with_registry(32.0, registry.clone());
        Arc::get_mut(&mut plugin.params).unwrap().one_shot = BoolParam::new("One Shot", true);
        render(&mut plugin, &[[0.75; 4]; 4], Some(0));
        assert_eq!(registry.read_buffer(0).unwrap(), [-512]);
        owner.stop_all_recording();
        render(&mut plugin, &[[0.5; 4]; 64], Some(0));
        assert_eq!(registry.read_buffer(0).unwrap(), [1024; 64]);

        // Selecting another buffer does not silently re-arm this recorder.
        Arc::get_mut(&mut plugin.params).unwrap().buffer = int(2);
        render(&mut plugin, &[[-0.5; 4]; 64], Some(0));
        assert_eq!(registry.buffer_len(2), 0);
        plugin.ui.rearm.store(true, Ordering::Relaxed);
        render(&mut plugin, &[[-0.5; 4]; 64], Some(0));
        assert_eq!(registry.read_buffer(2).unwrap(), [-1024; 64]);
        assert_eq!(registry.read_buffer(0).unwrap(), [1024; 64]);
    }

    #[test]
    fn midi_and_capture_are_independent_of_simd_and_host_block_boundaries() {
        let frames: Vec<_> = (0..257)
            .map(|i| {
                let f = i as f32 / 2048.0;
                [2.0 * f, -f, 0.5 * f, -0.25 * f]
            })
            .collect();
        // Two centered stereo buses sum to 5/8 of f. Integer arithmetic gives
        // independently rounded 12-bit codes, including half-step boundaries.
        let codes: Vec<_> = (0..257).map(|i| ((5 * i + 4) / 8) as i16).collect();
        for note in [0, 63, 64, 65, 256] {
            for blocks in [&[257][..], &[1, 3, 61, 2, 64, 126][..]] {
                let registry = Arc::new(BufferRegistry::new());
                let mut plugin = RecordPlugin::default();
                plugin.ram_record = RamRecord::with_registry(44100.0, registry.clone());
                let p = Arc::get_mut(&mut plugin.params).unwrap();
                p.ilev = int(127);
                p.cue2 = int(127);
                let mut offset = 0;
                let mut output = Vec::new();
                for &len in blocks {
                    let event = (offset..offset + len)
                        .contains(&note)
                        .then(|| (note - offset) as u32);
                    output.extend(render(&mut plugin, &frames[offset..offset + len], event));
                    offset += len;
                }
                assert_eq!(registry.read_buffer(0).unwrap(), codes[note..]);
                assert!(output[..note].iter().all(|&sample| sample == 0.0));
                assert_eq!(
                    output[note..],
                    codes[note..]
                        .iter()
                        .map(|&code| code as f32 / 2048.0)
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
