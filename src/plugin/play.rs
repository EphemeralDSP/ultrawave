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
        while let Some(event) = context.next_event() {
            if let NoteEvent::NoteOn {
                note, velocity, channel, ..
            } = event
            {
                nih_log!(
                    "MIDI NoteOn: note={note} vel={velocity:.2} ch={channel} buf={}",
                    self.params.buffer.value()
                );
                let chan = self.params.buffer.value() as usize;
                self.active_channel = chan;
                self.midi_led_hold_samples = (self.sample_rate * 0.25) as usize;
                let play_params = RamPlayMachineParams {
                    strt: 0,
                    end: 127,
                    pitch: 64,
                    hold: 0,
                    dec: 0,
                    rtrg: 0,
                    rtim: 0,
                    srr: 0.0,
                    vol: 127,
                };
                self.ram_play.trigger(&play_params, chan);
            }
        }

        for channel_samples in buffer.iter_samples() {
            let chan = self.active_channel;
            let sample_out = self.ram_play.process(chan);

            for sample in channel_samples {
                *sample = sample_out;
            }
        }
        self.midi_led_hold_samples = self.midi_led_hold_samples.saturating_sub(buffer.samples());
        set_play_midi_note_active(
            self.midi_led_hold_samples > 0 || self.ram_play.is_playing(self.active_channel),
        );
        ProcessStatus::Normal
    }

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        Box::new(|_task| {})
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

    #[test]
    fn note_on_sets_midi_led_active_flag() {
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
