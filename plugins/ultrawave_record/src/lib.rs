use nih_plug::prelude::*;
use ultrawave::plugin::RecordPlugin;

// Re-export and rename for clarity
pub type UltrawaveRecord = RecordPlugin;

nih_export_clap!(UltrawaveRecord);
nih_export_vst3!(UltrawaveRecord);
