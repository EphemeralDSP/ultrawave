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
