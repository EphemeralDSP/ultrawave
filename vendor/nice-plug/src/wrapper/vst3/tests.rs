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
