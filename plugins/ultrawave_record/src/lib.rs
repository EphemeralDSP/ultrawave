use nih_plug::prelude::*;
use ultrawave::plugin::StandaloneRecord;

// Re-export and rename for clarity
pub type UltrawaveRecord = StandaloneRecord;

nih_export_clap!(UltrawaveRecord);
nih_export_vst3!(UltrawaveRecord);
