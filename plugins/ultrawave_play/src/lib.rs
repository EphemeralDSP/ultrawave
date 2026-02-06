use nih_plug::prelude::*;
use ultrawave::standalone::StandalonePlay;

// Re-export and rename for clarity
pub type UltrawavePlay = StandalonePlay;

nih_export_clap!(UltrawavePlay);
nih_export_vst3!(UltrawavePlay);
