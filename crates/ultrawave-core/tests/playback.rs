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
