use nih_plug::prelude::*;
use std::sync::Arc;

use crate::dsp::filter::ResonantFilter;
use crate::machines::ram_play::{RamPlay, RamPlayParams as RamPlayMachineParams};
use crate::params::RamPlayParams;

pub struct StandalonePlay {
    params: Arc<RamPlayParams>,
    sample_rate: f32,
    ram_play: RamPlay,
    filter: ResonantFilter,
    test_buffer_loaded: bool,
}

impl Default for StandalonePlay {
    fn default() -> Self {
        let sample_rate = 44100.0;
        let mut ram_play = RamPlay::new(sample_rate);

        // Load a test buffer (saw wave)
        let test_buffer: Vec<i16> = (0..10000)
            .map(|i| {
                let phase = (i % 100) as f32 / 100.0;
                (phase * 2.0 - 1.0) * 2047.0 as f32
            })
            .map(|f| f as i16)
            .collect();
        ram_play.load_buffer(test_buffer, 0);

        Self {
            params: Arc::new(RamPlayParams::default()),
            sample_rate,
            ram_play,
            filter: ResonantFilter::new(sample_rate),
            test_buffer_loaded: true,
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

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        self.ram_play.set_sample_rate(buffer_config.sample_rate);
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
                NoteEvent::NoteOn { velocity, .. } => {
                    let chan = 0;
                    if self.ram_play.buffer_len(chan) > 0 || self.test_buffer_loaded {
                        let play_params = RamPlayMachineParams {
                            strt: self.params.strt.value(),
                            end: self.params.end.value(),
                            pitch: self.params.pitch.value(),
                            hold: self.params.hold.value(),
                            dec: self.params.dec.value(),
                            rtrg: self.params.rtrg.value(),
                            rtim: self.params.rtim.value(),
                            srr: self.params.srr.value(),
                            vol: ((velocity * 127.0) as i32).min(127),
                        };
                        self.ram_play.trigger(&play_params, chan);
                    }
                }
                NoteEvent::NoteOff { .. } => {
                    let chan = 0;
                    self.ram_play.stop(chan);
                }
                _ => {}
            }
        }

        // Update filter parameters (reusing srr and rtim for filter)
        let filter_freq =
            20.0 + (self.params.srr.value() as f32 / 127.0) * (self.sample_rate * 0.45 - 20.0);
        let filter_resonance = self.params.rtim.value() as f32 / 127.0;
        self.filter.set_params(
            filter_freq,
            filter_resonance,
            crate::dsp::filter::FilterMode::LowPass,
        );

        for channel_samples in buffer.iter_samples() {
            let chan = 0;
            let sample_out = self.ram_play.process(chan);
            let (left, right) = self.filter.process_stereo(sample_out, sample_out);

            let mut out_idx = 0;
            for sample in channel_samples {
                *sample = if out_idx == 0 { left } else { right };
                out_idx += 1;
            }
        }
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
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Sampler];
}

impl Vst3Plugin for StandalonePlay {
    const VST3_CLASS_ID: [u8; 16] = *b"EphemUWavePlayS1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Sampler];
}

// Standalone only - no plugin exports
