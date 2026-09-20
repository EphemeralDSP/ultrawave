//! Host-facing regression tests. Run with --no-default-features --features vst3.
use super::*;
use crate::prelude::*;
use crate::wrapper::util::hash_param_id;
use nice_plug_core::plugin::ParamValue as StoredValue;
use vst3::ComWrapper;
use vst3::Steinberg::Vst::{
    AudioBusBuffers, AudioBusBuffers__type0, IParamValueQueue, IParameterChanges,
};

#[derive(Debug, PartialEq)]
enum Mode {
    Dry,
    Driven,
}
impl Enum for Mode {
    fn variants() -> &'static [&'static str] {
        &["Dry", "Driven"]
    }
    fn ids() -> Option<&'static [&'static str]> {
        Some(&["dry-stable", "drive-stable"])
    }
    fn to_index(self) -> usize {
        usize::from(self == Self::Driven)
    }
    fn from_index(index: usize) -> Self {
        if index == 1 { Self::Driven } else { Self::Dry }
    }
}

struct TestParams {
    level: FloatParam,
    buffer: IntParam,
    enabled: BoolParam,
    mode: EnumParam<Mode>,
}
impl Default for TestParams {
    fn default() -> Self {
        Self {
            level: FloatParam::new(
                "Level",
                0.0,
                FloatRange::Linear {
                    min: -48.0,
                    max: 12.0,
                },
            ),
            buffer: IntParam::new("Buffer", -2, IntRange::Linear { min: -2, max: 7 }),
            enabled: BoolParam::new("Enabled", false),
            mode: EnumParam::new("Mode", Mode::Dry),
        }
    }
}
// SAFETY: every returned pointer belongs to this Arc-owned params instance.
unsafe impl Params for TestParams {
    fn param_map(&self) -> Vec<(String, ParamPtr, String)> {
        [
            ("level", self.level.as_ptr()),
            ("buffer", self.buffer.as_ptr()),
            ("enabled", self.enabled.as_ptr()),
            ("mode", self.mode.as_ptr()),
        ]
        .into_iter()
        .map(|(id, ptr)| (id.to_owned(), ptr, String::new()))
        .collect()
    }
}
#[derive(Default)]
struct TestPlugin<const ACCURATE: bool> {
    params: Arc<TestParams>,
}
impl<const S: bool> Plugin for TestPlugin<S> {
    const NAME: &'static str = "State test";
    const VENDOR: &'static str = "Test";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = "1.0.0";
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_output_channels: NonZeroU32::new(1),
        ..AudioIOLayout::const_default()
    }];
    const SAMPLE_ACCURATE_AUTOMATION: bool = S;
    type Editor = ();
    type SysExMessage = ();
    type BackgroundTask = ();
    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }
    fn process(
        &mut self,
        buffer: &mut Buffer,
        _: &mut AuxiliaryBuffers,
        _: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        buffer.as_slice()[0].fill(self.params.level.value());
        ProcessStatus::Normal
    }
}
impl<const S: bool> Vst3Plugin for TestPlugin<S> {
    const VST3_CLASS_ID: [u8; 16] = *b"ImmediateSaveTst";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[];
}
fn active<const S: bool>() -> Wrapper<TestPlugin<S>> {
    let wrapper = Wrapper::new();
    // SAFETY: setup is initialized and lives throughout the host call.
    unsafe {
        let mut setup: ProcessSetup = mem::zeroed();
        setup.maxSamplesPerBlock = 16;
        setup.sampleRate = 48000.0;
        assert_eq!(wrapper.setupProcessing(&mut setup), kResultOk);
        assert_eq!(wrapper.setActive(1), kResultOk);
        assert_eq!(wrapper.setProcessing(1), kResultOk);
    }
    wrapper
}
fn edit<const S: bool>(wrapper: &Wrapper<TestPlugin<S>>, id: &str, value: f64) {
    assert_eq!(
        unsafe { wrapper.setParamNormalized(hash_param_id(id), value) },
        kResultOk
    );
}
fn saved_level<const S: bool>(wrapper: &Wrapper<TestPlugin<S>>) -> f32 {
    match wrapper.inner.get_state_object().params["level"] {
        StoredValue::F32(value) => value,
        _ => panic!("wrong serialized type"),
    }
}

// The hook publishes an edit after process entry, making interleavings deterministic.
struct Queue {
    id: u32,
    points: Vec<(i32, f64)>,
    hook: Box<dyn Fn()>,
}
impl Class for Queue {
    type Interfaces = (IParamValueQueue,);
}
impl IParamValueQueueTrait for Queue {
    unsafe fn getParameterId(&self) -> u32 {
        self.id
    }
    unsafe fn getPointCount(&self) -> i32 {
        self.points.len() as i32
    }
    unsafe fn getPoint(&self, index: i32, offset: *mut i32, value: *mut f64) -> tresult {
        if index == 0 {
            (self.hook)();
        }
        // SAFETY: the wrapper supplies valid writable scalar pointers.
        unsafe {
            (*offset, *value) = self.points[index as usize];
        }
        kResultOk
    }
    unsafe fn addPoint(&self, _: i32, _: f64, _: *mut i32) -> tresult {
        kResultFalse
    }
}
struct Changes {
    queue: vst3::ComPtr<IParamValueQueue>,
}
impl Class for Changes {
    type Interfaces = (IParameterChanges,);
}
impl IParameterChangesTrait for Changes {
    unsafe fn getParameterCount(&self) -> i32 {
        1
    }
    unsafe fn getParameterData(&self, index: i32) -> *mut IParamValueQueue {
        assert_eq!(index, 0);
        self.queue.as_ptr()
    }
    unsafe fn addParameterData(&self, _: *const u32, _: *mut i32) -> *mut IParamValueQueue {
        std::ptr::null_mut()
    }
}
fn process<const S: bool>(
    wrapper: &Wrapper<TestPlugin<S>>,
    id: &str,
    points: &[(i32, f64)],
    samples: usize,
    hook: impl Fn() + 'static,
) -> Vec<f32> {
    let queue = ComWrapper::new(Queue {
        id: hash_param_id(id),
        points: points.to_vec(),
        hook: Box::new(hook),
    });
    let changes = ComWrapper::new(Changes {
        queue: queue.to_com_ptr().unwrap(),
    });
    let changes_ptr = changes.to_com_ptr::<IParameterChanges>().unwrap();
    let mut audio = vec![999.0f32; samples];
    let mut channel = audio.as_mut_ptr();
    let mut bus = AudioBusBuffers {
        numChannels: 1,
        silenceFlags: 0,
        __field0: AudioBusBuffers__type0 {
            channelBuffers32: &mut channel,
        },
    };
    // SAFETY: optional pointers are null; all active buffers and COM objects live through process.
    unsafe {
        let mut data: ProcessData = mem::zeroed();
        data.numSamples = samples as i32;
        data.numOutputs = 1;
        data.outputs = &mut bus;
        data.inputParameterChanges = changes_ptr.as_ptr();
        assert_eq!(wrapper.process(&mut data), kResultOk);
    }
    audio
}

#[test]
fn immediate_save_is_typed_without_mutating_dsp_and_load_clears_overrides() {
    let wrapper = active::<true>();
    edit(&wrapper, "level", 0.75); // -3 dB
    edit(&wrapper, "buffer", 4.0 / 9.0); // -2 + 4 = 2
    edit(&wrapper, "enabled", 1.0);
    edit(&wrapper, "mode", 1.0);
    let state = wrapper.inner.get_state_object();
    assert!(matches!(state.params["level"], StoredValue::F32(-3.0)));
    assert!(matches!(state.params["buffer"], StoredValue::I32(2)));
    assert!(matches!(state.params["enabled"], StoredValue::Bool(true)));
    assert!(matches!(&state.params["mode"], StoredValue::String(id) if id == "drive-stable"));
    assert_eq!(
        unsafe { wrapper.getParamNormalized(hash_param_id("level")) },
        0.75
    );
    let params = wrapper.inner.plugin.try_lock().unwrap().params.clone();
    assert_eq!(params.level.value(), 0.0);
    assert_eq!(params.buffer.value(), -2);
    assert!(!params.enabled.value());
    assert_eq!(params.mode.value(), Mode::Dry);
    let bytes = state::serialize_object_json(&state).unwrap();
    // SAFETY: decoding our serialized state does not access parameter pointers.
    let mut decoded = unsafe { state::deserialize_json(&bytes) }.unwrap();
    edit(&wrapper, "level", 0.25);
    assert!(wrapper.inner.set_state_inner(&mut decoded));
    assert_eq!(params.level.value(), -3.0);
    assert_eq!(params.buffer.value(), 2);
    assert!(params.enabled.value());
    assert_eq!(params.mode.value(), Mode::Driven);
    assert_eq!(saved_level(&wrapper), -3.0);
}
#[test]
fn automation_keeps_sample_offsets_and_supersedes_unmatched_edits() {
    let wrapper = active::<true>();
    edit(&wrapper, "level", 1.0);
    assert_eq!(
        process(&wrapper, "level", &[(3, 0.25), (6, 0.75)], 8, || {}),
        [0.0, 0.0, 0.0, -33.0, -33.0, -33.0, -3.0, -3.0]
    );
    assert_eq!(saved_level(&wrapper), -3.0);
}
#[test]
fn empty_unrelated_blocks_and_stop_preserve_edits_but_inactive_edits_apply() {
    let wrapper = active::<true>();
    edit(&wrapper, "level", 0.75);
    process(&wrapper, "level", &[], 8, || {});
    process(&wrapper, "buffer", &[(0, 1.0)], 8, || {});
    assert_eq!(saved_level(&wrapper), -3.0);
    unsafe {
        wrapper.setProcessing(0);
    }
    assert_eq!(saved_level(&wrapper), -3.0);
    edit(&wrapper, "level", 0.25);
    assert_eq!(
        wrapper
            .inner
            .plugin
            .try_lock()
            .unwrap()
            .params
            .level
            .value(),
        -33.0
    );
    assert_eq!(saved_level(&wrapper), -33.0);
}
#[test]
fn zero_sample_flush_and_block_rate_automation_retire_edits() {
    let wrapper = active::<true>();
    edit(&wrapper, "level", 0.75);
    process(&wrapper, "level", &[(0, 0.25)], 0, || {});
    assert_eq!(saved_level(&wrapper), -33.0);
    let wrapper = active::<false>();
    edit(&wrapper, "level", 0.75);
    assert_eq!(
        process(&wrapper, "level", &[(3, 0.25)], 8, || {}),
        [-33.0; 8]
    );
    assert_eq!(saved_level(&wrapper), -33.0);
}
#[test]
fn edit_during_block_survives_a_b_a_and_delayed_notification_yields_to_automation() {
    let wrapper = active::<true>();
    edit(&wrapper, "level", 0.75);
    let inner = wrapper.inner.clone();
    let id = hash_param_id("level");
    let audio = process(&wrapper, "level", &[(2, 0.25)], 5, move || {
        inner.set_controller_value(id, 0.5);
        inner.set_controller_value(id, 0.75);
    });
    assert_eq!(audio, [0.0, 0.0, -33.0, -33.0, -33.0]);
    assert_eq!(saved_level(&wrapper), -3.0);
    // VST3 cannot identify whether this is a delayed notification or a new edit.
    edit(&wrapper, "level", 1.0);
    assert_eq!(saved_level(&wrapper), 12.0);
    process(&wrapper, "level", &[(0, 0.5)], 5, || {});
    assert_eq!(saved_level(&wrapper), -18.0);
}

mod buses {
    use super::*;

    #[derive(Default)]
    struct BusPlugin {
        params: Arc<TestParams>,
        calls: usize,
    }
    impl Plugin for BusPlugin {
        const NAME: &'static str = "Bus test";
        const VENDOR: &'static str = "Test";
        const URL: &'static str = "";
        const EMAIL: &'static str = "";
        const VERSION: &'static str = "1.0.0";
        const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
            AudioIOLayout {
                main_input_channels: NonZeroU32::new(1),
                main_output_channels: NonZeroU32::new(1),
                aux_input_ports: &[new_nonzero_u32(2), new_nonzero_u32(1), new_nonzero_u32(1)],
                aux_output_ports: &[new_nonzero_u32(2)],
                ..AudioIOLayout::const_default()
            },
            AudioIOLayout {
                aux_input_ports: &[new_nonzero_u32(2), new_nonzero_u32(1), new_nonzero_u32(1)],
                aux_output_ports: &[new_nonzero_u32(2)],
                ..AudioIOLayout::const_default()
            },
            AudioIOLayout::const_default(),
        ];
        type Editor = ();
        type SysExMessage = ();
        type BackgroundTask = ();
        fn params(&self) -> Arc<dyn Params> {
            self.params.clone()
        }
        fn process(
            &mut self,
            buffer: &mut Buffer,
            aux: &mut AuxiliaryBuffers,
            _: &mut impl ProcessContext<Self>,
        ) -> ProcessStatus {
            self.calls += 1;
            let mut mix = [0.0; 16];
            for (input, gain) in aux.inputs.iter().zip([1.0, 10.0, 100.0]) {
                for (channel, samples) in input.as_slice_immutable().iter().enumerate() {
                    assert_eq!(samples.len(), buffer.samples());
                    for (mixed, sample) in mix.iter_mut().zip(samples.iter()) {
                        *mixed += sample * gain * (channel + 1) as f32;
                    }
                }
            }
            for samples in buffer.as_slice() {
                samples.copy_from_slice(&mix[..samples.len()]);
            }
            for output in aux.outputs.iter_mut() {
                for samples in output.as_slice() {
                    samples.copy_from_slice(&mix[..samples.len()]);
                }
            }
            ProcessStatus::Normal
        }
    }
    impl Vst3Plugin for BusPlugin {
        const VST3_CLASS_ID: [u8; 16] = *b"AuxBusBoundsTest";
        const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[];
    }

    // Own every allocation referenced by the host bus array. Truncating numInputs/numOutputs
    // leaves real allocated sentinel buses beyond the advertised count: a wrong access is
    // observable as wrong audio/a changed canary, rather than undefined behavior in the test.
    struct HostBuses {
        audio: Vec<Vec<Vec<f32>>>,
        _pointers: Vec<Vec<*mut f32>>,
        buses: Vec<AudioBusBuffers>,
    }
    impl HostBuses {
        fn new(values: &[&[f32]]) -> Self {
            let mut audio: Vec<Vec<Vec<f32>>> = values
                .iter()
                .map(|channels| channels.iter().map(|value| vec![*value; 16]).collect())
                .collect();
            let mut pointers: Vec<Vec<*mut f32>> = audio
                .iter_mut()
                .map(|channels| {
                    channels
                        .iter_mut()
                        .map(|samples| samples.as_mut_ptr())
                        .collect()
                })
                .collect();
            let buses = pointers
                .iter_mut()
                .map(|channels| AudioBusBuffers {
                    numChannels: channels.len() as i32,
                    silenceFlags: 0,
                    __field0: AudioBusBuffers__type0 {
                        channelBuffers32: channels.as_mut_ptr(),
                    },
                })
                .collect();
            Self {
                audio,
                _pointers: pointers,
                buses,
            }
        }
        fn assert_audio(&self, samples: usize, value: f32) {
            for channel in self.audio.iter().flatten() {
                assert_eq!(&channel[..samples], vec![value; samples]);
            }
        }
    }
    fn fixture() -> (Wrapper<BusPlugin>, HostBuses, HostBuses) {
        let wrapper = Wrapper::new();
        // SAFETY: initialized setup lives through setupProcessing; all calls follow host lifecycle.
        unsafe {
            let mut setup: ProcessSetup = mem::zeroed();
            setup.maxSamplesPerBlock = 16;
            setup.sampleRate = 48000.0;
            assert_eq!(wrapper.setupProcessing(&mut setup), kResultOk);
            assert_eq!(wrapper.setActive(1), kResultOk);
            assert_eq!(wrapper.setProcessing(1), kResultOk);
        }
        (
            wrapper,
            HostBuses::new(&[&[99.0], &[1.0, 2.0], &[3.0], &[4.0]]),
            HostBuses::new(&[&[-999.0], &[-999.0, -999.0]]),
        )
    }
    fn data(inputs: &mut HostBuses, outputs: &mut HostBuses, samples: i32) -> ProcessData {
        // SAFETY: zero initializes optional pointers and selects 32-bit realtime processing.
        let mut data: ProcessData = unsafe { mem::zeroed() };
        data.numSamples = samples;
        data.numInputs = inputs.buses.len() as i32;
        data.numOutputs = outputs.buses.len() as i32;
        data.inputs = inputs.buses.as_mut_ptr();
        data.outputs = outputs.buses.as_mut_ptr();
        data
    }

    #[test]
    fn arrangements_use_the_main_bus_offset_in_each_direction() {
        let wrapper = Wrapper::<BusPlugin>::new();
        // Extra allocated sentinels also make the old, reversed offsets safe to observe.
        let mut inputs = [1, 3, 1, 1, 0];
        let mut outputs = [1, 3, 0];
        // SAFETY: every advertised arrangement and the sentinels live throughout these calls.
        unsafe {
            assert_eq!(
                wrapper.setBusArrangements(inputs.as_mut_ptr(), 4, outputs.as_mut_ptr(), 2),
                kResultOk
            );
            inputs[1] = 1;
            assert_eq!(
                wrapper.setBusArrangements(inputs.as_mut_ptr(), 4, outputs.as_mut_ptr(), 2),
                kResultFalse
            );
            inputs[1] = 3;
            outputs[1] = 1;
            assert_eq!(
                wrapper.setBusArrangements(inputs.as_mut_ptr(), 4, outputs.as_mut_ptr(), 2),
                kResultFalse
            );
            outputs[1] = 3;
            assert_eq!(
                wrapper.setBusArrangements(
                    inputs.as_mut_ptr().add(1),
                    3,
                    outputs.as_mut_ptr().add(1),
                    1
                ),
                kResultOk
            );
        }
    }

    #[test]
    fn empty_arrangement_arrays_may_be_null() {
        let synth = Wrapper::<TestPlugin<false>>::new();
        let empty = Wrapper::<BusPlugin>::new();
        let mut mono = 1;
        // SAFETY: null arrays have zero length; the nonempty output array points to mono.
        unsafe {
            assert_eq!(
                synth.setBusArrangements(std::ptr::null_mut(), 0, &mut mono, 1),
                kResultOk
            );
            assert_eq!(
                empty.setBusArrangements(std::ptr::null_mut(), 0, std::ptr::null_mut(), 0),
                kResultOk
            );
            for (ins, outs) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
                assert_eq!(
                    empty.setBusArrangements(std::ptr::null_mut(), ins, std::ptr::null_mut(), outs),
                    kInvalidArgument
                );
            }
        }
    }

    #[test]
    fn all_aux_inputs_are_read_when_there_are_fewer_output_buses() {
        let (wrapper, mut inputs, mut outputs) = fixture();
        let mut data = data(&mut inputs, &mut outputs, 16);
        // SAFETY: fixture owns all bus/channel/sample arrays through process.
        assert_eq!(unsafe { wrapper.process(&mut data) }, kResultOk);
        outputs.assert_audio(16, 435.0); // 1 + 2*2 + 10*3 + 100*4
    }

    #[test]
    fn unadvertised_input_buses_are_silent_even_with_nonnull_sentinels() {
        let (wrapper, mut inputs, mut outputs) = fixture();
        let mut data = data(&mut inputs, &mut outputs, 16);
        for (count, expected) in [(2, 5.0), (1, 0.0), (0, 0.0), (4, 435.0)] {
            data.numInputs = count;
            // SAFETY: all advertised buses and additional sentinels are allocated by the fixture.
            assert_eq!(unsafe { wrapper.process(&mut data) }, kResultOk);
            outputs.assert_audio(16, expected);
        }
    }

    #[test]
    fn unadvertised_output_bus_is_untouched_and_processing_is_skipped() {
        let (wrapper, mut inputs, mut outputs) = fixture();
        let mut data = data(&mut inputs, &mut outputs, 16);
        for count in [1, 0] {
            data.numOutputs = count;
            // SAFETY: output bus 1 is allocated but not advertised; it must not be touched.
            assert_eq!(unsafe { wrapper.process(&mut data) }, kResultOk);
            assert_eq!(wrapper.inner.plugin.try_lock().unwrap().calls, 0);
            assert_eq!(outputs.audio[1], [vec![-999.0; 16], vec![-999.0; 16]]);
        }
    }

    #[test]
    fn missing_input_buses_and_channels_match_the_current_block_length() {
        let (wrapper, mut inputs, mut outputs) = fixture();
        let mut data = data(&mut inputs, &mut outputs, 16);
        // SAFETY: all pointers remain fixture-owned; null bus arrays denote missing audio.
        unsafe {
            assert_eq!(wrapper.process(&mut data), kResultOk);
            outputs.assert_audio(16, 435.0);
            let stereo = inputs.buses[1].__field0.channelBuffers32;
            inputs.buses[1].__field0.channelBuffers32 = std::ptr::null_mut();
            data.numSamples = 5;
            assert_eq!(wrapper.process(&mut data), kResultOk);
            outputs.assert_audio(5, 430.0);
            inputs.buses[1].__field0.channelBuffers32 = stereo;
            inputs.buses[1].numChannels = 1;
            data.numSamples = 11;
            assert_eq!(wrapper.process(&mut data), kResultOk);
            outputs.assert_audio(11, 431.0);
            data.inputs = std::ptr::null_mut();
            data.numSamples = 3;
            assert_eq!(wrapper.process(&mut data), kResultOk);
            outputs.assert_audio(3, 0.0);
        }
        assert_eq!(wrapper.inner.plugin.try_lock().unwrap().calls, 4);
    }

    #[test]
    fn null_output_arrays_skip_processing_without_reusing_previous_buffers() {
        let (wrapper, mut inputs, mut outputs) = fixture();
        let mut data = data(&mut inputs, &mut outputs, 16);
        // SAFETY: pointers are fixture-owned or null; the host calls are sequential.
        unsafe {
            assert_eq!(wrapper.process(&mut data), kResultOk);
            inputs.audio[1][0].fill(8.0);
            outputs.buses[1].__field0.channelBuffers32 = std::ptr::null_mut();
            assert_eq!(wrapper.process(&mut data), kResultOk);
            assert_eq!(outputs.audio[1], [vec![435.0; 16], vec![435.0; 16]]);
            data.outputs = std::ptr::null_mut();
            assert_eq!(wrapper.process(&mut data), kResultOk);
        }
        assert_eq!(wrapper.inner.plugin.try_lock().unwrap().calls, 1);
    }

    #[test]
    fn negative_process_counts_are_rejected_before_unsigned_conversion() {
        let (wrapper, _, _) = fixture();
        for (inputs, outputs, samples) in [(-1, 0, 0), (0, -1, 0), (0, 0, -1)] {
            // SAFETY: no audio arrays are supplied; the malformed counts must be rejected.
            unsafe {
                let mut data: ProcessData = mem::zeroed();
                data.numInputs = inputs;
                data.numOutputs = outputs;
                data.numSamples = samples;
                assert_eq!(wrapper.process(&mut data), kInvalidArgument);
            }
        }
        assert_eq!(wrapper.inner.plugin.try_lock().unwrap().calls, 0);
    }
}
