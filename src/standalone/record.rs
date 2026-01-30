use nih_plug::prelude::*;
use std::sync::Arc;

use crate::dsp::filter::ResonantFilter;
use crate::machines::ram_record::{RamRecord, RamRecordParams as RamRecordMachineParams};
use crate::params::RamRecordParams;
use crate::standalone::record_editor;

pub struct StandaloneRecord {
    params: Arc<RamRecordParams>,
    editor_state: Arc<nih_plug_vizia::ViziaState>,
    sample_rate: f32,
    ram_record: RamRecord,
    filter: ResonantFilter,
}

impl Default for StandaloneRecord {
    fn default() -> Self {
        let sample_rate = 44100.0;
        Self {
            params: Arc::new(RamRecordParams::default()),
            editor_state: record_editor::default_state(),
            sample_rate,
            ram_record: RamRecord::new(sample_rate),
            filter: ResonantFilter::new(sample_rate),
        }
    }
}

impl Plugin for StandaloneRecord {
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
        record_editor::create(self.params.clone(), self.editor_state.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        self.ram_record.set_sample_rate(buffer_config.sample_rate);
        self.filter.set_sample_rate(buffer_config.sample_rate);
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
                NoteEvent::NoteOn { .. } => {
                    let chan = 0;
                    let machine_params = RamRecordMachineParams {
                        mlev: self.params.mlev.value(),
                        mbal: self.params.mbal.value(),
                        ilev: self.params.ilev.value(),
                        ibal: self.params.ibal.value(),
                        rec_len: self.params.rec_len.value(),
                        rec_rate: self.params.rec_rate.value(),
                    };
                    self.ram_record.start_recording(&machine_params, true, chan);
                }
                NoteEvent::NoteOff { .. } => {
                    let chan = 0;
                    self.ram_record.stop_recording(chan);
                }
                _ => {}
            }
        }

        // Update filter parameters
        let filter_freq =
            20.0 + (self.params.cue1.value() as f32 / 127.0) * (self.sample_rate * 0.45 - 20.0);
        let filter_resonance = self.params.cue2.value() as f32 / 127.0;
        self.filter.set_params(
            filter_freq,
            filter_resonance,
            crate::dsp::filter::FilterMode::LowPass,
        );

        for mut channel_samples in buffer.iter_samples() {
            let chan = 0;
            let machine_params = RamRecordMachineParams {
                mlev: self.params.mlev.value(),
                mbal: self.params.mbal.value(),
                ilev: self.params.ilev.value(),
                ibal: self.params.ibal.value(),
                rec_len: self.params.rec_len.value(),
                rec_rate: self.params.rec_rate.value(),
            };

            // Collect samples first
            let samples: Vec<f32> = channel_samples.iter_mut().map(|s| *s).collect();
            let left = samples.get(0).copied().unwrap_or(0.0);
            let right = samples.get(1).copied().unwrap_or(0.0);

            // Record the sample (main input is the audio input)
            self.ram_record
                .record_sample(left, right, left, right, &machine_params, chan);

            // Pass through with filtering
            let (filtered_l, filtered_r) = self.filter.process_stereo(left, right);

            // Write back filtered samples
            for (idx, sample) in channel_samples.iter_mut().enumerate() {
                *sample = if idx == 0 { filtered_l } else { filtered_r };
            }
        }
        ProcessStatus::Normal
    }

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        Box::new(|_task| {})
    }
}

impl ClapPlugin for StandaloneRecord {
    const CLAP_ID: &'static str = "com.ephemeraldsp.ultrawave-record";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Ultrawave RAM RECORD machine - standalone test");
    const CLAP_MANUAL_URL: Option<&'static str> = Some("https://github.com/EphemeralDSP/ultrawave");
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect];
}

impl Vst3Plugin for StandaloneRecord {
    const VST3_CLASS_ID: [u8; 16] = *b"EphemUWaveRecSt1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

// Standalone only - no plugin exports
