use nih_plug::prelude::*;
use ultrawave::plugin::PlayPlugin;

// Re-export and rename for clarity
pub type UltrawavePlay = PlayPlugin;

nih_export_clap!(UltrawavePlay);
nih_export_vst3!(UltrawavePlay);
