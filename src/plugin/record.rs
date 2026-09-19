use nih_plug::prelude::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::machines::ram_record::{RamRecord, RamRecordParams as RamRecordMachineParams};
use crate::machines::shared::{
    register_plugin_instance, set_record_midi_note_active, unregister_plugin_instance,
};
use crate::params::RamRecordParams;
use crate::plugin::record_editor;

pub struct RecordPlugin {
    params: Arc<RamRecordParams>,
    editor_state: Arc<nih_plug_vizia::ViziaState>,
    sample_rate: f32,
    ram_record: RamRecord,
    active_channel: usize,
    midi_led_hold_samples: usize,
    lifecycle_registered: bool,
    preload_attempted: bool,
}

impl Default for RecordPlugin {
    fn default() -> Self {
        let sample_rate = 44100.0;
        Self {
            params: Arc::new(RamRecordParams::default()),
            editor_state: record_editor::default_state(),
            sample_rate,
            ram_record: RamRecord::new(sample_rate),
            active_channel: 0,
            midi_led_hold_samples: 0,
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
        record_editor::create(self.params.clone(), self.editor_state.clone())
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
        self.midi_led_hold_samples = 0;
        set_record_midi_note_active(false);
        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let tempo_bpm = context.transport().tempo.unwrap_or(120.0);
        let machine_params = RamRecordMachineParams {
            mlev: self.params.mlev.value(),
            mbal: self.params.mbal.value(),
            ilev: self.params.ilev.value(),
            ibal: self.params.ibal.value(),
            rec_len: self.params.rec_len.value(),
            rec_rate: self.params.rec_rate.value(),
            tempo_bpm,
        };
        let mut next_event = context.next_event();
        for (sample_id, mut channel_samples) in buffer.iter_samples().enumerate() {
            while let Some(event) = next_event {
                if event.timing() > sample_id as u32 {
                    break;
                }
                if let NoteEvent::NoteOn { velocity, .. } = event {
                    if velocity > 0.0 {
                        self.ram_record.stop_all_recording();
                        let chan = self.params.buffer.value() as usize;
                        self.active_channel = chan;
                        self.midi_led_hold_samples = (self.sample_rate * 0.25) as usize;
                        self.ram_record.start_recording(&machine_params, true, chan);
                    }
                }
                next_event = context.next_event();
            }
            let chan = self.active_channel;
            let left = channel_samples.get_mut(0).copied().unwrap_or(0.0);
            let right = channel_samples.get_mut(1).copied().unwrap_or(left);

            // There is one stereo input bus, not two copies of the same input.
            self.ram_record
                .record_sample(left, right, 0.0, 0.0, &machine_params, chan);

            // No audio output - Record plugin is silent
            // (Cue 1 and Cue 2 will be used for aux outputs in the future)
            for sample in channel_samples.iter_mut() {
                *sample = 0.0;
            }
            self.midi_led_hold_samples = self.midi_led_hold_samples.saturating_sub(1);
        }
        set_record_midi_note_active(
            self.midi_led_hold_samples > 0 || self.ram_record.is_recording(self.active_channel),
        );
        ProcessStatus::Normal
    }

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        Box::new(|_task| {})
    }

    fn reset(&mut self) {
        self.ram_record.stop_all_recording();
        self.midi_led_hold_samples = 0;
        set_record_midi_note_active(false);
    }
}

impl ClapPlugin for RecordPlugin {
    const CLAP_ID: &'static str = "com.ephemeraldsp.ultrawave-record";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Ultrawave RAM RECORD machine - standalone test");
    const CLAP_MANUAL_URL: Option<&'static str> = Some("https://github.com/EphemeralDSP/ultrawave");
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Sampler];
}

impl Vst3Plugin for RecordPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"EphemUWaveRecSt1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Sampler];
}

impl Drop for RecordPlugin {
    fn drop(&mut self) {
        set_record_midi_note_active(false);
        if self.lifecycle_registered {
            unregister_plugin_instance();
            self.lifecycle_registered = false;
        }
    }
}

// Standalone only - no plugin exports
