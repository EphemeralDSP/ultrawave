use std::sync::atomic::AtomicBool;

pub mod dsp;
pub mod offline;
pub mod params;
pub mod plugin;
pub mod widgets;

pub use ultrawave_core::{machines, Decimator};

/// When set, standalone binaries skip the GUI (avoids baseview crash on some systems).
pub static NO_GUI: AtomicBool = AtomicBool::new(false);

// Core library - the plugin is exported from plugins/ultrawave.
