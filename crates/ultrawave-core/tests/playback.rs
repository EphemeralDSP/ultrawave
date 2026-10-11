use std::sync::Arc;
use ultrawave_core::dsp::sample_engine::SampleEngine;

#[test]
fn shared_samples_have_independent_cursors_and_no_recording_capacity_limit() {
    let mut codes = vec![0; 441_003];
    codes[441_000..].copy_from_slice(&[-512, 1024, 1536]);
    let samples: Arc<[i16]> = codes.into();
    let mut forward = SampleEngine::new();
    let mut reverse = SampleEngine::new();
    forward.load_shared_buffer(samples.clone());
    reverse.load_shared_buffer(samples);
    forward.trigger(441_000.0);
    reverse.trigger(441_003.0);

    for (a, b) in [(-0.25, 0.75), (0.125, 0.5), (0.5, -0.25)] {
        assert_eq!(forward.read_sample_linear(0.5), a);
        assert_eq!(reverse.read_sample_linear(-1.0), b);
    }
    assert_eq!(forward.playback_position(), 441_001);
    assert_eq!(reverse.playback_position(), 441_000);
}

#[test]
fn progressive_overwrite_reads_old_cells_before_writes_without_resetting_voices() {
    use ultrawave_core::sampler::{Sample, Sampler};
    let mut sampler = Sampler::new(vec![Sample::Ram { capacity: 8 }], 2);
    sampler.record(0, 8).unwrap();
    for code in [100, 200, 300, 400, 500, 600, 700, 800] {
        sampler.render_frame(|_, inputs| inputs[0] = code as f32 / 2048.0);
    }
    sampler.play(0, 0, 1.0, 1.0).unwrap();
    sampler.play(1, 0, -1.0, 1.0).unwrap();
    sampler.record(0, 8).unwrap();
    for (i, expected_reverse) in [800, 700, 600, 500, -400, -300, -200, -100]
        .into_iter()
        .enumerate()
    {
        sampler.render_frame(|voices, inputs| {
            assert_eq!(voices[0], (i + 1) as f32 * 100.0 / 2048.0);
            assert_eq!(voices[1], expected_reverse as f32 / 2048.0);
            inputs[0] = -((i + 1) as f32) * 100.0 / 2048.0;
        });
    }
    assert!(!sampler.capture_status(0).unwrap().recording);
    for i in 0..8 {
        assert_eq!(sampler.sample(0, i), Some(-((i + 1) as i16) * 100));
    }
}

#[test]
fn extension_preserves_latched_voice_bounds_including_the_interpolation_edge() {
    use ultrawave_core::sampler::{Sample, Sampler};
    let mut sampler = Sampler::new(vec![Sample::Ram { capacity: 5 }], 1);
    sampler.record(0, 3).unwrap();
    for code in [100, 200, 300] {
        sampler.render_frame(|_, inputs| inputs[0] = code as f32 / 2048.0);
    }
    sampler.play(0, 0, 0.5, 1.0).unwrap();
    sampler.record(0, 5).unwrap();
    for (i, code) in [100, 300, 500, 550, 600, 600, 0].into_iter().enumerate() {
        sampler.render_frame(|voices, inputs| {
            assert_eq!(voices[0], code as f32 / 2048.0, "frame {i}");
            inputs[0] = (400 + 100 * i) as f32 / 2048.0;
        });
    }

    // A shrink past either direction's cursor kills that voice immediately,
    // even if a later extension occurs before another audio frame is rendered.
    sampler.play(0, 0, 0.5, 1.0).unwrap();
    for _ in 0..4 {
        sampler.render_frame(|_, _| ());
    }
    sampler.record(0, 2).unwrap();
    sampler.record(0, 5).unwrap();
    for _ in 0..8 {
        sampler.render_frame(|voices, inputs| {
            assert_eq!(voices, &[0.0]);
            inputs[0] = 0.25;
        });
    }
    sampler.play(0, 0, -1.0, 1.0).unwrap();
    sampler.record(0, 2).unwrap();
    sampler.record(0, 5).unwrap();
    for _ in 0..8 {
        sampler.render_frame(|voices, inputs| {
            assert_eq!(voices, &[0.0]);
            inputs[0] = 0.25;
        });
    }
}
