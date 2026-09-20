use std::sync::Arc;

#[path = "support/allocations.rs"]
mod allocations;

use ultrawave::machines::ram_play::{RamPlay, RamPlayParams};
use ultrawave::machines::ram_record::{RamRecord, RamRecordParams};
use ultrawave::machines::shared::{BufferRegistry, MAX_BUFFER_SAMPLES};

fn neutral() -> RamPlayParams {
    RamPlayParams {
        vol: 127,
        ..Default::default()
    }
}

#[test]
fn active_capture_excludes_all_other_writes() {
    let registry = Arc::new(BufferRegistry::new());
    let params = RamRecordParams {
        mlev: 127,
        ilev: 0,
        ..Default::default()
    };
    let mut recorder = RamRecord::with_registry(44_100.0, registry.clone());
    let mut contender = RamRecord::with_registry(44_100.0, registry.clone());
    recorder.start_recording(&params, true, 0);
    recorder.record_sample(0.25, 0.25, 0.0, 0.0, &params, 0);
    contender.start_recording(&params, true, 0);
    assert!(!contender.is_recording(0));
    assert!(!registry.replace_buffer(0, vec![-1024]));
    assert!(!registry.clear_buffer(0));
    assert!(!registry.reset_write_position(0));
    assert!(!registry.write_sample(0, -1024));
    registry.clear_all();
    recorder.record_sample(0.5, 0.5, 0.0, 0.0, &params, 0);
    assert_eq!(registry.read_buffer(0).unwrap(), [512, 1024]);
    drop(recorder);
    contender.start_recording(&params, true, 0);
    assert!(contender.is_recording(0));
}

#[test]
fn trigger_and_process_do_not_allocate_or_free() {
    let registry = Arc::new(BufferRegistry::new());
    registry.replace_buffer(0, vec![512; 1024]);
    let mut player = RamPlay::with_registry(44_100.0, registry.clone());
    player.load_buffer(vec![512; 1024], 0);
    let mut recorder = RamRecord::with_registry(44_100.0, registry);
    let rec_params = RamRecordParams::default();
    let params = neutral();
    allocations::assert_no_alloc(|| {
        let mut mixed = [0.0; 67];
        ultrawave::dsp::mix::mix_inputs([&[0.25; 67]; 4], [0.25; 4], &mut mixed);
        let mut gains = ultrawave::dsp::mix::SmoothedGains::<4>::default();
        gains.set([0.25; 4], 44100.0);
        gains.set([0.1, 0.2, 0.3, 0.4], 44100.0);
        for _ in 0..5 {
            gains.mix([&[0.25; 67]; 4], &mut mixed);
        }
        std::hint::black_box(mixed);
        player.trigger(&params, 0);
        recorder.start_recording(&rec_params, true, 1);
        for _ in 0..100 {
            std::hint::black_box(player.process(0));
            recorder.record_sample(0.25, 0.25, 0.0, 0.0, &rec_params, 1);
        }
        player.trigger(
            &RamPlayParams {
                strt: 127,
                end: 0,
                rtrg: 127,
                rtim: 32,
                ..params
            },
            0,
        );
        player.set_tempo(137.0);
        for _ in 0..10_000 {
            std::hint::black_box(player.process(0));
        }
        recorder.stop_all_recording();
        player.stop_all();
    });
}

#[test]
fn rebind_active_machines_defers_old_registry_destruction() {
    let registry = Arc::new(BufferRegistry::new());
    registry.replace_buffer(0, vec![1024; 32]);
    let weak = Arc::downgrade(&registry);
    let mut player = RamPlay::with_registry(44100.0, registry.clone());
    let mut recorder = RamRecord::with_registry(44100.0, registry);
    player.trigger(&neutral(), 0);
    recorder.start_recording(&RamRecordParams::default(), true, 1);
    let next = Arc::new(BufferRegistry::new());
    let mut retired = None;
    allocations::assert_no_alloc(|| {
        retired = Some((
            player.rebind_registry(next.clone()),
            recorder.rebind_registry(next),
        ));
    });
    assert!(!player.is_any_playing());
    assert!(!recorder.is_any_recording());
    assert!(weak.upgrade().is_some());
    drop(retired);
    assert!(
        weak.upgrade().is_none(),
        "no engine retains the retired bank"
    );
}

#[test]
fn replacement_invalidates_an_existing_voice_without_mixing_versions() {
    let registry = Arc::new(BufferRegistry::new());
    assert!(registry.replace_buffer(0, vec![1024; 16]));
    let mut player = RamPlay::with_registry(44_100.0, registry.clone());
    player.trigger(&neutral(), 0);
    assert_eq!(player.process(0), 0.5);

    assert!(registry.replace_buffer(0, vec![-1024; 16]));
    assert_eq!(player.process(0), 0.0);
    assert!(!player.is_playing(0));

    player.trigger(&neutral(), 0);
    assert_eq!(player.process(0), -0.5);
}

#[test]
fn invalidation_clears_srr_held_sample_and_retrigger_is_safe() {
    let registry = Arc::new(BufferRegistry::new());
    assert!(registry.replace_buffer(0, vec![1024; 16]));
    let mut player = RamPlay::with_registry(44_100.0, registry.clone());
    let params = RamPlayParams {
        srr: 0.5,
        vol: 127,
        ..neutral()
    };
    player.trigger(&params, 0);
    let mut held_nonzero = false;
    for _ in 0..8 {
        if player.process(0) == 0.5 {
            held_nonzero = true;
            break;
        }
    }
    assert!(
        held_nonzero,
        "SRR should eventually latch the nonzero source"
    );

    assert!(registry.replace_buffer(0, vec![-1024; 16]));
    assert_eq!(player.process(0), 0.0);
    player.trigger(&params, 0);
    for _ in 0..8 {
        let sample = player.process(0);
        assert!(sample <= 0.0, "retrigger leaked the old positive hold");
        if sample == -0.5 {
            return;
        }
    }
    panic!("SRR should eventually latch the replacement source");
}

#[test]
fn source_rate_preserves_duration_at_48_and_96_khz_outputs() {
    for output_rate in [48_000.0, 96_000.0] {
        let registry = Arc::new(BufferRegistry::new());
        assert!(registry.replace_buffer_at_rate(0, vec![1024; 441], 44_100.0));
        let mut player = RamPlay::with_registry(output_rate, registry);
        player.trigger(&neutral(), 0);
        let mut frames = 0;
        while player.is_playing(0) {
            player.process(0);
            frames += 1;
            assert!(frames < 2_000);
        }
        let expected = output_rate as usize / 100;
        assert!(
            (frames as isize - expected as isize).abs() <= 1,
            "{output_rate}: {frames}"
        );
    }
}

#[test]
fn final_sample_and_capacity_bound_are_preserved() {
    let registry = Arc::new(BufferRegistry::new());
    assert!(registry.replace_buffer(0, vec![0, 0, 2047]));
    let mut player = RamPlay::with_registry(44_100.0, registry.clone());
    player.trigger(&neutral(), 0);
    assert_eq!(player.process(0), 0.0);
    assert_eq!(player.process(0), 0.0);
    assert_eq!(player.process(0), 2047.0 / 2048.0);

    assert!(registry.replace_buffer(1, vec![1; MAX_BUFFER_SAMPLES + 10]));
    assert_eq!(registry.buffer_len(1), MAX_BUFFER_SAMPLES);
}
