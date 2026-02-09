use nih_plug::prelude::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::machines::ram_play::{RamPlay, RamPlayParams as RamPlayMachineParams};
use crate::machines::shared::{
    register_plugin_instance, set_play_midi_note_active, unregister_plugin_instance,
};
use crate::params::RamPlayParams;
use crate::plugin::play_editor;

pub struct StandalonePlay {
    params: Arc<RamPlayParams>,
    editor_state: Arc<nih_plug_vizia::ViziaState>,
    ram_play: RamPlay,
    sample_rate: f32,
    active_channel: usize,
    midi_led_hold_samples: usize,
    lifecycle_registered: bool,
    preload_attempted: bool,
}

impl Default for StandalonePlay {
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

impl Plugin for StandalonePlay {
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
            if let NoteEvent::NoteOn { .. } = event {
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
                    srr: 0,
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

impl ClapPlugin for StandalonePlay {
    const CLAP_ID: &'static str = "com.ephemeraldsp.ultrawave-play";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Ultrawave RAM PLAY machine - standalone test");
    const CLAP_MANUAL_URL: Option<&'static str> = Some("https://github.com/EphemeralDSP/ultrawave");
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Sampler];
}

impl Vst3Plugin for StandalonePlay {
    const VST3_CLASS_ID: [u8; 16] = *b"EphemUWavePlayS1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Sampler];
}

impl Drop for StandalonePlay {
    fn drop(&mut self) {
        set_play_midi_note_active(false);
        if self.lifecycle_registered {
            unregister_plugin_instance();
            self.lifecycle_registered = false;
        }
    }
}

// Standalone only - no plugin exports
