use nih_plug::prelude::*;
use std::sync::Arc;

mod dsp;
mod editor;
mod machines;
mod params;

use params::UltrawaveParams;

pub struct Ultrawave {
    params: Arc<UltrawaveParams>,
    editor_state: Arc<nih_plug_vizia::ViziaState>,
    sample_rate: f32,
}

impl Default for Ultrawave {
    fn default() -> Self {
        Self {
            params: Arc::new(UltrawaveParams::default()),
            editor_state: editor::default_state(),
            sample_rate: 44100.0,
        }
    }
}

impl Plugin for Ultrawave {
    const NAME: &'static str = "Ultrawave";
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
        editor::create(self.params.clone(), self.editor_state.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        while let Some(event) = context.next_event() {
            match event {
                NoteEvent::NoteOn { note, velocity, .. } => {
                    nih_log!("Note ON: {} velocity: {}", note, velocity);
                }
                NoteEvent::NoteOff { note, .. } => {
                    nih_log!("Note OFF: {}", note);
                }
                _ => {}
            }
        }

        for channel_samples in buffer.iter_samples() {
            for sample in channel_samples {
                *sample = *sample;
            }
        }
        ProcessStatus::Normal
    }

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        Box::new(|_task| {})
    }
}

impl ClapPlugin for Ultrawave {
    const CLAP_ID: &'static str = "com.ephemeraldsp.ultrawave";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Elektron Machinedrum UW RAM machine emulation");
    const CLAP_MANUAL_URL: Option<&'static str> = Some("https://github.com/EphemeralDSP/ultrawave");
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Sampler];
}

impl Vst3Plugin for Ultrawave {
    const VST3_CLASS_ID: [u8; 16] = *b"EphemeralUltrav1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Sampler,
    ];
}

nih_export_clap!(Ultrawave);
nih_export_vst3!(Ultrawave);
