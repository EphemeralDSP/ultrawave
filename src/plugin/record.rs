use nih_plug::prelude::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::dsp::effects::Distortion;
use crate::machines::ram_record::{RamRecord, RamRecordParams as RamRecordMachineParams};
use crate::machines::shared::{register_plugin_instance, unregister_plugin_instance};
use crate::params::RamRecordParams;
use crate::plugin::{
    editor::{self, EditorParams},
    UiState,
};

pub struct RecordPlugin {
    params: Arc<RamRecordParams>,
    editor_state: Arc<nih_plug_vizia::ViziaState>,
    sample_rate: f32,
    ram_record: RamRecord,
    active_channel: usize,
    ui: Arc<UiState>,
    capture_was_high: bool,
    decimator: crate::Decimator,
    distortion: Distortion,
    lifecycle_registered: bool,
    preload_attempted: bool,
}

impl Default for RecordPlugin {
    fn default() -> Self {
        let sample_rate = 44100.0;
        Self {
            params: Arc::new(RamRecordParams::default()),
            editor_state: editor::default_state(),
            sample_rate,
            ram_record: RamRecord::new(sample_rate),
            active_channel: 0,
            ui: Arc::new(UiState::default()),
            capture_was_high: false,
            decimator: crate::Decimator::new(),
            distortion: Distortion::new(),
            lifecycle_registered: false,
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

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        if crate::NO_GUI.load(Ordering::Relaxed) {
            return None;
        }
        editor::create(
            EditorParams::Record(self.params.clone()),
            self.editor_state.clone(),
            self.ui.clone(),
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        if !self.lifecycle_registered {
            register_plugin_instance();
            self.lifecycle_registered = true;
        }
        if !self.preload_attempted {
            if let Err(error) = crate::plugin::maybe_preload_buffer_from_env() {
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
        self.ram_record.stop_all_recording();
        self.active_channel = self.params.buffer.value() as usize;
        self.decimator = crate::Decimator::new();
        self.ram_record
            .start_recording(params, true, self.active_channel);
    }

    fn process_audio(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
        tempo_bpm: f64,
    ) -> ProcessStatus {
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
        for (sample_id, mut channel_samples) in buffer.iter_samples().enumerate() {
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
            let left = channel_samples.get_mut(0).copied().unwrap_or(0.0);
            let right = channel_samples.get_mut(1).copied().unwrap_or(left);
            let ext_l = external
                .as_ref()
                .and_then(|bus| bus.first())
                .and_then(|samples| samples.get(sample_id))
                .copied()
                .unwrap_or(0.0);
            let ext_r = external
                .as_ref()
                .and_then(|bus| bus.get(1))
                .and_then(|samples| samples.get(sample_id))
                .copied()
                .unwrap_or(0.0);
            let dry = left * gains[0] + right * gains[1] + ext_l * gains[2] + ext_r * gains[3];
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
            // Monitoring is opt-in. Zero cue gains preserve the silent sampler output.
            for sample in channel_samples.iter_mut() {
                *sample = monitor;
            }
        }
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
        self.ram_record.stop_all_recording();
        if self.lifecycle_registered {
            unregister_plugin_instance();
            self.lifecycle_registered = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machines::shared::BufferRegistry;
    use nih_plug::context::{process::Transport, PluginApi};
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
        fn send_event(&mut self, _: PluginNoteEvent<RecordPlugin>) {}
        fn set_latency_samples(&self, _: u32) {}
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
                    voice_id: None,
                    channel: 0,
                    note: 60,
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
}
