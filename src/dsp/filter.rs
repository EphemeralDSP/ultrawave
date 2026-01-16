use biquad::{Biquad, Coefficients, DirectForm1, ToHertz, Type, Q_BUTTERWORTH_F32};

pub struct ResonantFilter {
    filter_l: DirectForm1<f32>,
    filter_r: DirectForm1<f32>,
    sample_rate: f32,
}

impl ResonantFilter {
    pub fn new(sample_rate: f32) -> Self {
        let coeffs = Self::make_coeffs(sample_rate, 1000.0, Q_BUTTERWORTH_F32);
        Self {
            filter_l: DirectForm1::<f32>::new(coeffs),
            filter_r: DirectForm1::<f32>::new(coeffs),
            sample_rate,
        }
    }

    fn make_coeffs(sample_rate: f32, cutoff: f32, q: f32) -> Coefficients<f32> {
        let cutoff_clamped = cutoff.clamp(20.0, sample_rate * 0.45);
        Coefficients::<f32>::from_params(
            Type::LowPass,
            sample_rate.hz(),
            cutoff_clamped.hz(),
            q,
        )
        .unwrap_or_else(|_| {
            Coefficients::<f32>::from_params(
                Type::LowPass,
                sample_rate.hz(),
                1000.0.hz(),
                Q_BUTTERWORTH_F32,
            )
            .unwrap()
        })
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.reset();
    }

    pub fn set_params(&mut self, cutoff: f32, resonance: f32) {
        let q = 0.5 + resonance * 10.0;
        let coeffs = Self::make_coeffs(self.sample_rate, cutoff, q);
        self.filter_l.update_coefficients(coeffs);
        self.filter_r.update_coefficients(coeffs);
    }

    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        (self.filter_l.run(left), self.filter_r.run(right))
    }

    pub fn reset(&mut self) {
        let coeffs = Self::make_coeffs(self.sample_rate, 1000.0, Q_BUTTERWORTH_F32);
        self.filter_l = DirectForm1::<f32>::new(coeffs);
        self.filter_r = DirectForm1::<f32>::new(coeffs);
    }
}
