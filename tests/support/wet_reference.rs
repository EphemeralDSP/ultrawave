/// Independent f64 reference: subtract an exponentially weighted input mean,
/// rather than using the production high-pass output-feedback recurrence.
/// Inputs are the fixture's known raw playback, including idle frames.
pub fn wet_reference(input: impl IntoIterator<Item = f32>) -> Vec<f32> {
    let pole = (-std::f64::consts::TAU * 17.0 / 44100.0).exp();
    let mut mean = 0.0;
    input
        .into_iter()
        .map(|sample| {
            let output = (1.0 + pole) * 0.5 * (sample as f64 - mean);
            mean = (1.0 - pole) * sample as f64 + pole * mean;
            output as f32
        })
        .collect()
}
