//! Small, deliberately project-specific CLAP host for exported-plugin regressions.
//!
//! Safety model: this loads only our freshly built CLAP modules, which must honor
//! CLAP's C ABI (valid function tables, NUL-terminated strings, initialized output
//! structs on success, and synchronous borrowed event/audio/stream pointers).
//! All fixture helpers execute serially on one thread; no callback is concurrent.
//! Modules outlive every Plugin and extension reference; Plugins own stable boxed
//! host tables until destroy. Test helpers accepting Modules/Plugins inherit these
//! lifetime and thread requirements. This is not a host for untrusted binaries.
#![allow(unsafe_code)]

use clap_sys::{
    audio_buffer::clap_audio_buffer,
    entry::clap_plugin_entry,
    events::*,
    ext::{
        audio_ports::{clap_audio_port_info, clap_plugin_audio_ports, CLAP_EXT_AUDIO_PORTS},
        params::{clap_param_info, clap_plugin_params, CLAP_EXT_PARAMS},
        state::{clap_plugin_state, CLAP_EXT_STATE},
    },
    factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID},
    host::clap_host,
    plugin::clap_plugin,
    process::{clap_process, CLAP_PROCESS_ERROR},
    stream::{clap_istream, clap_ostream},
    version::CLAP_VERSION,
};
use libloading::{Library, Symbol};
use std::{
    ffi::{c_char, c_void, CStr, CString},
    mem::{self, MaybeUninit},
    path::{Path, PathBuf},
    ptr,
    time::{Duration, Instant},
};

static HOST_NAME: &CStr = c"Ultrawave bounded test host";
unsafe extern "C" fn no_extension(_: *const clap_host, _: *const c_char) -> *const c_void {
    ptr::null()
}
unsafe extern "C" fn host_noop(_: *const clap_host) {}

struct Module {
    _library: Library,
    entry: *const clap_plugin_entry,
}
impl Module {
    // SAFETY: caller supplies our CLAP cdylib. Keep Library alive for every symbol
    // and every created plugin; the temporary path is borrowed only by init().
    unsafe fn load(path: &Path) -> Self {
        let library = Library::new(path).unwrap_or_else(|e| panic!("load {}: {e}", path.display()));
        let entry = {
            let symbol: Symbol<*const clap_plugin_entry> =
                library.get(b"clap_entry\0").expect("clap_entry");
            *symbol
        };
        let path = CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
        assert!((*entry).init.unwrap()(path.as_ptr()), "entry init");
        Self {
            _library: library,
            entry,
        }
    }
    // SAFETY: self is initialized and must outlive the returned Plugin. Factory
    // count is checked before descriptor 0; CLAP guarantees that descriptor.
    unsafe fn create(&self) -> Plugin {
        let factory = ((*self.entry).get_factory.unwrap()(CLAP_PLUGIN_FACTORY_ID.as_ptr()))
            as *const clap_plugin_factory;
        assert!(!factory.is_null());
        assert_eq!((*factory).get_plugin_count.unwrap()(factory), 1);
        let desc = (*factory).get_plugin_descriptor.unwrap()(factory, 0);
        let host = Box::new(clap_host {
            clap_version: CLAP_VERSION,
            host_data: ptr::null_mut(),
            name: HOST_NAME.as_ptr(),
            vendor: HOST_NAME.as_ptr(),
            url: c"".as_ptr(),
            version: c"1".as_ptr(),
            get_extension: Some(no_extension),
            request_restart: Some(host_noop),
            request_process: Some(host_noop),
            request_callback: Some(host_noop),
        });
        let raw = (*factory).create_plugin.unwrap()(factory, &*host, (*desc).id);
        assert!(!raw.is_null() && (*raw).init.unwrap()(raw));
        Plugin {
            raw,
            _host: host,
            active: false,
        }
    }
}
impl Drop for Module {
    fn drop(&mut self) {
        // SAFETY: fixtures destroy all plugins first; Library is still loaded.
        unsafe { (*self.entry).deinit.unwrap()() }
    }
}

struct Plugin {
    raw: *const clap_plugin,
    _host: Box<clap_host>,
    active: bool,
}
impl Plugin {
    // SAFETY: caller pairs the exact CLAP extension ID with its ABI struct T.
    // The returned table belongs to this live plugin and is checked for null.
    unsafe fn ext<T>(&self, id: &CStr) -> &T {
        let extension = (*self.raw).get_extension.unwrap()(self.raw, id.as_ptr()) as *const T;
        assert!(!extension.is_null(), "required extension {id:?}");
        &*extension
    }
    unsafe fn params(&self) -> &clap_plugin_params {
        self.ext(CLAP_EXT_PARAMS)
    }
    unsafe fn param_names(&self) -> Vec<String> {
        let ext = self.params();
        (0..ext.count.unwrap()(self.raw))
            .map(|index| {
                let mut info = MaybeUninit::zeroed();
                assert!(ext.get_info.unwrap()(self.raw, index, info.as_mut_ptr()));
                CStr::from_ptr(info.assume_init().name.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }
    unsafe fn param(&self, name: &str) -> clap_param_info {
        // SAFETY: indices are below count; get_info initializes the POD output on
        // success and its name is NUL-terminated under the CLAP ABI contract.
        let ext = self.params();
        for i in 0..ext.count.unwrap()(self.raw) {
            let mut info = MaybeUninit::zeroed();
            assert!(ext.get_info.unwrap()(self.raw, i, info.as_mut_ptr()));
            let info = info.assume_init();
            if CStr::from_ptr(info.name.as_ptr()).to_string_lossy() == name {
                return info;
            }
        }
        panic!("parameter {name:?} not exported")
    }
    unsafe fn text_value(&self, name: &str, text: &str) -> f64 {
        // SAFETY: valid parameter ID and owned C string; conversion destinations
        // remain live for each synchronous call and output capacity is explicit.
        let info = self.param(name);
        let text = CString::new(text).unwrap();
        let mut value = 0.0;
        assert!(self.params().text_to_value.unwrap()(
            self.raw,
            info.id,
            text.as_ptr(),
            &mut value
        ));
        let mut shown = [0i8; 64];
        assert!(self.params().value_to_text.unwrap()(
            self.raw,
            info.id,
            value,
            shown.as_mut_ptr(),
            64
        ));
        value
    }
    unsafe fn value(&self, name: &str) -> f64 {
        // SAFETY: a valid ID and writable f64 borrowed for this call only.
        let mut value = 0.0;
        assert!(self.params().get_value.unwrap()(
            self.raw,
            self.param(name).id,
            &mut value,
        ));
        value
    }
    unsafe fn flush(&self, values: &[(&str, &str)]) {
        // SAFETY: events, context and lists stay alive until synchronous flush
        // returns. No process call runs concurrently, including when activated.
        let events: Vec<Event> = values
            .iter()
            .map(|&(name, text)| Event::param(0, self.param(name), self.text_value(name, text)))
            .collect();
        let mut ctx = EventContext::new(&events);
        let input = event_list(&mut ctx);
        let output = output_events();
        self.params().flush.unwrap()(self.raw, &input, &output);
        (*self.raw).on_main_thread.unwrap()(self.raw);
    }
    unsafe fn activate(&mut self, rate: f64, max: u32) {
        // SAFETY: initialized inactive plugin; fixtures use finite positive rates
        // and process sizes within [1,max]. Calls follow CLAP lifecycle order.
        assert!(!self.active);
        assert!((*self.raw).activate.unwrap()(self.raw, rate, 1, max));
        assert!((*self.raw).start_processing.unwrap()(self.raw));
        self.active = true;
    }
    unsafe fn stop(&mut self) {
        // SAFETY: active tracks successful activate/start; no callback is in flight.
        if self.active {
            (*self.raw).stop_processing.unwrap()(self.raw);
            (*self.raw).deactivate.unwrap()(self.raw);
            self.active = false;
        }
    }
    unsafe fn reset(&self) {
        // SAFETY: called only between synchronous process calls on active plugins.
        (*self.raw).reset.unwrap()(self.raw)
    }
    unsafe fn ports(&self, input: bool) -> Vec<clap_audio_port_info> {
        // SAFETY: exact extension type, indices below count, POD initialized on success.
        let ext: &clap_plugin_audio_ports = self.ext(CLAP_EXT_AUDIO_PORTS);
        (0..ext.count.unwrap()(self.raw, input))
            .map(|i| {
                let mut x = MaybeUninit::zeroed();
                assert!(ext.get.unwrap()(self.raw, i, input, x.as_mut_ptr()));
                x.assume_init()
            })
            .collect()
    }
    unsafe fn save(&self) -> Vec<u8> {
        unsafe extern "C" fn write(s: *const clap_ostream, p: *const c_void, n: u64) -> i64 {
            // SAFETY: save borrows the stream/context below synchronously; CLAP
            // supplies n readable bytes. No other access to data occurs in save.
            if n == 0 {
                return 0;
            }
            let v = &mut *((*s).ctx as *mut Vec<u8>);
            v.extend_from_slice(std::slice::from_raw_parts(p as *const u8, n as usize));
            n as i64
        }
        let mut data = Vec::new();
        let stream = clap_ostream {
            ctx: &mut data as *mut _ as *mut _,
            write: Some(write),
        };
        assert!(self.ext::<clap_plugin_state>(CLAP_EXT_STATE).save.unwrap()(
            self.raw, &stream
        ));
        data
    }
    unsafe fn load_state(&self, data: &[u8]) {
        struct Reader<'a> {
            data: &'a [u8],
            at: usize,
        }
        unsafe extern "C" fn read(s: *const clap_istream, p: *mut c_void, n: u64) -> i64 {
            // SAFETY: load borrows the live Reader below exclusively. CLAP gives
            // n writable bytes disjoint from our input; clamp reads to remaining.
            if n == 0 {
                return 0;
            }
            let r = &mut *((*s).ctx as *mut Reader);
            let n = (n as usize).min(r.data.len() - r.at);
            ptr::copy_nonoverlapping(r.data.as_ptr().add(r.at), p as *mut u8, n);
            r.at += n;
            n as i64
        }
        let mut r = Reader { data, at: 0 };
        let stream = clap_istream {
            ctx: &mut r as *mut _ as *mut _,
            read: Some(read),
        };
        assert!(self.ext::<clap_plugin_state>(CLAP_EXT_STATE).load.unwrap()(
            self.raw, &stream
        ));
        (*self.raw).on_main_thread.unwrap()(self.raw);
    }
    unsafe fn process(
        &self,
        input: [&[f32]; 4],
        frames: usize,
        events: &[Event],
        transport: &clap_event_transport,
    ) -> [Vec<f32>; 2] {
        // SAFETY: local Vecs own disjoint channel storage, pointer arrays and all
        // process/event/transport structs survive the synchronous call. Empty
        // inputs are omitted entirely; advertised channels have frames elements.
        assert!(input.iter().all(|x| x.is_empty()) || input.iter().all(|x| x.len() == frames));
        let mut ins: [Vec<f32>; 4] = input.map(|x| x.to_vec());
        let mut out = [vec![0.0; frames], vec![0.0; frames]];
        let mut main_ptr = [ins[0].as_mut_ptr(), ins[1].as_mut_ptr()];
        let mut aux_ptr = [ins[2].as_mut_ptr(), ins[3].as_mut_ptr()];
        let mut out_ptr = [out[0].as_mut_ptr(), out[1].as_mut_ptr()];
        let in_buf = [audio(&mut main_ptr), audio(&mut aux_ptr)];
        let mut out_buf = [audio(&mut out_ptr)];
        let mut ctx = EventContext::new(events);
        let event_list = event_list(&mut ctx);
        let output = output_events();
        let p = clap_process {
            steady_time: -1, // This offline host does not provide a steady clock.
            frames_count: frames as u32,
            transport,
            audio_inputs: in_buf.as_ptr(),
            audio_outputs: out_buf.as_mut_ptr(),
            audio_inputs_count: if input[0].is_empty() { 0 } else { 2 },
            audio_outputs_count: 1,
            in_events: &event_list,
            out_events: &output,
        };
        assert_ne!(
            (*self.raw).process.unwrap()(self.raw, &p),
            CLAP_PROCESS_ERROR
        );
        out
    }
}
impl Drop for Plugin {
    fn drop(&mut self) {
        // SAFETY: Module and host table still live; stop before destroying once.
        unsafe {
            self.stop();
            (*self.raw).destroy.unwrap()(self.raw)
        }
    }
}

fn audio(ptrs: &mut [*mut f32; 2]) -> clap_audio_buffer {
    clap_audio_buffer {
        data32: ptrs.as_mut_ptr(),
        data64: ptr::null_mut(),
        channel_count: 2,
        latency: 0,
        constant_mask: 0,
    }
}
#[repr(C)]
enum Event {
    Param(clap_event_param_value),
    Note(clap_event_note),
}
impl Event {
    fn param(time: u32, info: clap_param_info, value: f64) -> Self {
        Self::Param(clap_event_param_value {
            header: header::<clap_event_param_value>(time, CLAP_EVENT_PARAM_VALUE),
            param_id: info.id,
            cookie: info.cookie,
            note_id: -1,
            port_index: -1,
            channel: -1,
            key: -1,
            value,
        })
    }
    fn note(time: u32, key: i16, velocity: f64) -> Self {
        Self::Note(clap_event_note {
            header: header::<clap_event_note>(time, CLAP_EVENT_NOTE_ON),
            note_id: -1,
            port_index: 0,
            channel: 0,
            key,
            velocity,
        })
    }
    fn note_off(time: u32, key: i16) -> Self {
        Self::Note(clap_event_note {
            header: header::<clap_event_note>(time, CLAP_EVENT_NOTE_OFF),
            note_id: -1,
            port_index: 0,
            channel: 0,
            key,
            velocity: 1.0,
        })
    }
    fn header(&self) -> *const clap_event_header {
        match self {
            Self::Param(x) => &x.header,
            Self::Note(x) => &x.header,
        }
    }
}
fn header<T>(time: u32, type_: u16) -> clap_event_header {
    clap_event_header {
        size: mem::size_of::<T>() as u32,
        time,
        space_id: CLAP_CORE_EVENT_SPACE_ID,
        type_,
        flags: 0,
    }
}
struct EventContext {
    ptr: *const Event,
    len: usize,
}
impl EventContext {
    fn new(events: &[Event]) -> Self {
        Self {
            ptr: events.as_ptr(),
            len: events.len(),
        }
    }
}
unsafe extern "C" fn event_size(l: *const clap_input_events) -> u32 {
    // SAFETY: only invoked synchronously with event_list's live EventContext.
    (*((*l).ctx as *const EventContext)).len as u32
}
unsafe extern "C" fn event_get(l: *const clap_input_events, i: u32) -> *const clap_event_header {
    // SAFETY: same live context; bounds checked before indexing the borrowed
    // Event slice. Each variant's header starts its repr(C) CLAP event payload.
    let c = &*((*l).ctx as *const EventContext);
    if i as usize >= c.len {
        ptr::null()
    } else {
        (*c.ptr.add(i as usize)).header()
    }
}
fn event_list(ctx: &mut EventContext) -> clap_input_events {
    clap_input_events {
        ctx: ctx as *mut _ as *mut _,
        size: Some(event_size),
        get: Some(event_get),
    }
}
unsafe extern "C" fn discard(_: *const clap_output_events, _: *const clap_event_header) -> bool {
    true
}
fn output_events() -> clap_output_events {
    clap_output_events {
        ctx: ptr::null_mut(),
        try_push: Some(discard),
    }
}
fn transport(playing: bool, tempo: f64, pos: i64, looping: bool) -> clap_event_transport {
    clap_event_transport {
        header: header::<clap_event_transport>(0, CLAP_EVENT_TRANSPORT),
        flags: CLAP_TRANSPORT_HAS_TEMPO
            | CLAP_TRANSPORT_HAS_SECONDS_TIMELINE
            | if playing {
                CLAP_TRANSPORT_IS_PLAYING
            } else {
                0
            }
            | if looping {
                CLAP_TRANSPORT_IS_LOOP_ACTIVE
            } else {
                0
            },
        song_pos_beats: 0,
        song_pos_seconds: pos,
        tempo,
        tempo_inc: 0.0,
        loop_start_beats: 0,
        loop_end_beats: 0,
        loop_start_seconds: 0,
        loop_end_seconds: 0,
        bar_start: 0,
        bar_number: 0,
        tsig_num: 4,
        tsig_denom: 4,
    }
}

fn approx(actual: f32, expected: f32, message: &str) {
    assert!(
        (actual - expected).abs() < 0.0006,
        "{message}: got {actual}, expected {expected}"
    );
}

fn quantize(sample: f32) -> f32 {
    (sample * 2048.0).round().clamp(-2048.0, 2047.0) / 2048.0
}

unsafe fn configure(plugin: &Plugin) {
    plugin.flush(&[
        ("Master Mode", "Off"),
        ("Track Input Level", "127"),
        ("Track Input Balance", "L64"),
        ("Sidechain Level", "127"),
        ("Sidechain Balance", "R64"),
        ("Capture Length", "INF"),
        ("Capture Rate", "HI"),
        ("Capture Level", "0 dB"),
        ("Start", "0"),
        ("End", "127"),
        ("Pitch", "+0 st"),
        ("Hold", "OFF"),
        ("Decay", "OFF"),
        ("Retrigger", "OFF"),
        ("Retrig Time", "OFF"),
        ("Playback Level", "0 dB"),
    ]);
}

unsafe fn contract_and_sample_only_output(module: &Module) {
    let factory = ((*module.entry).get_factory.unwrap()(CLAP_PLUGIN_FACTORY_ID.as_ptr()))
        as *const clap_plugin_factory;
    let descriptor = (*factory).get_plugin_descriptor.unwrap()(factory, 0);
    assert_eq!(
        CStr::from_ptr((*descriptor).id).to_bytes(),
        b"com.ephemeraldsp.ultrawave"
    );
    assert_eq!(CStr::from_ptr((*descriptor).name).to_bytes(), b"Ultrawave");

    let mut plugin = module.create();
    assert_eq!(
        plugin
            .ports(true)
            .iter()
            .map(|port| port.channel_count)
            .collect::<Vec<_>>(),
        [2, 2],
        "main and side inputs must both be stereo"
    );
    assert_eq!(
        plugin
            .ports(false)
            .iter()
            .map(|p| p.channel_count)
            .collect::<Vec<_>>(),
        [2]
    );
    let names = plugin.param_names();
    assert_eq!(names.len(), 27, "single-sample plugin parameter count");
    for name in [
        "Master Mode",
        "Record Once",
        "Capture Note",
        "Track Input Level",
        "Track Input Balance",
        "Capture Length",
        "Capture Rate",
    ] {
        assert!(names.iter().any(|actual| actual == name), "missing {name}");
    }
    for removed in [
        "Buffer",
        "Playback Only",
        "One Shot",
        "Record Note",
        "Main Level",
        "Main Balance",
        "Rec Length",
        "Rec Rate",
    ] {
        assert!(
            !names.iter().any(|actual| actual == removed),
            "obsolete parameter name still exported: {removed}"
        );
    }
    for (name, expected) in [
        ("Capture Note", 48.0),
        ("Track Input Level", 127.0),
        ("Track Input Balance", 64.0),
        ("Capture Length", 48.0),
        ("Capture Rate", 127.0),
    ] {
        assert_eq!(plugin.value(name).round(), expected, "default {name}");
    }
    assert_eq!(plugin.value("Record Once"), 0.0);
    assert_eq!(plugin.value("Play Note").round(), 50.0);
    assert_eq!(plugin.value("Master Mode"), 1.0);
    plugin.activate(48000.0, 512);
    let master_l = [0.125, -0.75, 0.0, 0.25, -0.125];
    let master_r = [-0.25, 0.5, 0.75, 0.0, -0.5];
    let side = [0.875; 5];
    assert_eq!(
        plugin.process(
            [&master_l, &master_r, &side, &side],
            5,
            &[],
            &transport(true, 120.0, 0, false),
        ),
        [master_l.to_vec(), master_r.to_vec()],
        "default Master mode must pass stereo input unchanged, without sidechain monitoring"
    );
    plugin.stop();
    configure(&plugin);
    plugin.activate(48000.0, 512);

    let left: Vec<_> = (0..17).map(|i| (i as f32 - 8.0) * 0.073).collect();
    let right: Vec<_> = (0..17).map(|i| (3.0 - i as f32) * 0.047).collect();
    let side = vec![0.875; 17];
    let tr = transport(true, 120.0, 0, false);
    assert_eq!(
        plugin.process([&left, &right, &side, &side], 17, &[], &tr),
        [vec![0.0; 17], vec![0.0; 17]],
        "active exported effect must never pass dry main or side input"
    );

    // The runner deliberately supplies a valid screenshot WAV environment
    // variable. Only the standalone may preload it; exported effects start empty.
    let silence = [0.0; 512];
    assert_eq!(
        plugin.process([&silence; 4], 512, &[Event::note(0, 50, 1.0)], &tr),
        [vec![0.0; 512], vec![0.0; 512]],
        "an exported effect must not load screenshot audio"
    );

    // Capture while every input bus is nonzero and asymmetric. Output remains
    // silent, while the configured hard pans preserve main L + side R in RAM.
    let capture_main_l = vec![0.125; 400];
    let capture_main_r = vec![-0.75; 400];
    let capture_side_l = vec![0.625; 400];
    let capture_side_r = vec![0.375; 400];
    let captured = plugin.process(
        [
            &capture_main_l,
            &capture_main_r,
            &capture_side_l,
            &capture_side_r,
        ],
        400,
        &[Event::note(0, 48, 1.0)],
        &tr,
    );
    assert_eq!(captured, [vec![0.0; 400], vec![0.0; 400]]);
    plugin.reset();

    // An event beyond the plugin's internal 64-frame mixing chunk proves that
    // Playback Level's 5 ms smoothing remains sample accurate. Independent,
    // nonzero host input must never contribute to the played output.
    let level_event = Event::param(
        73,
        plugin.param("Playback Level"),
        plugin.text_value("Playback Level", "-6 dB"),
    );
    let host_l = vec![0.9; 400];
    let host_r = vec![-0.8; 400];
    let host_side_l = vec![0.7; 400];
    let host_side_r = vec![-0.6; 400];
    let output = plugin.process(
        [&host_l, &host_r, &host_side_l, &host_side_r],
        400,
        &[Event::note(0, 50, 1.0), level_event],
        &tr,
    );
    let sample = quantize(0.125 + 0.375);
    let target_gain = 10.0f32.powf(-6.0 / 20.0);
    for (i, (&left, &right)) in output[0].iter().zip(&output[1]).enumerate() {
        let gain = if i < 73 {
            1.0
        } else {
            1.0 - (1.0 - target_gain) * ((i - 73 + 1) as f32 / 240.0).min(1.0)
        };
        let expected = sample * gain;
        approx(
            left,
            expected,
            &format!("Playback Level automation frame {i}"),
        );
        approx(
            right,
            expected,
            &format!("Playback Level automation R frame {i}"),
        );
    }
}

unsafe fn master_mode_automation_during_capture(module: &Module) {
    let mut plugin = module.create();
    configure(&plugin);
    plugin.activate(48000.0, 1025);
    let tr = transport(true, 120.0, 0, false);
    let silent = vec![0.0; 1025];
    let seed = vec![0.25; 1025];
    plugin.process(
        [&seed, &silent, &silent, &silent],
        1025,
        &[Event::note(0, 48, 1.0)],
        &tr,
    );
    plugin.flush(&[("Master Mode", "On")]);
    plugin.reset();
    let frames = 800;
    let left = vec![0.125; frames];
    let right = vec![-0.5; frames];
    let silence = &silent[..frames];
    let output = plugin.process(
        [&left, &right, silence, silence],
        frames,
        &[
            Event::note(0, 50, 1.0),
            Event::note(0, 48, 1.0),
            Event::param(73, plugin.param("Master Mode"), 0.0),
            Event::param(337, plugin.param("Master Mode"), 1.0),
        ],
        &tr,
    );
    // Both edges fall beyond a 64-frame mix chunk. Switching mode ramps live
    // routing over 240 frames, but keeps the capture's latched staging strategy.
    let gain = |i: usize| {
        if i < 73 {
            1.0
        } else if i < 337 {
            1.0 - ((i - 73 + 1) as f32 / 240.0).min(1.0)
        } else {
            ((i - 337 + 1) as f32 / 240.0).min(1.0)
        }
    };
    for (i, (&left, &right)) in output[0].iter().zip(&output[1]).enumerate() {
        approx(left, 0.125 * gain(i) + 0.25, "Master mode dry L ramp");
        approx(right, -0.5 * gain(i) + 0.25, "Master mode dry R ramp");
    }
    plugin.reset(); // Publish the nonempty interrupted staged take.
    let recorded = plugin.process([silence; 4], frames, &[Event::note(0, 50, 1.0)], &tr);
    for (i, (&left, &right)) in recorded[0].iter().zip(&recorded[1]).enumerate() {
        approx(
            left,
            quantize(0.125 + 0.25 * gain(i)),
            "Master mode feedback ramp",
        );
        assert_eq!(left, right);
    }
}

unsafe fn master_resamples_playback(module: &Module, rate: f64, block: usize) {
    let tr = transport(true, 120.0, 0, false);
    let render = |plugin: &Plugin, input: &[Vec<f32>; 4], notes: &[(usize, i16, f64)]| {
        let mut output = [Vec::new(), Vec::new()];
        for start in (0..input[0].len()).step_by(block) {
            let end = (start + block).min(input[0].len());
            let events: Vec<_> = notes
                .iter()
                .filter(|(at, _, _)| (start..end).contains(at))
                .map(|&(at, key, velocity)| Event::note((at - start) as u32, key, velocity))
                .collect();
            let chunk = plugin.process(
                input.each_ref().map(|channel| &channel[start..end]),
                end - start,
                &events,
                &tr,
            );
            for (out, samples) in output.iter_mut().zip(chunk) {
                out.extend(samples);
            }
        }
        output
    };
    let mut plugin = module.create();
    configure(&plugin);
    plugin.flush(&[("Master Mode", "On"), ("Sidechain Level", "0")]);
    plugin.activate(rate, block as u32);
    let duration = (rate / 4.0) as usize; // Half a beat at 120 BPM.
    let seed: [Vec<f32>; 4] = std::array::from_fn(|channel| {
        (0..duration * 4)
            .map(|i| {
                if channel == 0 {
                    ((i * 37 % 1700) as f32 - 850.0) / 2048.0
                } else {
                    0.0
                }
            })
            .collect()
    });
    assert_eq!(render(&plugin, &seed, &[(0, 48, 1.0)])[0], seed[0]);
    plugin.reset(); // Finish the first, deliberately longer take.
    plugin.flush(&[
        ("Capture Length", "0"),
        ("Track Input Balance", "L-R"),
        ("Sidechain Level", "127"),
        ("Pitch", "+12 st"),
        ("Playback Level", "-6 dB"),
        ("Playback Distortion", "32"),
        ("Bit Reduction", "24"),
    ]);
    plugin.reset();
    let capture_at = 73;
    let end = capture_at + duration;
    let frames = end + 91;
    let silent = std::array::from_fn(|_| vec![0.0; frames]);
    // Separate playback establishes the post-pitch/FX/velocity/level signal.
    // Capturing must neither erase it early nor tap a pre-processing version.
    let reference = render(&plugin, &silent, &[(5, 50, 0.625)]);
    assert!(reference[0][capture_at..end].iter().any(|s| s.abs() > 0.01));
    plugin.reset();
    let input = [
        vec![0.25; frames],
        vec![-0.125; frames],
        vec![-0.75; frames],
        vec![0.0625; frames],
    ];
    let captured = render(&plugin, &input, &[(5, 50, 0.625), (capture_at, 48, 1.0)]);
    for i in 0..frames {
        let played = if i < end { reference[0][i] } else { 0.0 };
        approx(
            captured[0][i],
            0.25 + played,
            "master capture keeps old playback until publication",
        );
        approx(
            captured[1][i],
            -0.125 + played,
            "master preserves dry stereo during resampling",
        );
    }
    plugin.flush(&[
        ("Pitch", "+0 st"),
        ("Playback Level", "0 dB"),
        ("Playback Distortion", "0"),
        ("Bit Reduction", "0"),
    ]);
    plugin.reset();
    let replacement = render(&plugin, &silent, &[(0, 50, 1.0)]);
    for i in 0..frames {
        let expected = if i < duration {
            // Centered main input = 0.0625; hard-right side = 0.0625.
            quantize(0.125 + reference[0][capture_at + i])
        } else {
            0.0
        };
        approx(
            replacement[0][i],
            expected,
            "new take contains master sum, post playback processing",
        );
        assert_eq!(replacement[0][i], replacement[1][i]);
    }
    plugin.reset();
    // Record the sample again with silent host input. The next generation must
    // retain it, with no within-take read/write feedback or automatic restart.
    let generation = render(&plugin, &silent, &[(0, 50, 1.0), (0, 48, 1.0)]);
    assert_eq!(generation, replacement);
    plugin.reset();
    assert_eq!(render(&plugin, &silent, &[(0, 50, 1.0)]), replacement);

    plugin.flush(&[("Record Once", "On")]);
    plugin.reset();
    assert_eq!(
        render(&plugin, &silent, &[(0, 50, 1.0), (0, 48, 1.0)]),
        replacement
    );
    plugin.reset();
    let denied = render(&plugin, &input, &[(0, 48, 1.0)]);
    assert_eq!(denied, [input[0].clone(), input[1].clone()]);
    plugin.reset();
    assert_eq!(render(&plugin, &silent, &[(0, 50, 1.0)]), replacement);
    let state = plugin.save();
    let mut restored = module.create();
    restored.load_state(&state);
    assert_eq!(restored.value("Master Mode"), 1.0);
    assert_eq!(restored.value("Record Once"), 1.0);
    restored.activate(rate, block as u32);
    assert_eq!(render(&restored, &silent, &[(0, 50, 1.0)]), silent[..2]);
    render(&restored, &input, &[(0, 48, 1.0)]);
    restored.reset();
    let fresh = render(&restored, &silent, &[(0, 50, 1.0)]);
    assert!(fresh[0][..duration].iter().all(|&s| s == 0.125));
    assert!(fresh[0][duration..].iter().all(|&s| s == 0.0));

    // Main level scales the whole master capture source, including the sample,
    // but does not attenuate the audible master bus or the sample's output.
    plugin.flush(&[
        ("Record Once", "Off"),
        ("Track Input Level", "0"),
        ("Sidechain Level", "0"),
    ]);
    plugin.reset();
    assert_eq!(
        render(&plugin, &silent, &[(0, 50, 1.0), (0, 48, 1.0)]),
        replacement
    );
    plugin.reset();
    assert_eq!(render(&plugin, &silent, &[(0, 50, 1.0)]), silent[..2]);
}

unsafe fn capture_playback_at_rate(module: &Module, rate: f64, block: usize) {
    let mut plugin = module.create();
    configure(&plugin);
    plugin.activate(rate, block as u32);
    let tr = transport(true, 120.0, 0, false);
    let main_l: Vec<_> = (0..block).map(|i| (i as f32 - 11.0) / 64.0).collect();
    let main_r = vec![0.8125; block];
    let side_l = vec![-0.6875; block];
    let side_r: Vec<_> = (0..block).map(|i| ((i % 11) as f32 - 5.0) / 64.0).collect();
    let at = block / 3;
    let monitored = plugin.process(
        [&main_l, &main_r, &side_l, &side_r],
        block,
        &[Event::note(at as u32, 48, 1.0)],
        &tr,
    );
    assert_eq!(monitored, [vec![0.0; block], vec![0.0; block]]);
    plugin.reset();

    let play_at = 5;
    let playback_main_l = vec![0.9375; block];
    let playback_main_r = vec![-0.8125; block];
    let playback_side_l = vec![0.6875; block];
    let playback_side_r = vec![-0.5625; block];
    let played = plugin.process(
        [
            &playback_main_l,
            &playback_main_r,
            &playback_side_l,
            &playback_side_r,
        ],
        block,
        &[Event::note(play_at as u32, 50, 0.5)],
        &tr,
    );
    assert!(played[0][..play_at].iter().all(|&x| x == 0.0));
    for (i, (&left, &right)) in played[0].iter().zip(&played[1]).enumerate().skip(play_at) {
        let source = at + i - play_at;
        let expected = if source < block {
            quantize(main_l[source] + side_r[source]) * 0.5
        } else {
            0.0
        };
        assert_eq!(left, expected, "{rate} Hz source frame {i}");
        assert_eq!(right, expected);
    }
}

unsafe fn reverse_pitch_notes_and_triggers(module: &Module) {
    let mut plugin = module.create();
    configure(&plugin);
    plugin.flush(&[("Capture Note", "61"), ("Play Note", "63")]);
    plugin.activate(48000.0, 64);
    let tr = transport(true, 120.0, 0, false);
    let source = [0.125, -0.25, 0.375, -0.5, 0.625];
    let zero = [0.0; 5];
    plugin.process(
        [&source, &zero, &zero, &zero],
        5,
        &[Event::note(0, 61, 1.0)],
        &tr,
    );
    plugin.reset();

    // Nearby MIDI notes do not transpose: only the assigned key triggers.
    let ignored = plugin.process(
        [&[0.0; 8]; 4],
        8,
        &[
            Event::note(0, 62, 1.0),
            Event::note(1, 63, 0.0),
            Event::note_off(2, 63),
        ],
        &tr,
    );
    assert_eq!(ignored, [vec![0.0; 8], vec![0.0; 8]]);

    plugin.flush(&[("Start", "127"), ("End", "0")]);
    let reverse = plugin.process([&[0.0; 8]; 4], 8, &[Event::note(2, 63, 0.25)], &tr);
    let expected = [0.0, 0.0, 0.15625, -0.125, 0.09375, -0.0625, 0.03125, 0.0];
    assert_eq!(reverse[0], expected);
    assert_eq!(reverse[1], expected);

    plugin.flush(&[("Start", "0"), ("End", "127"), ("Pitch", "+12 st")]);
    let pitched = plugin.process([&[0.0; 5]; 4], 5, &[Event::note(0, 63, 1.0)], &tr);
    assert_eq!(pitched[0], [0.125, 0.375, 0.625, 0.0, 0.0]);

    // Boolean controls are rising-edge triggers, not level gates.
    plugin.flush(&[("Pitch", "+0 st"), ("Playback", "On")]);
    let first = plugin.process([&[0.0; 3]; 4], 3, &[], &tr);
    let held = plugin.process([&[0.0; 3]; 4], 3, &[], &tr);
    assert_eq!(first[0], source[..3]);
    assert_eq!(held[0], [source[3], source[4], 0.0]);
}

unsafe fn capture_control_is_a_rising_edge(module: &Module) {
    let mut plugin = module.create();
    configure(&plugin);
    plugin.activate(48000.0, 128);
    let tr = transport(true, 120.0, 0, false);
    let first: Vec<_> = (0..100).map(|i| i as f32 / 512.0).collect();
    let second = [0.375; 8];
    let zero100 = [0.0; 100];
    let zero8 = [0.0; 8];
    let capture_on = Event::param(
        70,
        plugin.param("Capture"),
        plugin.text_value("Capture", "On"),
    );
    plugin.process(
        [&first, &zero100, &zero100, &zero100],
        100,
        &[capture_on],
        &tr,
    );
    // Holding Capture high across a block must continue, not restart, the take.
    plugin.process([&second, &zero8, &zero8, &zero8], 8, &[], &tr);
    plugin.flush(&[("Capture", "Off")]);
    plugin.reset();
    let silence = [0.0; 38];
    let played = plugin.process([&silence; 4], 38, &[Event::note(0, 50, 1.0)], &tr);
    let expected = first[70..]
        .iter()
        .chain(second.iter())
        .copied()
        .map(quantize)
        .collect::<Vec<_>>();
    assert_eq!(played[0], expected);
}

unsafe fn record_priority_and_no_feedback(module: &Module) {
    let mut plugin = module.create();
    configure(&plugin);
    plugin.flush(&[("Capture Note", "60"), ("Play Note", "60")]);
    plugin.activate(48000.0, 64);
    let tr = transport(true, 120.0, 0, false);
    let quarter = [0.25; 32];
    let zero = [0.0; 32];
    let out = plugin.process(
        [&quarter, &zero, &zero, &zero],
        32,
        &[Event::note(0, 60, 1.0)],
        &tr,
    );
    assert_eq!(
        out,
        [vec![0.0; 32], vec![0.0; 32]],
        "Record wins equal note assignment"
    );
    plugin.reset();
    plugin.flush(&[("Play Note", "62")]);
    let take = plugin.process([&zero[..8]; 4], 8, &[Event::note(0, 62, 1.0)], &tr);
    assert_eq!(
        take[0],
        quarter[..8]
            .iter()
            .copied()
            .map(quantize)
            .collect::<Vec<_>>()
    );

    // Replace the single sample with silence at an exact nonzero MIDI offset
    // while its old voice is still active. Capture must read host input before
    // output and invalidate the outgoing voice exactly at the trigger frame.
    plugin.flush(&[("Capture Note", "61")]);
    let replacement = plugin.process([&zero; 4], 32, &[Event::note(9, 61, 1.0)], &tr);
    let mut expected = vec![0.0; 32];
    expected[..9].fill(quantize(0.25));
    assert_eq!(replacement, [expected.clone(), expected]);
    plugin.reset();
    let replaced = plugin.process([&zero; 4], 32, &[Event::note(0, 62, 1.0)], &tr);
    assert_eq!(
        replaced,
        [vec![0.0; 32], vec![0.0; 32]],
        "single-sample replacement must exclude playback feedback"
    );
}

unsafe fn one_shot_rearm_and_invalidation(module: &Module) {
    let mut plugin = module.create();
    configure(&plugin);
    plugin.flush(&[("Record Once", "On")]);
    plugin.activate(48000.0, 64);
    let tr = transport(true, 120.0, 0, false);
    let a = [0.25; 8];
    let b = [-0.5; 8];
    let c = [0.75; 8];
    let z = [0.0; 8];
    plugin.process([&a, &z, &z, &z], 8, &[Event::note(0, 48, 1.0)], &tr);
    plugin.reset();
    plugin.process([&b, &z, &z, &z], 8, &[Event::note(0, 48, 1.0)], &tr);
    plugin.reset();
    let protected = plugin.process([&z; 4], 8, &[Event::note(0, 50, 1.0)], &tr);
    assert_eq!(
        protected[0],
        a.map(quantize),
        "reset must not clear Record Once latch"
    );

    plugin.flush(&[("Re-arm", "On")]);
    plugin.process([&z[..1]; 4], 1, &[], &tr);
    plugin.flush(&[("Re-arm", "Off")]);
    plugin.process([&c, &z, &z, &z], 8, &[Event::note(0, 48, 1.0)], &tr);
    // Re-arm while the take is active must not interrupt or unlock it.
    plugin.flush(&[("Re-arm", "On")]);
    plugin.process([&b, &z, &z, &z], 8, &[Event::note(0, 48, 1.0)], &tr);
    plugin.reset();
    let silence = [0.0; 16];
    let replaced = plugin.process([&silence; 4], 16, &[Event::note(0, 50, 1.0)], &tr);
    assert_eq!(
        replaced[0],
        [c, b]
            .concat()
            .into_iter()
            .map(quantize)
            .collect::<Vec<_>>()
    );

    // A same-length replacement invalidates a voice latched to the old revision.
    plugin.flush(&[("Record Once", "Off"), ("Re-arm", "Off")]);
    plugin.reset();
    plugin.process([&z[..1]; 4], 1, &[Event::note(0, 50, 1.0)], &tr);
    let replacement = [b, b].concat();
    let invalidated = plugin.process(
        [&replacement, &silence, &silence, &silence],
        16,
        &[Event::note(0, 48, 1.0)],
        &tr,
    );
    assert_eq!(invalidated, [vec![0.0; 16], vec![0.0; 16]]);
}

unsafe fn retrigger_tempo_transport_and_reset(module: &Module) {
    let mut plugin = module.create();
    configure(&plugin);
    plugin.activate(48000.0, 16384);
    let playing = transport(true, 120.0, 0, false);
    plugin.process(
        [&[0.25], &[0.0], &[0.0], &[0.0]],
        1,
        &[Event::note(0, 48, 1.0)],
        &playing,
    );
    plugin.reset();
    plugin.flush(&[("Retrigger", "2"), ("Retrig Time", "127")]);
    let output = plugin.process(
        [&vec![0.0; 12001]; 4],
        12001,
        &[Event::note(0, 50, 1.0)],
        &playing,
    );
    for (i, &sample) in output[0].iter().enumerate() {
        assert_eq!(
            sample,
            if matches!(i, 0 | 6000 | 12000) {
                0.25
            } else {
                0.0
            },
            "120 BPM frame {i}"
        );
    }
    plugin.reset();
    let silent = plugin.process(
        [&vec![0.0; 6001]; 4],
        6001,
        &[],
        &transport(true, 240.0, 0, false),
    );
    assert!(silent[0].iter().all(|&x| x == 0.0));
    let at_240 = plugin.process(
        [&vec![0.0; 6001]; 4],
        6001,
        &[Event::note(0, 50, 1.0)],
        &transport(true, 240.0, 0, false),
    );
    assert_eq!(at_240[0][0], 0.25);
    assert_eq!(at_240[0][3000], 0.25);
    assert_eq!(at_240[0][6000], 0.25);
    let stopped = plugin.process([&[0.0; 8]; 4], 8, &[], &transport(false, 240.0, 0, false));
    assert_eq!(stopped, [vec![0.0; 8], vec![0.0; 8]]);
}

unsafe fn state_and_instance_isolation(module: &Module) {
    let mut owner = module.create();
    configure(&owner);
    owner.flush(&[
        ("Capture Note", "44"),
        ("Play Note", "45"),
        ("Record Once", "On"),
    ]);
    owner.activate(48000.0, 16);
    let tr = transport(true, 120.0, 0, false);
    let source = [0.375; 8];
    let zero = [0.0; 8];
    owner.process(
        [&source, &zero, &zero, &zero],
        8,
        &[Event::note(0, 44, 1.0)],
        &tr,
    );
    owner.reset();
    let state = owner.save();

    let mut isolated = module.create();
    isolated.load_state(&state);
    assert_eq!(isolated.value("Capture Note").round(), 44.0);
    assert_eq!(isolated.value("Play Note").round(), 45.0);
    assert_eq!(isolated.value("Record Once"), 1.0);
    isolated.activate(48000.0, 16);
    let empty = isolated.process([&zero; 4], 8, &[Event::note(0, 45, 1.0)], &tr);
    assert_eq!(
        empty,
        [vec![0.0; 8], vec![0.0; 8]],
        "RAM must not be serialized or shared"
    );
    // Record Once is a saved control, but its spent latch is runtime-only: this
    // new instance must be able to capture immediately after loading.
    isolated.process(
        [&source, &zero, &zero, &zero],
        8,
        &[Event::note(0, 44, 1.0)],
        &tr,
    );
    isolated.reset();
    let recaptured = isolated.process([&zero; 4], 8, &[Event::note(0, 45, 1.0)], &tr);
    assert_eq!(
        recaptured[0],
        source.map(quantize),
        "restored instance must be able to recapture"
    );

    let owner_take = owner.process([&zero; 4], 8, &[Event::note(0, 45, 1.0)], &tr);
    assert_eq!(
        owner_take[0],
        source.map(quantize),
        "other instance retains only its own RAM"
    );
}

unsafe fn restored_and_reset_commands_need_new_edges(module: &Module) {
    let owner = module.create();
    configure(&owner);
    owner.flush(&[
        ("Record Once", "On"),
        ("Capture", "On"),
        ("Playback", "On"),
        ("Re-arm", "On"),
    ]);
    let state = owner.save();
    let mut restored = module.create();
    restored.load_state(&state);
    restored.activate(48000.0, 32);
    let tr = transport(true, 120.0, 0, false);
    let a = [0.25; 8];
    let b = [-0.5; 8];
    let z = [0.0; 16];
    restored.process([&a, &z[..8], &z[..8], &z[..8]], 8, &[], &tr);
    restored.flush(&[("Capture", "Off"), ("Playback", "Off"), ("Re-arm", "Off")]);
    restored.reset();
    let empty = restored.process([&z; 4], 16, &[Event::note(0, 50, 1.0)], &tr);
    assert_eq!(
        empty,
        [vec![0.0; 16], vec![0.0; 16]],
        "restoring high commands must not capture audio"
    );

    restored.process(
        [&a, &z[..8], &z[..8], &z[..8]],
        8,
        &[Event::note(0, 48, 1.0)],
        &tr,
    );
    // A rising Re-arm during capture is ignored. Holding it across reset must
    // not turn that ignored edge into permission to overwrite the protected take.
    restored.flush(&[("Capture", "On"), ("Playback", "On"), ("Re-arm", "On")]);
    restored.process([&a, &z[..8], &z[..8], &z[..8]], 8, &[], &tr);
    restored.reset();
    let stopped = restored.process(
        [&b, &z[..8], &z[..8], &z[..8]],
        8,
        &[Event::note(0, 48, 1.0)],
        &tr,
    );
    assert_eq!(
        stopped,
        [vec![0.0; 8], vec![0.0; 8]],
        "held Playback must not restart after reset"
    );
    restored.flush(&[("Capture", "Off"), ("Playback", "Off"), ("Re-arm", "Off")]);
    restored.reset();
    let protected = restored.process([&z; 4], 16, &[Event::note(0, 50, 1.0)], &tr);
    assert_eq!(
        protected[0],
        vec![quantize(0.25); 16],
        "held Re-arm must not release Record Once on reset"
    );
}

fn library(name: &str) -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(libloading::library_filename(name))
}

fn main() {
    // SAFETY: load only the sibling build artifact, keep it alive until all
    // fixture plugins drop, and drive every CLAP callback synchronously.
    unsafe {
        let started = Instant::now();
        let module = Module::load(&library("ultrawave_plugin"));
        contract_and_sample_only_output(&module);
        master_mode_automation_during_capture(&module);
        for (rate, block) in [(44100.0, 65), (48000.0, 257), (96000.0, 1025)] {
            master_resamples_playback(&module, rate, block);
            capture_playback_at_rate(&module, rate, block);
        }
        reverse_pitch_notes_and_triggers(&module);
        capture_control_is_a_rising_edge(&module);
        record_priority_and_no_feedback(&module);
        one_shot_rearm_and_invalidation(&module);
        retrigger_tempo_transport_and_reset(&module);
        state_and_instance_isolation(&module);
        restored_and_reset_commands_need_new_edges(&module);
        assert!(started.elapsed() < Duration::from_secs(20));
        println!(
            "CLAP audio harness ok: master self-resampling + sample-only; one sample; 27 params; rates=44.1/48/96k; routing, offsets, automation, replacement, retrigger, Record Once, volatile RAM ({:.2?})",
            started.elapsed()
        );
    }
}
