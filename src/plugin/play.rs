use nice_plug::prelude::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::machines::ram_play::{RamPlay, RamPlayParams as RamPlayMachineParams};
use crate::params::RamPlayParams;
use crate::plugin::{
    editor::{self, EditorParams},
    pairing::Pairing,
    UiState,
};

pub struct PlayPlugin {
    params: Arc<RamPlayParams>,
    editor_state: Arc<nice_plug_egui::EguiEditorState>,
    ui: Arc<UiState>,
    ram_play: RamPlay,
    active_channel: usize,
    velocity: f32,
    transport_was_playing: bool,
    pairing: Pairing,
    preload_attempted: bool,
}

impl Default for PlayPlugin {
    fn default() -> Self {
        let sample_rate = 44100.0;
        let pairing = Pairing::new();
        let ram_play = RamPlay::with_registry(sample_rate, pairing.current_registry());

        Self {
            params: Arc::new(RamPlayParams::default()),
            editor_state: editor::default_state(),
            ui: Arc::new(UiState::default()),
            ram_play,
            active_channel: 0,
            velocity: 1.0,
            transport_was_playing: false,
            pairing,
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
    type Editor = editor::PluginEditor;

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        if crate::NO_GUI.load(Ordering::Relaxed) {
            return None;
        }
        editor::create(
            EditorParams::Play(self.params.clone()),
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
                self.ram_play.rebind_registry(registry)
            });
        if !self.preload_attempted {
            if let Err(error) =
                crate::plugin::maybe_preload_buffer_from_env(&self.pairing.current_registry())
            {
                eprintln!("standalone preload failed: {error}");
            }
            self.preload_attempted = true;
        }

        self.ram_play.set_sample_rate(buffer_config.sample_rate);
        self.reset();
        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let bank = self.params.bank.value() as u8;
        if self.pairing.request(bank) {
            self.ram_play.stop_all();
        }
        self.pairing
            .adopt(|registry| self.ram_play.rebind_registry(registry));
        self.ui
            .bank_status
            .store(self.pairing.status() as u32, Ordering::Relaxed);
        if matches!(
            self.pairing.status(),
            crate::plugin::pairing::CONNECTING | crate::plugin::pairing::FAILED
        ) {
            self.ui.trigger.store(false, Ordering::Relaxed);
            for channel in buffer.as_slice() {
                channel.fill(0.0);
            }
            self.ui.publish(false, 0.0);
            return ProcessStatus::Normal;
        }
        let transport = context.transport();
        self.ram_play.set_tempo(transport.tempo.unwrap_or(120.0));
        if self.transport_was_playing && !transport.playing {
            self.ram_play.stop_all();
        }
        self.transport_was_playing = transport.playing;
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
        if self.ui.trigger.swap(false, Ordering::Relaxed) {
            self.ram_play.stop_all();
            self.active_channel = self.params.buffer.value() as usize;
            self.velocity = 1.0;
            self.ram_play.trigger(&play_params, self.active_channel);
        }
        let mut peak = 0.0f32;
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
                        self.ram_play.trigger(&play_params, chan);
                    }
                }
                next_event = context.next_event();
            }
            let chan = self.active_channel;
            let sample_out = self.ram_play.process(chan) * self.velocity * gain;
            peak = peak.max(sample_out.abs());

            for sample in channel_samples {
                *sample = sample_out;
            }
        }
        self.ui
            .publish(self.ram_play.is_playing(self.active_channel), peak);
        ProcessStatus::Normal
    }

    fn reset(&mut self) {
        self.ram_play.stop_all();
        self.transport_was_playing = false;
        self.ui.active.store(false, Ordering::Relaxed);
        self.ui.peak.store(0, Ordering::Relaxed);
        self.ui.trigger.store(false, Ordering::Relaxed);
    }
}

impl ClapPlugin for PlayPlugin {
    const CLAP_ID: &'static str = "com.ephemeraldsp.ultrawave-play";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("12-bit RAM sampler with varispeed playback and independent BRR/SRR");
    const CLAP_MANUAL_URL: Option<&'static str> = Some("https://github.com/EphemeralDSP/ultrawave");
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Sampler];
}

impl Vst3Plugin for PlayPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"EphemUWavePlayS1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Sampler];
}

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::context::process::{ProcessContext, Transport};
    use nice_plug::context::PluginApi;
    use std::collections::VecDeque;

    struct TestProcessContext {
        events: VecDeque<PluginNoteEvent<PlayPlugin>>,
        transport: Transport,
    }

    impl TestProcessContext {
        fn with_events(events: Vec<PluginNoteEvent<PlayPlugin>>) -> Self {
            Self {
                events: events.into(),
                transport: Transport::new(44100.0),
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
            &self.transport
        }

        fn next_event(&mut self) -> Option<PluginNoteEvent<PlayPlugin>> {
            self.events.pop_front()
        }

        fn try_send_event(
            &mut self,
            event: PluginNoteEvent<PlayPlugin>,
        ) -> Result<
            (),
            (
                PluginNoteEvent<PlayPlugin>,
                nice_plug::context::process::SendEventError,
            ),
        > {
            Err((
                event,
                nice_plug::context::process::SendEventError::NoOutputBuffer,
            ))
        }

        fn set_latency_samples(&self, _samples: u32) {}

        fn request_restart(&self) {}

        fn set_current_voice_capacity(&self, _capacity: u32) {}
    }

    fn render(plugin: &mut PlayPlugin, len: usize, notes: &[(u32, f32)]) -> Vec<f32> {
        render_at_tempo(plugin, len, notes, None, false)
    }

    fn render_at_tempo(
        plugin: &mut PlayPlugin,
        len: usize,
        notes: &[(u32, f32)],
        tempo: Option<f64>,
        playing: bool,
    ) -> Vec<f32> {
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
                    voice_id: nice_plug::midi::VoiceID::Wildcard,
                    channel: nice_plug::midi::Channel::Number(0),
                    key: nice_plug::midi::Key::MIDDLE_C,
                    velocity,
                })
                .collect(),
        );
        context.transport.tempo = tempo;
        context.transport.playing = playing;
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
    fn retrigger_callback_follows_host_tempo_midi_and_block_boundaries() {
        let registry = Arc::new(crate::machines::shared::BufferRegistry::new());
        registry.replace_buffer_at_rate(0, vec![1024], 1024.0);
        for (tempo, interval) in [
            (Some(60.0), 256),
            (Some(120.0), 128),
            (Some(240.0), 64),
            (None, 128),
        ] {
            for block_size in [1, 7, 64, 129] {
                let mut plugin = PlayPlugin {
                    ram_play: RamPlay::with_registry(1024.0, registry.clone()),
                    ..Default::default()
                };
                let params = Arc::get_mut(&mut plugin.params).unwrap();
                params.rtrg = IntParam::new("Retrigger", 2, IntRange::Linear { min: 0, max: 127 });
                params.rtim =
                    IntParam::new("Retrig Time", 127, IntRange::Linear { min: 0, max: 127 });
                let mut output = Vec::new();
                for start in (0..900).step_by(block_size) {
                    let len = block_size.min(900 - start);
                    let notes: Vec<_> = [(3, 0.5), (300, 1.0)]
                        .into_iter()
                        .filter(|(t, _)| *t >= start && *t < start + len)
                        .map(|(t, v)| ((t - start) as u32, v))
                        .collect();
                    output.extend(render_at_tempo(&mut plugin, len, &notes, tempo, true));
                }
                let mut expected = vec![0.0; 900];
                for hit in 0..=2 {
                    if 3 + hit * interval < 300 {
                        expected[3 + hit * interval] = 0.25;
                    }
                    expected[300 + hit * interval] = 0.5;
                }
                assert_eq!(output, expected, "tempo={tempo:?} block={block_size}");
            }
        }
    }

    #[test]
    fn retrigger_callback_retimes_running_voice_and_stops_infinite_train() {
        let registry = Arc::new(crate::machines::shared::BufferRegistry::new());
        registry.replace_buffer_at_rate(0, vec![1024], 1024.0);
        let mut plugin = PlayPlugin {
            ram_play: RamPlay::with_registry(1024.0, registry),
            ..Default::default()
        };
        let params = Arc::get_mut(&mut plugin.params).unwrap();
        params.rtrg = IntParam::new("Retrigger", 127, IntRange::Linear { min: 0, max: 127 });
        params.rtim = IntParam::new("Retrig Time", 127, IntRange::Linear { min: 0, max: 127 });
        let first = render_at_tempo(&mut plugin, 64, &[(0, 1.0)], Some(120.0), true);
        assert_eq!(first[0], 0.5);
        assert!(first[1..].iter().all(|&v| v == 0.0));
        assert_eq!(
            render_at_tempo(&mut plugin, 32, &[], Some(240.0), true),
            vec![0.0; 32]
        );
        assert_eq!(
            render_at_tempo(&mut plugin, 1, &[], Some(240.0), true),
            [0.5]
        );
        assert_eq!(
            render_at_tempo(&mut plugin, 300, &[], Some(240.0), false),
            vec![0.0; 300]
        );
        assert!(!plugin.ram_play.is_playing(0));
        // Stopped transport does not prevent deliberate auditioning.
        plugin.ui.trigger.store(true, Ordering::Relaxed);
        assert_eq!(render(&mut plugin, 1, &[]), [0.5]);
        plugin.reset();
        assert_eq!(render(&mut plugin, 300, &[]), vec![0.0; 300]);
    }

    #[test]
    fn playback_honors_note_offsets_velocity_pitch_and_block_boundaries() {
        let registry = Arc::new(crate::machines::shared::BufferRegistry::new());
        registry.replace_buffer(0, vec![512, -1024, 1536, 256, -512]);
        let mut plugin = PlayPlugin {
            ram_play: RamPlay::with_registry(44100.0, registry.clone()),
            ..Default::default()
        };
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
            let mut plugin = PlayPlugin {
                ram_play: RamPlay::with_registry(44100.0, registry.clone()),
                ..Default::default()
            };
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
    fn reversed_markers_honor_midi_velocity_offsets_and_host_block_boundaries() {
        let registry = Arc::new(crate::machines::shared::BufferRegistry::new());
        registry.replace_buffer(0, vec![512, -1024, 1536, 256, -512]);
        let expected = [
            0.0, 0.0, -0.125, 0.0625, 0.375, -0.25, 0.125, 0.75, -0.5, 0.25, 0.0,
        ];
        for block_size in [1, 3, 4, 11] {
            let mut plugin = PlayPlugin {
                ram_play: RamPlay::with_registry(44100.0, registry.clone()),
                ..Default::default()
            };
            let params = Arc::get_mut(&mut plugin.params).unwrap();
            params.strt = IntParam::new("Start", 127, IntRange::Linear { min: 0, max: 127 });
            params.end = IntParam::new("End", 0, IntRange::Linear { min: 0, max: 127 });
            let mut output = Vec::new();
            for start in (0..expected.len()).step_by(block_size) {
                let len = block_size.min(expected.len() - start);
                let notes: Vec<_> = [(2, 0.5), (5, 1.0)]
                    .into_iter()
                    .filter(|(t, _)| *t >= start && *t < start + len)
                    .map(|(t, velocity)| ((t - start) as u32, velocity))
                    .collect();
                output.extend(render(&mut plugin, len, &notes));
            }
            assert_eq!(output, expected, "block size {block_size}");
        }
    }

    #[test]
    fn brr_srr_distortion_and_level_are_independent_live_controls() {
        let registry = Arc::new(crate::machines::shared::BufferRegistry::new());
        registry.replace_buffer(0, vec![256, 768, -256, -768, 256, 768, -256, -768]);
        let mut plugin = PlayPlugin {
            ram_play: RamPlay::with_registry(44100.0, registry),
            ..Default::default()
        };
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
    fn manual_audition_and_meter_are_instance_local() {
        let registry = Arc::new(crate::machines::shared::BufferRegistry::new());
        registry.replace_buffer(0, vec![512; 4]);
        let mut plugin = PlayPlugin {
            ram_play: RamPlay::with_registry(44100.0, registry),
            ..Default::default()
        };
        let other = PlayPlugin::default();
        plugin.ui.trigger.store(true, Ordering::Relaxed);
        assert_eq!(render(&mut plugin, 2, &[]), [0.25; 2]);
        assert!(plugin.ui.active.load(Ordering::Relaxed));
        assert_eq!(f32::from_bits(plugin.ui.peak.load(Ordering::Relaxed)), 0.25);
        assert!(!other.ui.active.load(Ordering::Relaxed));
        assert_eq!(other.ui.peak.load(Ordering::Relaxed), 0);
        plugin.reset();
        assert!(!plugin.ui.active.load(Ordering::Relaxed));
    }
}
