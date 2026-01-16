pub struct SampleRateReducer {
    hold_sample: f32,
    counter: f32,
    reduction_factor: f32,
}

impl SampleRateReducer {
    pub fn new() -> Self {
        Self {
            hold_sample: 0.0,
            counter: 0.0,
            reduction_factor: 1.0,
        }
    }

    pub fn set_reduction(&mut self, factor: f32) {
        self.reduction_factor = factor.max(1.0);
    }

    pub fn process(&mut self, input: f32) -> f32 {
        self.counter += 1.0;
        if self.counter >= self.reduction_factor {
            self.counter = 0.0;
            self.hold_sample = input;
        }
        self.hold_sample
    }

    pub fn reset(&mut self) {
        self.hold_sample = 0.0;
        self.counter = 0.0;
    }
}

impl Default for SampleRateReducer {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Distortion {
    drive: f32,
}

impl Distortion {
    pub fn new() -> Self {
        Self { drive: 0.0 }
    }

    pub fn set_drive(&mut self, drive: f32) {
        self.drive = drive.clamp(0.0, 1.0);
    }

    pub fn process(&self, input: f32) -> f32 {
        if self.drive <= 0.0 {
            return input;
        }
        let gain = 1.0 + self.drive * 20.0;
        let amplified = input * gain;
        (amplified).tanh() / (gain.tanh())
    }
}

impl Default for Distortion {
    fn default() -> Self {
        Self::new()
    }
}

pub fn quantize_12bit(sample: f32) -> f32 {
    let scaled = sample * 2048.0;
    let quantized = scaled.round();
    (quantized / 2048.0).clamp(-1.0, 1.0)
}
