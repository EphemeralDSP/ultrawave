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
    registry.replace_buffer(vec![512; 1024]);
    let mut player = RamPlay::new(44_100.0, registry.clone());
    player.load_buffer(vec![512; 1024]);
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
        player.trigger(&params);
        recorder.start_recording(&rec_params);
        for _ in 0..100 {
            std::hint::black_box(player.process());
            recorder.record_sample(0.25, 0.25, 0.0, 0.0, &rec_params);
        }
        player.trigger(&RamPlayParams {
            strt: 127,
            end: 0,
            rtrg: 127,
            rtim: 32,
            ..params
        });
        player.set_tempo(137.0);
        for _ in 0..10_000 {
            std::hint::black_box(player.process());
        }
        recorder.stop_recording();
        player.stop();
    });
}

#[test]
fn replacement_invalidates_an_existing_voice_without_mixing_versions() {
    let registry = Arc::new(SampleMemory::new());
    assert!(registry.replace_buffer(vec![1024; 16]));
    let mut player = RamPlay::new(44_100.0, registry.clone());
    player.trigger(&neutral());
    assert_eq!(player.process(), 0.5);

    assert!(registry.replace_buffer(vec![-1024; 16]));
    assert_eq!(player.process(), 0.0);
    assert!(!player.is_playing());

    player.trigger(&neutral());
    assert_eq!(player.process(), -0.5);
}

#[test]
fn invalidation_clears_srr_held_sample_and_retrigger_is_safe() {
    let registry = Arc::new(SampleMemory::new());
    assert!(registry.replace_buffer(vec![1024; 16]));
    let mut player = RamPlay::new(44_100.0, registry.clone());
    let params = RamPlayParams {
        srr: 0.5,
        vol: 127,
        ..neutral()
    };
    player.trigger(&params);
    let mut held_nonzero = false;
    for _ in 0..8 {
        if player.process() == 0.5 {
            held_nonzero = true;
            break;
        }
    }
    assert!(
        held_nonzero,
        "SRR should eventually latch the nonzero source"
    );

    assert!(registry.replace_buffer(vec![-1024; 16]));
    assert_eq!(player.process(), 0.0);
    player.trigger(&params);
    for _ in 0..8 {
        let sample = player.process();
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
        assert!(registry.replace_buffer_at_rate(vec![1024; 441], 44_100.0));
        let mut player = RamPlay::new(output_rate, registry);
        player.trigger(&neutral());
        let mut frames = 0;
        while player.is_playing() {
            player.process();
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
    assert!(registry.replace_buffer(vec![0, 0, 2047]));
    let mut player = RamPlay::new(44_100.0, registry.clone());
    player.trigger(&neutral());
    assert_eq!(player.process(), 0.0);
    assert_eq!(player.process(), 0.0);
    assert_eq!(player.process(), 2047.0 / 2048.0);

    assert!(registry.replace_buffer(vec![1; MAX_BUFFER_SAMPLES + 10]));
    assert_eq!(registry.buffer_len(), MAX_BUFFER_SAMPLES);
}

#[test]
fn resampling_preserves_the_old_take_and_rate_until_publication_without_allocating() {
    let memory = Arc::new(SampleMemory::new());
    memory.replace_buffer_at_rate((0..64).map(|i| i * 16 - 512).collect(), 200.0);
    let old = memory.snapshot().unwrap();
    let mut player = RamPlay::new(100.0, memory.clone());
    let mut recorder = RamRecord::new(100.0, memory.clone());
    let params = RamRecordParams {
        rec_len: 0, // 25 host frames, one stored frame at 1/16 rate.
        rec_rate: 0,
        ..Default::default()
    };
    allocations::assert_no_alloc(|| {
        player.trigger(&neutral());
        recorder.start_resampling(&params);
        for frame in 0..25 {
            assert_eq!(memory.snapshot().unwrap(), old);
            player.update_params(&neutral());
            assert_eq!(player.process(), (frame * 32 - 512) as f32 / 2048.0);
            recorder.record_mono(-0.25);
        }
        assert!(!recorder.is_recording());
        let (len, rate, revision) = memory.snapshot().unwrap();
        assert_eq!((len, rate), (1, 6.25));
        assert_ne!(revision, old.2);
        assert_eq!(player.process(), 0.0);
        player.trigger(&neutral());
        assert_eq!(player.process(), -0.25);
        player.stop();
    });
    assert_eq!(memory.read_buffer().unwrap(), [-512]);
}

#[test]
fn interrupted_resampling_publishes_only_written_samples_and_empty_attempts_keep_the_take() {
    let memory = Arc::new(SampleMemory::new());
    memory.replace_buffer(vec![256, -512, 128]);
    let mut recorder = RamRecord::new(44100.0, memory.clone());
    let params = RamRecordParams::default();
    let old = memory.snapshot();
    recorder.start_resampling(&params);
    recorder.stop_recording();
    assert_eq!(memory.snapshot(), old);

    recorder.start_resampling(&params);
    recorder.record_mono(-0.125);
    recorder.record_mono(0.25);
    assert_eq!(memory.read_buffer().unwrap(), [256, -512, 128]);
    recorder.stop_recording();
    assert_eq!(memory.read_buffer().unwrap(), [-256, 512]);
    recorder.start_resampling(&params);
    recorder.record_mono(0.75);
    recorder.clear();
    recorder.stop_recording();
    assert_eq!(memory.buffer_len(), 0);
    recorder.start_recording(&params);
    recorder.record_mono(0.125);
    assert_eq!(memory.read_buffer().unwrap(), [256]);
}

#[test]
fn staged_capacity_publishes_on_the_last_available_sample() {
    let memory = Arc::new(SampleMemory::new());
    memory.replace_buffer(vec![100]);
    let mut recorder = RamRecord::new(96000.0, memory.clone());
    let params = RamRecordParams {
        rec_len: 127,
        ..Default::default()
    };
    allocations::assert_no_alloc(|| {
        recorder.start_resampling(&params);
        for _ in 0..MAX_BUFFER_SAMPLES - 1 {
            recorder.record_mono(-0.125);
        }
        assert!(recorder.is_recording());
        assert_eq!(memory.buffer_len(), 1);
        recorder.record_mono(0.375);
        assert!(!recorder.is_recording());
        assert_eq!(memory.buffer_len(), MAX_BUFFER_SAMPLES);
    });
    let recorded = memory.read_buffer().unwrap();
    assert!(recorded[..MAX_BUFFER_SAMPLES - 1]
        .iter()
        .all(|&s| s == -256));
    assert_eq!(recorded[MAX_BUFFER_SAMPLES - 1], 768);
}
