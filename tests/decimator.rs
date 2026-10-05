use ultrawave::Decimator;

#[test]
fn zero_reduction_passes_every_sample_through() {
    let mut decimator = Decimator::new();
    for sample in [-0.75, 0.125, 0.0, 0.875, -0.25] {
        assert_eq!(decimator.process(sample, 0.0), sample);
    }
}

#[test]
fn fractional_intervals_keep_the_remainder_and_capture_at_the_boundary() {
    let mut decimator = Decimator::new();
    // A 5/8 phase step captures inputs 2, 4, 5, 7, 8 and 10. Input 8
    // lands exactly on phase 1; resetting phase on capture loses this pattern.
    let input = [
        -0.75, 0.25, 0.875, -0.5, 0.125, -0.875, 0.5, -0.25, 0.75, 0.375,
    ];
    let expected = [
        0.0, 0.25, 0.25, -0.5, 0.125, 0.125, 0.5, -0.25, -0.25, 0.375,
    ];
    for (sample, expected) in input.into_iter().zip(expected) {
        assert_eq!(decimator.process(sample, 0.375), expected);
    }
}

#[test]
fn changing_reduction_preserves_phase_and_the_held_sample() {
    let mut decimator = Decimator::new();
    // Bypass still captures history without discarding the fractional phase;
    // full reduction holds that history without advancing phase.
    for (sample, reduction, expected) in [
        (0.125, 0.75, 0.0),
        (-0.25, 0.75, 0.0),
        (0.375, 0.875, 0.0),
        (-0.5, 0.0, -0.5),
        (0.625, 1.0, -0.5),
        (-0.75, 0.625, -0.75),
        (0.875, 0.5, -0.75),
        (-0.125, 0.5, -0.125),
    ] {
        assert_eq!(decimator.process(sample, reduction), expected);
    }
}
