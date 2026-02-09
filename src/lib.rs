use std::sync::atomic::AtomicBool;

pub mod dsp;
pub mod machines;
pub mod offline;
pub mod params;
pub mod plugin;
pub mod widgets;

/// When set, standalone binaries skip the GUI (avoids baseview crash on some systems).
pub static NO_GUI: AtomicBool = AtomicBool::new(false);

// Core library - plugins are exported from separate crates in plugins/

pub mod standalone {
    pub use crate::plugin::*;
}
