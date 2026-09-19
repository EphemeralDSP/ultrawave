use nih_plug::prelude::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::machines::ram_play::{RamPlay, RamPlayParams as RamPlayMachineParams};
use crate::machines::shared::{
    register_plugin_instance, set_play_midi_note_active, unregister_plugin_instance,
};
use crate::params::RamPlayParams;
use crate::plugin::play_editor;

pub struct PlayPlugin {
    params: Arc<RamPlayParams>,
    editor_state: Arc<nih_plug_vizia::ViziaState>,
    ram_play: RamPlay,
    sample_rate: f32,
    active_channel: usize,
    velocity: f32,
    midi_led_hold_samples: usize,
    lifecycle_registered: bool,
    preload_attempted: bool,
}

impl Default for PlayPlugin {
    fn default() -> Self {
        let sample_rate = 44100.0;
        let ram_play = RamPlay::new(sample_rate);

        Self {
            params: Arc::new(RamPlayParams::default()),
            editor_state: play_editor::default_state(),
            ram_play,
            sample_rate,
            active_channel: 0,
            velocity: 1.0,
            midi_led_hold_samples: 0,
            lifecycle_registered: false,
            preload_attempted: false,
        }
    }
}

impl Plugin for PlayPlugin {
    const NAME: &'static str = "Ultrawave-Play";
    const VENDOR: &'static str = "EphemeralDSP";
    const URL: &'static str = "https://github.com/EphemeralDSP/ultrawave";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[],
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
        play_editor::create(self.params.clone(), self.editor_state.clone())
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

        self.ram_play.set_sample_rate(buffer_config.sample_rate);
        self.sample_rate = buffer_config.sample_rate;
        self.midi_led_hold_samples = 0;
        set_play_midi_note_active(false);
        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let play_params = RamPlayMachineParams {
            strt: self.params.strt.value(),
            end: self.params.end.value(),
            pitch: self.params.pitch.value(),
            hold: self.params.hold.value(),
            dec: self.params.dec.value(),
            rtrg: self.params.rtrg.value(),
            rtim: self.params.rtim.value(),
            srr: self.params.srr.value(),
            brr: self.params.brr.value(),
            dist: self.params.dist.value(),
            vol: 127,
        };
        self.ram_play
            .update_params(&play_params, self.active_channel);
        let gain = util::db_to_gain(self.params.lev.value());
        let mut next_event = context.next_event();
        for (sample_id, channel_samples) in buffer.iter_samples().enumerate() {
            while let Some(event) = next_event {
                if event.timing() > sample_id as u32 {
                    break;
                }
                if let NoteEvent::NoteOn { velocity, .. } = event {
                    if velocity > 0.0 {
                        self.ram_play.stop_all();
                        let chan = self.params.buffer.value() as usize;
                        self.active_channel = chan;
                        self.velocity = velocity;
                        self.midi_led_hold_samples = (self.sample_rate * 0.25) as usize;
                        self.ram_play.trigger(&play_params, chan);
                    }
                }
                next_event = context.next_event();
            }
            let chan = self.active_channel;
            let sample_out = self.ram_play.process(chan) * self.velocity * gain;

            for sample in channel_samples {
                *sample = sample_out;
            }
            self.midi_led_hold_samples = self.midi_led_hold_samples.saturating_sub(1);
        }
        set_play_midi_note_active(
            self.midi_led_hold_samples > 0 || self.ram_play.is_playing(self.active_channel),
        );
        ProcessStatus::Normal
    }

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        Box::new(|_task| {})
    }

    fn reset(&mut self) {
        self.ram_play.stop_all();
        self.midi_led_hold_samples = 0;
        set_play_midi_note_active(false);
    }
}

impl ClapPlugin for PlayPlugin {
    const CLAP_ID: &'static str = "com.ephemeraldsp.ultrawave-play";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Ultrawave RAM PLAY machine - standalone test");
    const CLAP_MANUAL_URL: Option<&'static str> = Some("https://github.com/EphemeralDSP/ultrawave");
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Sampler];
}

impl Vst3Plugin for PlayPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"EphemUWavePlayS1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Sampler];
}

impl Drop for PlayPlugin {
    fn drop(&mut self) {
        set_play_midi_note_active(false);
        if self.lifecycle_registered {
            unregister_plugin_instance();
            self.lifecycle_registered = false;
        }
    }
}

// Standalone only - no plugin exports

#[cfg(test)]
mod tests {
    use super::*;
    use nih_plug::context::init::InitContext;
    use nih_plug::context::process::{ProcessContext, Transport};
    use nih_plug::context::PluginApi;
    use std::collections::VecDeque;

    use crate::machines::shared::{get_global_registry, play_midi_note_active};

    // Process/reset/Drop all update the same process-global MIDI LED flag.
    static PLUGIN_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct TestInitContext;

    impl InitContext<PlayPlugin> for TestInitContext {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Standalone
        }

        fn execute(&self, _task: <PlayPlugin as Plugin>::BackgroundTask) {}

        fn set_latency_samples(&self, _samples: u32) {}

        fn set_current_voice_capacity(&self, _capacity: u32) {}
    }

    struct TestProcessContext {
        events: VecDeque<PluginNoteEvent<PlayPlugin>>,
    }

    impl TestProcessContext {
        fn with_events(events: Vec<PluginNoteEvent<PlayPlugin>>) -> Self {
            Self {
                events: events.into(),
            }
        }
    }

    impl ProcessContext<PlayPlugin> for TestProcessContext {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Standalone
        }

        fn execute_background(&self, _task: <PlayPlugin as Plugin>::BackgroundTask) {}

        fn execute_gui(&self, _task: <PlayPlugin as Plugin>::BackgroundTask) {}

        fn transport(&self) -> &Transport {
            panic!("transport is not used in this test")
        }

        fn next_event(&mut self) -> Option<PluginNoteEvent<PlayPlugin>> {
            self.events.pop_front()
        }

        fn send_event(&mut self, _event: PluginNoteEvent<PlayPlugin>) {}

        fn set_latency_samples(&self, _samples: u32) {}

        fn set_current_voice_capacity(&self, _capacity: u32) {}
    }

    fn render(plugin: &mut PlayPlugin, len: usize, notes: &[(u32, f32)]) -> Vec<f32> {
        let mut left = vec![0.0; len];
        let mut right = left.clone();
        let mut buffer = Buffer::default();
        // SAFETY: Equal-sized channel slices remain alive and exclusively
        // borrowed throughout the process call.
        unsafe {
            buffer.set_slices(len, |slices| *slices = vec![&mut left, &mut right]);
        }
        let mut context = TestProcessContext::with_events(
            notes
                .iter()
                .map(|&(timing, velocity)| NoteEvent::NoteOn {
                    timing,
                    voice_id: None,
                    channel: 0,
                    note: 60,
                    velocity,
                })
                .collect(),
        );
        let mut aux = AuxiliaryBuffers {
            inputs: &mut [],
            outputs: &mut [],
        };
        assert_eq!(
            plugin.process(&mut buffer, &mut aux, &mut context),
            ProcessStatus::Normal
        );
        assert_eq!(left, right);
        left
    }

    #[test]
    fn playback_honors_note_offsets_velocity_pitch_and_block_boundaries() {
        let _guard = PLUGIN_TEST_LOCK.lock().unwrap();
        let registry = Arc::new(crate::machines::shared::BufferRegistry::new());
        registry.replace_buffer(0, vec![512, -1024, 1536, 256, -512]);
        let mut plugin = PlayPlugin::default();
        plugin.ram_play = RamPlay::with_registry(44100.0, registry.clone());
        assert_eq!(
            render(&mut plugin, 9, &[(2, 0.5), (5, 1.0)]),
            [0.0, 0.0, 0.125, -0.25, 0.375, 0.25, -0.5, 0.75, 0.125]
        );

        Arc::get_mut(&mut plugin.params).unwrap().pitch =
            IntParam::new("Pitch", 76, IntRange::Linear { min: 0, max: 127 });
        assert_eq!(
            render(&mut plugin, 5, &[(0, 1.0)]),
            [0.25, 0.75, -0.25, 0.0, 0.0]
        );

        let timeline = |block_size: usize| {
            let mut plugin = PlayPlugin::default();
            plugin.ram_play = RamPlay::with_registry(44100.0, registry.clone());
            let mut output = Vec::new();
            for start in (0..128).step_by(block_size) {
                let len = block_size.min(128 - start);
                let notes: Vec<_> = [3, 19, 70]
                    .into_iter()
                    .filter(|t| *t >= start && *t < start + len)
                    .map(|t| ((t - start) as u32, 1.0))
                    .collect();
                output.extend(render(&mut plugin, len, &notes));
            }
            output
        };
        assert_eq!(timeline(1), timeline(7));
        assert_eq!(timeline(7), timeline(64));
    }

    #[test]
    fn brr_srr_distortion_and_level_are_independent_live_controls() {
        let _guard = PLUGIN_TEST_LOCK.lock().unwrap();
        let registry = Arc::new(crate::machines::shared::BufferRegistry::new());
        registry.replace_buffer(0, vec![256, 768, -256, -768, 256, 768, -256, -768]);
        let mut plugin = PlayPlugin::default();
        plugin.ram_play = RamPlay::with_registry(44100.0, registry);
        assert_eq!(
            render(&mut plugin, 4, &[(0, 1.0)]),
            [0.125, 0.375, -0.125, -0.375]
        );

        Arc::get_mut(&mut plugin.params).unwrap().brr =
            IntParam::new("BRR", 127, IntRange::Linear { min: 0, max: 127 });
        // Change BRR on a running voice, with no new note. At 2 bits the
        // amplitude grid is 0.5; a duplicated SRR binding cannot produce this.
        assert_eq!(render(&mut plugin, 4, &[]), [0.0, 0.5, 0.0, -0.5]);
        Arc::get_mut(&mut plugin.params).unwrap().brr =
            IntParam::new("BRR", 0, IntRange::Linear { min: 0, max: 127 });
        Arc::get_mut(&mut plugin.params).unwrap().srr = FloatParam::new(
            "SRR",
            0.5,
            FloatRange::Linear {
                min: 0.0,
                max: 0.9629,
            },
        );
        // SRR holds every second frame without changing amplitude precision.
        assert_eq!(
            render(&mut plugin, 4, &[(0, 1.0)]),
            [0.0, 0.375, 0.375, -0.375]
        );

        let params = Arc::get_mut(&mut plugin.params).unwrap();
        params.srr = FloatParam::new(
            "SRR",
            0.0,
            FloatRange::Linear {
                min: 0.0,
                max: 0.9629,
            },
        );
        params.dist = IntParam::new("DIST", 127, IntRange::Linear { min: 0, max: 127 });
        let driven = render(&mut plugin, 4, &[(0, 1.0)]);
        assert!(driven[0] > 0.9 && driven[2] < -0.9);
        assert!(driven.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        Arc::get_mut(&mut plugin.params).unwrap().lev = FloatParam::new(
            "Level",
            -20.0,
            FloatRange::Linear {
                min: -60.0,
                max: 12.0,
            },
        );
        for (actual, full_level) in render(&mut plugin, 4, &[]).iter().zip(driven) {
            assert!((actual - full_level * 0.1).abs() < 1e-6);
        }
        render(&mut plugin, 1, &[(0, 1.0)]);
        // Envelope changes are latched until the next trigger, not applied
        // using the previous voice's decay coefficient.
        Arc::get_mut(&mut plugin.params).unwrap().dec =
            IntParam::new("Decay", 64, IntRange::Linear { min: 0, max: 127 });
        assert!(render(&mut plugin, 1, &[])[0] > 0.09);
        plugin.reset();
        assert_eq!(render(&mut plugin, 4, &[]), [0.0; 4]);
    }

    #[test]
    fn note_on_sets_midi_led_active_flag() {
        let _guard = PLUGIN_TEST_LOCK.lock().unwrap();
        const BLOCK_SIZE: usize = 64;
        const SAMPLE_RATE: f32 = 44_100.0;

        get_global_registry().clear_all();
        set_play_midi_note_active(false);
        assert!(!play_midi_note_active());

        let mut plugin = PlayPlugin::default();
        let mut init_context = TestInitContext;
        let buffer_config = BufferConfig {
            sample_rate: SAMPLE_RATE,
            min_buffer_size: Some(BLOCK_SIZE as u32),
            max_buffer_size: BLOCK_SIZE as u32,
            process_mode: ProcessMode::Realtime,
        };
        assert!(plugin.initialize(
            &PlayPlugin::AUDIO_IO_LAYOUTS[0],
            &buffer_config,
            &mut init_context
        ));

        let mut left = vec![0.0f32; BLOCK_SIZE];
        let mut right = vec![0.0f32; BLOCK_SIZE];
        let mut buffer = Buffer::default();
        unsafe {
            buffer.set_slices(BLOCK_SIZE, |output_slices| {
                *output_slices = vec![left.as_mut_slice(), right.as_mut_slice()];
            });
        }
        let mut aux_inputs: [Buffer; 0] = [];
        let mut aux_outputs: [Buffer; 0] = [];
        let mut aux = AuxiliaryBuffers {
            inputs: &mut aux_inputs,
            outputs: &mut aux_outputs,
        };

        let note_on = NoteEvent::NoteOn {
            timing: 0,
            voice_id: None,
            channel: 0,
            note: 60,
            velocity: 1.0,
        };
        let mut process_context = TestProcessContext::with_events(vec![note_on]);

        let status = plugin.process(&mut buffer, &mut aux, &mut process_context);
        assert_eq!(status, ProcessStatus::Normal);
        assert!(
            play_midi_note_active(),
            "MIDI LED flag should be true after NoteOn"
        );
    }
}
