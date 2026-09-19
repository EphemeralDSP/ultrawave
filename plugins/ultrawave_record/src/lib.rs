use nice_plug::prelude::*;
use ultrawave::plugin::RecordPlugin;

// Re-export and rename for clarity
pub type UltrawaveRecord = RecordPlugin;

nice_export_clap!(UltrawaveRecord);
nice_export_vst3!(UltrawaveRecord);
