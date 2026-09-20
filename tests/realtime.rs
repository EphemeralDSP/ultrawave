use std::sync::Arc;

#[path = "support/allocations.rs"]
mod allocations;

use ultrawave::machines::memory::{SampleMemory, MAX_BUFFER_SAMPLES};
use ultrawave::machines::ram_play::{RamPlay, RamPlayParams};
use ultrawave::machines::ram_record::{RamRecord, RamRecordParams};

fn neutral() -> RamPlayParams {
    RamPlayParams {
        vol: 127,
        ..Default::default()
    }
}

#[test]
fn trigger_and_process_do_not_allocate_or_free() {
    let registry = Arc::new(SampleMemory::new());
    registry.replace_buffer(0, vec![512; 1024]);
    let mut player = RamPlay::new(44_100.0, registry.clone());
    player.load_buffer(vec![512; 1024], 0);
    let mut recorder = RamRecord::new(44_100.0, registry);
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
fn replacement_invalidates_an_existing_voice_without_mixing_versions() {
    let registry = Arc::new(SampleMemory::new());
    assert!(registry.replace_buffer(0, vec![1024; 16]));
    let mut player = RamPlay::new(44_100.0, registry.clone());
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
    let registry = Arc::new(SampleMemory::new());
    assert!(registry.replace_buffer(0, vec![1024; 16]));
    let mut player = RamPlay::new(44_100.0, registry.clone());
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
        let registry = Arc::new(SampleMemory::new());
        assert!(registry.replace_buffer_at_rate(0, vec![1024; 441], 44_100.0));
        let mut player = RamPlay::new(output_rate, registry);
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
    let registry = Arc::new(SampleMemory::new());
    assert!(registry.replace_buffer(0, vec![0, 0, 2047]));
    let mut player = RamPlay::new(44_100.0, registry.clone());
    player.trigger(&neutral(), 0);
    assert_eq!(player.process(0), 0.0);
    assert_eq!(player.process(0), 0.0);
    assert_eq!(player.process(0), 2047.0 / 2048.0);

    assert!(registry.replace_buffer(1, vec![1; MAX_BUFFER_SAMPLES + 10]));
    assert_eq!(registry.buffer_len(1), MAX_BUFFER_SAMPLES);
}
