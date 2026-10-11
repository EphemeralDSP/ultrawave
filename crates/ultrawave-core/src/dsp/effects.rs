/// Conservative wet-path coloration: the measured RAM1 playback and internal
/// generation paths both lose about 1.1–1.2 dB at 30 Hz. This normalized one-pole
/// high-pass models that broad low cut, not an identified hardware circuit.
/// Keep state across voice boundaries; reset only with the host audio state.
pub struct DcBlocker {
    pole: f32,
    gain: f32,
    previous_input: f32,
    previous_output: f32,
}

impl DcBlocker {
    pub fn new(sample_rate: f32) -> Self {
        let pole = (-std::f32::consts::TAU * 17.0 / sample_rate).exp();
        Self {
            pole,
            gain: (1.0 + pole) * 0.5,
            previous_input: 0.0,
            previous_output: 0.0,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let output = self.gain * (input - self.previous_input) + self.pole * self.previous_output;
        self.previous_input = input;
        // Avoid denormal work after long silent periods, far below audibility.
        self.previous_output = if output.abs() < 1e-20 { 0.0 } else { output };
        self.previous_output
    }
}

pub struct Distortion {
    gain: f32,
    normalization: f32,
}

impl Distortion {
    pub fn new() -> Self {
        Self {
            gain: 1.0,
            normalization: 1.0,
        }
    }

    pub fn set_drive(&mut self, drive: f32) {
        self.gain = 1.0 + drive.clamp(0.0, 1.0) * 20.0;
        self.normalization = self.gain.tanh();
    }

    pub fn process(&self, input: f32) -> f32 {
        if self.gain == 1.0 {
            return input;
        }
        (input * self.gain).tanh() / self.normalization
    }
}

impl Default for Distortion {
    fn default() -> Self {
        Self::new()
    }
}
