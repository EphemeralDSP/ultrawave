//! Small, deliberately project-specific CLAP host for exported-plugin regressions.
#![allow(unsafe_code)]

use clap_sys::{
    audio_buffer::clap_audio_buffer,
    entry::clap_plugin_entry,
    events::*,
    ext::{
        audio_ports::{clap_audio_port_info, clap_plugin_audio_ports, CLAP_EXT_AUDIO_PORTS},
        params::{clap_param_info, clap_plugin_params, CLAP_EXT_PARAMS, CLAP_PARAM_IS_AUTOMATABLE},
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
        unsafe { (*self.entry).deinit.unwrap()() }
    }
}

struct Plugin {
    raw: *const clap_plugin,
    _host: Box<clap_host>,
    active: bool,
}
impl Plugin {
    unsafe fn ext<T>(&self, id: &CStr) -> &T {
        let extension = (*self.raw).get_extension.unwrap()(self.raw, id.as_ptr()) as *const T;
        assert!(!extension.is_null(), "required extension {id:?}");
        &*extension
    }
    unsafe fn params(&self) -> &clap_plugin_params {
        self.ext(CLAP_EXT_PARAMS)
    }
    unsafe fn param(&self, name: &str) -> clap_param_info {
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
        let mut value = 0.0;
        assert!(self.params().get_value.unwrap()(
            self.raw,
            self.param(name).id,
            &mut value,
        ));
        value
    }
    unsafe fn flush(&self, values: &[(&str, &str)]) {
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
        assert!(!self.active);
        assert!((*self.raw).activate.unwrap()(self.raw, rate, 1, max));
        assert!((*self.raw).start_processing.unwrap()(self.raw));
        self.active = true;
    }
    unsafe fn stop(&mut self) {
        if self.active {
            (*self.raw).stop_processing.unwrap()(self.raw);
            (*self.raw).deactivate.unwrap()(self.raw);
            self.active = false;
        }
    }
    unsafe fn reset(&self) {
        (*self.raw).reset.unwrap()(self.raw)
    }
    unsafe fn ports(&self, input: bool) -> Vec<clap_audio_port_info> {
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
        assert!(input.iter().all(|x| x.len() == frames || x.is_empty()));
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
    fn note(time: u32) -> Self {
        Self::Note(clap_event_note {
            header: header::<clap_event_note>(time, CLAP_EVENT_NOTE_ON),
            note_id: -1,
            port_index: 0,
            channel: 0,
            key: 60,
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
    (*((*l).ctx as *const EventContext)).len as u32
}
unsafe extern "C" fn event_get(l: *const clap_input_events, i: u32) -> *const clap_event_header {
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

unsafe fn configure_record(p: &Plugin, bank: &str, monitor: &str) {
    p.flush(&[
        ("Bank", bank),
        ("Main Level", "127"),
        ("Main Balance", "L64"),
        ("Sidechain Level", "127"),
        ("Sidechain Balance", "R64"),
        ("Dry Monitor", monitor),
        ("Capture Monitor", "-inf"),
        ("Rec Length", "1/8"),
        ("Rec Rate", "HI"),
        ("Level", "0 dB"),
    ]);
}
unsafe fn configure_play(p: &Plugin, bank: &str) {
    p.flush(&[
        ("Bank", bank),
        ("Start", "0"),
        ("End", "127"),
        ("Pitch", "+0 st"),
        ("Hold", "OFF"),
        ("Decay", "OFF"),
        ("Retrigger", "OFF"),
        ("Retrig Time", "OFF"),
        ("Level", "0 dB"),
    ]);
}
fn approx(actual: f32, expected: f32, msg: &str) {
    assert!(
        (actual - expected).abs() < 0.0006,
        "{msg}: got {actual}, expected {expected}"
    )
}

unsafe fn run(record: &Module, play: &Module, rate: f64, frames: usize, bank: &str) {
    let mut r = record.create();
    let mut p = play.create();
    assert_eq!(
        r.ports(true)
            .iter()
            .map(|x| x.channel_count)
            .collect::<Vec<_>>(),
        [2, 2]
    );
    assert_eq!(r.ports(false)[0].channel_count, 2);
    assert!(p.ports(true).is_empty());
    assert_eq!(p.ports(false)[0].channel_count, 2);
    assert_eq!(r.param("Bank").flags & CLAP_PARAM_IS_AUTOMATABLE, 0);
    configure_record(&r, bank, "127");
    configure_play(&p, bank);
    r.activate(rate, frames as u32);
    p.activate(rate, frames as u32);
    let main_l: Vec<_> = (0..frames)
        .map(|i| ((i % 17) as f32 - 8.0) / 64.0)
        .collect();
    let main_r = vec![0.3125; frames];
    let side_l = vec![-0.4375; frames];
    let side_r: Vec<_> = (0..frames)
        .map(|i| ((i % 13) as f32 - 6.0) / 32.0)
        .collect();
    let tr = transport(true, 120.0, 0, false);
    let capture_at = frames / 3;
    let events = [Event::note(capture_at as u32)];
    let monitored = r.process([&main_l, &main_r, &side_l, &side_r], frames, &events, &tr);
    // Finish the take so reverse/state assertions see a fixed region.
    r.reset();
    // Asymmetric hard-pans prove lane exclusion and output duplication.
    for i in [0, capture_at, frames - 1] {
        let dry = main_l[i] + side_r[i];
        approx(monitored[0][i], dry, "dry route L");
        approx(monitored[1][i], dry, "dry route R");
    }
    let played = p.process([&[], &[], &[], &[]], frames, &[Event::note(5)], &tr);
    for (i, sample) in played[0][..5].iter().enumerate() {
        assert_eq!(*sample, 0.0, "MIDI offset {i}");
    }
    // Capture begins on the exact event frame; 12-bit conversion is the independent oracle.
    for (i, sample) in played[0].iter().enumerate().skip(5) {
        let source = capture_at + i - 5;
        if source >= frames {
            break;
        }
        let expected = ((main_l[source] + side_r[source]) * 2048.0)
            .round()
            .clamp(-2048.0, 2047.0)
            / 2048.0;
        assert_eq!(*sample, expected, "captured code-grid frame {i}");
        assert_eq!(played[1][i], expected, "stereo play frame {i}");
    }
    p.flush(&[("Start", "127"), ("End", "0")]);
    let reverse_state = p.save();
    p.stop();
    p.flush(&[("Bank", "OFF"), ("Start", "0"), ("End", "127")]);
    p.load_state(&reverse_state);
    assert_eq!(p.value("Bank"), p.text_value("Bank", bank));
    p.activate(rate, frames as u32);
    let reverse = p.process([&[]; 4], frames, &[Event::note(0)], &tr);
    let length = frames - capture_at;
    for i in 0..frames {
        let expected = if i < length {
            main_l[frames - 1 - i] + side_r[frames - 1 - i]
        } else {
            0.0
        };
        assert_eq!(reverse[0][i], expected, "restored reverse frame {i}");
        assert_eq!(reverse[1][i], expected);
    }
    // Dry-monitor automation changes output on its exact event frame.
    let cue = r.param("Dry Monitor");
    let zero = r.text_value("Dry Monitor", "-inf");
    let automated = [Event::param(frames as u32 / 2, cue, zero)];
    let out = r.process(
        [&main_l, &main_r, &side_l, &side_r],
        frames,
        &automated,
        &tr,
    );
    approx(
        out[0][frames / 2 - 1],
        main_l[frames / 2 - 1] + side_r[frames / 2 - 1],
        "automation pre-frame",
    );
    assert_eq!(out[0][frames / 2], 0.0, "automation event offset");
    let state = r.save();
    r.stop();
    drop(r);
    drop(p);
    let mut restored = record.create();
    restored.load_state(&state);
    assert_eq!(restored.value("Bank"), restored.text_value("Bank", bank));
    assert_eq!(restored.value("Dry Monitor"), 0.0);
    restored.activate(rate, frames as u32);
    drop(restored);
}

// Nonzero, unequal values on all four lanes catch swapped buses, duplicated
// channels, or accidentally including a channel whose gain should be zero.
unsafe fn routing(record: &Module, play: &Module) {
    for (mbal, ibal, main, side, expected) in [
        ("L64", "L64", "127", "0", 0.125),
        ("R64", "L64", "127", "0", -0.25),
        ("L64", "L64", "0", "127", 0.375),
        ("L64", "R64", "0", "127", -0.5),
        ("L-R", "L-R", "127", "127", -0.125),
    ] {
        let mut r = record.create();
        let mut p = play.create();
        configure_record(&r, "8", "-inf");
        configure_play(&p, "8");
        r.flush(&[
            ("Main Level", main),
            ("Sidechain Level", side),
            ("Main Balance", mbal),
            ("Sidechain Balance", ibal),
        ]);
        r.activate(48000.0, 17);
        p.activate(48000.0, 17);
        let source = [[0.125; 17], [-0.25; 17], [0.375; 17], [-0.5; 17]];
        let tr = transport(true, 120.0, 0, false);
        let silent = r.process(
            source.each_ref().map(|v| v.as_slice()),
            17,
            &[Event::note(0)],
            &tr,
        );
        assert_eq!(
            silent,
            [vec![0.0; 17], vec![0.0; 17]],
            "monitor defaults must be silent"
        );
        let take = p.process([&[]; 4], 17, &[Event::note(0)], &tr);
        assert_eq!(
            take,
            [vec![expected; 17], vec![expected; 17]],
            "routing {mbal}/{ibal}/{main}/{side}"
        );
        r.flush(&[("Capture Monitor", "127")]);
        let wet = r.process(source.each_ref().map(|v| v.as_slice()), 17, &[], &tr);
        assert_eq!(
            wet, take,
            "capture monitor must use the captured mono signal"
        );
        // Another live bank must not hear bank 8 (not just OFF or a closed bank).
        let mut other = play.create();
        configure_play(&other, "7");
        other.activate(48000.0, 17);
        assert_eq!(
            other.process([&[]; 4], 17, &[Event::note(0)], &tr),
            [vec![0.0; 17], vec![0.0; 17]]
        );
    }
}

unsafe fn retriggers(record: &Module, play: &Module, rate: f64, block: usize) {
    let mut r = record.create();
    let mut p = play.create();
    configure_record(&r, "6", "-inf");
    configure_play(&p, "6");
    r.activate(rate, block as u32);
    p.activate(rate, block as u32);
    let tr = transport(true, 120.0, 0, false);
    // A one-frame capture makes every hit and every silent gap observable.
    r.process([&[0.25], &[0.0], &[0.0], &[0.0]], 1, &[Event::note(0)], &tr);
    r.reset();
    for (count, time, bpm, interval) in [
        ("2", "127", 120.0, rate / 8.0),
        ("2", "127", 60.0, rate / 4.0),
        ("INF", "127", 120.0, rate / 8.0),
        ("2", "OFF", 120.0, rate / 8.0),
    ] {
        p.reset();
        p.flush(&[("Retrigger", count), ("Retrig Time", time)]);
        let hits = if time == "OFF" {
            1
        } else if count == "INF" {
            140
        } else {
            3
        };
        let end = 5 + ((hits + 1) as f64 * interval).ceil() as usize;
        let mut next_hit = 0;
        for start in (0..end).step_by(block) {
            let len = block.min(end - start);
            let notes = if start <= 5 && 5 < start + len {
                vec![Event::note((5 - start) as u32)]
            } else {
                vec![]
            };
            let output = p.process([&[]; 4], len, &notes, &transport(true, bpm, 0, false));
            for (offset, (left, right)) in output[0].iter().zip(&output[1]).enumerate() {
                let frame = start + offset;
                let hit_frame = 5 + (next_hit as f64 * interval).round() as usize;
                let hit =
                    frame == hit_frame && (count == "INF" && time != "OFF" || next_hit < hits);
                let expected = if hit {
                    next_hit += 1;
                    0.25
                } else {
                    0.0
                };
                assert_eq!(
                    *left, expected,
                    "{rate}/{block} {count}/{time} bpm{bpm} frame{frame}"
                );
                assert_eq!(*right, expected);
            }
        }
    }
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
    unsafe {
        let started = Instant::now();
        let record = Module::load(&library("ultrawave_record"));
        let play = Module::load(&library("ultrawave_play"));
        routing(&record, &play);
        for (i, (rate, block)) in [(44100.0, 65), (48000.0, 257), (96000.0, 1025)]
            .into_iter()
            .enumerate()
        {
            run(&record, &play, rate, block, &format!("{}", i + 1));
            retriggers(&record, &play, rate, block);
        }
        // The final lease destroys volatile RAM: reopening bank 1 is silent.
        let mut cleared = play.create();
        configure_play(&cleared, "1");
        cleared.activate(44100.0, 65);
        let silent = cleared.process(
            [&[], &[], &[], &[]],
            65,
            &[Event::note(0)],
            &transport(true, 120.0, 0, false),
        );
        assert!(
            silent[0].iter().all(|x| *x == 0.0),
            "bank RAM survived its last lease"
        );
        drop(cleared);
        // OFF instances have private RAM even while alive.
        let mut r = record.create();
        let mut p = play.create();
        configure_record(&r, "OFF", "-inf");
        configure_play(&p, "OFF");
        r.activate(44100.0, 33);
        p.activate(44100.0, 33);
        let one = vec![0.25; 33];
        let z = vec![0.0; 33];
        r.process(
            [&one, &z, &z, &z],
            33,
            &[Event::note(0)],
            &transport(true, 120.0, 0, false),
        );
        let o = p.process(
            [&[], &[], &[], &[]],
            33,
            &[Event::note(0)],
            &transport(true, 120.0, 0, false),
        );
        assert!(
            o[0].iter().all(|x| *x == 0.0),
            "OFF RAM leaked between instances"
        );
        // Reset and a playing->stopped edge cancel playback. Seek and loop metadata alone do not.
        drop(r);
        drop(p);
        let mut r = record.create();
        let mut p = play.create();
        configure_record(&r, "9", "-inf");
        configure_play(&p, "9");
        r.activate(44100.0, 129);
        p.activate(44100.0, 129);
        let long_one: Vec<_> = (0..129).map(|i| (i as f32 - 64.0) / 2048.0).collect();
        let long_zero = vec![0.0; 129];
        r.process(
            [&long_one, &long_zero, &long_zero, &long_zero],
            129,
            &[Event::note(0)],
            &transport(true, 120.0, 0, false),
        );
        let a = p.process(
            [&[], &[], &[], &[]],
            33,
            &[Event::note(0)],
            &transport(true, 120.0, 0, false),
        );
        assert_eq!(a[0], long_one[..33]);
        let seek = p.process(
            [&[], &[], &[], &[]],
            33,
            &[],
            &transport(true, 120.0, 99 << 31, true),
        );
        assert_eq!(
            seek[0],
            long_one[33..66],
            "seek must neither stop nor restart the voice"
        );
        let looped = p.process([&[]; 4], 33, &[], &transport(true, 120.0, 0, true));
        assert_eq!(
            looped[0],
            long_one[66..99],
            "loop wrap must not restart the voice"
        );
        let stopped = p.process(
            [&[], &[], &[], &[]],
            33,
            &[],
            &transport(false, 120.0, 0, false),
        );
        assert!(stopped[0].iter().all(|x| *x == 0.0));
        p.process(
            [&[]; 4],
            1,
            &[Event::note(0)],
            &transport(true, 120.0, 0, false),
        );
        p.reset();
        assert_eq!(
            p.process([&[]; 4], 33, &[], &transport(true, 120.0, 0, false)),
            [vec![0.0; 33], vec![0.0; 33]]
        );
        assert!(started.elapsed() < Duration::from_secs(20));
        println!("CLAP audio harness ok: rates=44.1/48/96k blocks=65/257/1025; seek/loop metadata preserves voices; stop/reset cancels ({:.2?})",started.elapsed());
    }
}
