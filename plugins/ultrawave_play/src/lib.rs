use nice_plug::prelude::*;
use ultrawave::plugin::PlayPlugin;

// Re-export and rename for clarity
pub type UltrawavePlay = PlayPlugin;

nice_export_clap!(UltrawavePlay);
nice_export_vst3!(UltrawavePlay);
