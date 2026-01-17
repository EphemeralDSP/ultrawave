use biquad::{Biquad, Coefficients, DirectForm1, ToHertz, Type, Q_BUTTERWORTH_F32};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterMode {
    LowPass,
    BandPass,
    HighPass,
}

impl FilterMode {
    /// Convert 0-127 parameter to filter mode
    /// 0-42: LowPass, 43-84: BandPass, 85-127: HighPass
    pub fn from_param(value: i32) -> Self {
        match value {
            0..=42 => FilterMode::LowPass,
            43..=84 => FilterMode::BandPass,
            85..=127 => FilterMode::HighPass,
            _ => FilterMode::LowPass,
        }
    }

    fn to_biquad_type(self) -> Type<f32> {
        match self {
            FilterMode::LowPass => Type::LowPass,
            FilterMode::BandPass => Type::BandPass,
            FilterMode::HighPass => Type::HighPass,
        }
    }
}

pub struct ResonantFilter {
    filter_l: DirectForm1<f32>,
    filter_r: DirectForm1<f32>,
    sample_rate: f32,
    mode: FilterMode,
}

impl ResonantFilter {
    pub fn new(sample_rate: f32) -> Self {
        let mode = FilterMode::LowPass;
        let coeffs = Self::make_coeffs(sample_rate, 1000.0, Q_BUTTERWORTH_F32, mode);
        Self {
            filter_l: DirectForm1::<f32>::new(coeffs),
            filter_r: DirectForm1::<f32>::new(coeffs),
            sample_rate,
            mode,
        }
    }

    fn make_coeffs(sample_rate: f32, cutoff: f32, q: f32, mode: FilterMode) -> Coefficients<f32> {
        let cutoff_clamped = cutoff.clamp(20.0, sample_rate * 0.45);
        Coefficients::<f32>::from_params(
            mode.to_biquad_type(),
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

    pub fn set_params(&mut self, cutoff: f32, resonance: f32, mode: FilterMode) {
        let q = 0.5 + resonance * 10.0;
        self.mode = mode;
        let coeffs = Self::make_coeffs(self.sample_rate, cutoff, q, mode);
        self.filter_l.update_coefficients(coeffs);
        self.filter_r.update_coefficients(coeffs);
    }

    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        (self.filter_l.run(left), self.filter_r.run(right))
    }

    pub fn reset(&mut self) {
        let coeffs = Self::make_coeffs(self.sample_rate, 1000.0, Q_BUTTERWORTH_F32, self.mode);
        self.filter_l = DirectForm1::<f32>::new(coeffs);
        self.filter_r = DirectForm1::<f32>::new(coeffs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_mode_from_param() {
        assert_eq!(FilterMode::from_param(0), FilterMode::LowPass);
        assert_eq!(FilterMode::from_param(42), FilterMode::LowPass);
        assert_eq!(FilterMode::from_param(43), FilterMode::BandPass);
        assert_eq!(FilterMode::from_param(84), FilterMode::BandPass);
        assert_eq!(FilterMode::from_param(85), FilterMode::HighPass);
        assert_eq!(FilterMode::from_param(127), FilterMode::HighPass);
    }

    #[test]
    fn test_filter_creation() {
        let filter = ResonantFilter::new(44100.0);
        assert_eq!(filter.sample_rate, 44100.0);
        assert_eq!(filter.mode, FilterMode::LowPass);
    }

    #[test]
    fn test_filter_mode_changes() {
        let mut filter = ResonantFilter::new(44100.0);

        filter.set_params(1000.0, 0.5, FilterMode::LowPass);
        assert_eq!(filter.mode, FilterMode::LowPass);

        filter.set_params(1000.0, 0.5, FilterMode::BandPass);
        assert_eq!(filter.mode, FilterMode::BandPass);

        filter.set_params(1000.0, 0.5, FilterMode::HighPass);
        assert_eq!(filter.mode, FilterMode::HighPass);
    }

    #[test]
    fn test_filter_processes_audio() {
        let mut filter = ResonantFilter::new(44100.0);
        filter.set_params(1000.0, 0.5, FilterMode::LowPass);

        let (left, right) = filter.process_stereo(0.5, 0.5);
        assert!(left.is_finite());
        assert!(right.is_finite());
    }

    #[test]
    fn test_filter_cutoff_clamping() {
        let mut filter = ResonantFilter::new(44100.0);

        // Test low cutoff clamping (should clamp to 20 Hz)
        filter.set_params(10.0, 0.5, FilterMode::LowPass);
        let (left, _) = filter.process_stereo(1.0, 1.0);
        assert!(left.is_finite());

        // Test high cutoff clamping (should clamp to 0.45 * sample_rate)
        filter.set_params(30000.0, 0.5, FilterMode::LowPass);
        let (left, _) = filter.process_stereo(1.0, 1.0);
        assert!(left.is_finite());
    }

    #[test]
    fn test_filter_sample_rate_change() {
        let mut filter = ResonantFilter::new(44100.0);
        filter.set_sample_rate(48000.0);
        assert_eq!(filter.sample_rate, 48000.0);
    }
}
