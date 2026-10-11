//! Sample-and-hold rate reduction copied from `ephemeral-dsp` at revision
//! 243d21b7aef5337ea8ff0e30750c01d1fa3af296 (`rust/ephemeral-dsp/src/lib.rs`).
//! Copyright (c) 2026 EphemeralDSP and contributors. MIT licensed; see LICENSE.
//! Uses Rust's `f32::floor` instead of the original C `floorf` binding.

#[derive(Clone, Copy, Debug)]
pub struct Decimator {
    phase: f32,
    history: f32,
}

impl Decimator {
    pub const fn new() -> Self {
        Self {
            phase: 0.0,
            history: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32, percentage: f32) -> f32 {
        self.phase += 1.0 - percentage;
        let ready = self.phase >= 1.0;
        self.phase -= self.phase.floor();
        if ready {
            self.history = sample;
        }
        self.history
    }
}

impl Default for Decimator {
    fn default() -> Self {
        Self::new()
    }
}
