//! Host-independent sample memory, capture, playback and DSP.
//!
//! Device IO, file formats, plugin parameters and UI belong to callers.

pub mod dsp;
pub mod machines;

pub use dsp::decimator::Decimator;
