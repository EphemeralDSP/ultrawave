use nih_plug::prelude::*;
use std::sync::Arc;

/// Format balance value: L64 to R64 with CENTER at 64
fn format_balance(value: i32) -> String {
    match value {
        64 => "L-R".to_string(),
        0..=63 => format!("L{}", 64 - value),
        65..=127 => format!("R{}", value - 64),
        _ => "L-R".to_string(),
    }
}

/// Format CUE level: -inf at 0, otherwise dB value
fn format_cue(value: i32) -> String {
    if value == 0 {
        "-inf".to_string()
    } else {
        format!("{}", value)
    }
}

/// Format record length: bars based on value ranges
fn format_rec_len(value: i32) -> String {
    match value {
        0..=15 => "1/16".to_string(),
        16..=31 => "1/8".to_string(),
        32..=47 => "1/4".to_string(),
        48..=63 => "1/2".to_string(),
        64..=79 => "1 BAR".to_string(),
        80..=95 => "2 BAR".to_string(),
        96..=111 => "4 BAR".to_string(),
        _ => "INF".to_string(),
    }
}

/// Format sample rate: HI at max, LO at min, otherwise numeric
fn format_rate(value: i32) -> String {
    match value {
        0 => "LO".to_string(),
        127 => "HI".to_string(),
        _ => format!("{}", value),
    }
}

/// Format main/input level: signed offset from center (64)
fn format_level_offset(value: i32) -> String {
    let offset = value - 64;
    if offset == 0 {
        "0".to_string()
    } else if offset > 0 {
        format!("+{}", offset)
    } else {
        format!("{}", offset)
    }
}

/// Format pitch: semitones from center (64)
fn format_pitch(value: i32) -> String {
    let semitones = (value as f32 - 64.0) / 5.33;
    if semitones == 0.0 {
        "+0.00".to_string()
    } else {
        format!("{:+.2}", semitones)
    }
}

/// Format decay: ms value based on 0-127 range
fn format_decay(value: i32) -> String {
    let ms = 10 + (value as f32 * 2.36) as i32;
    format!("{}ms", ms)
}

/// Format hold: OFF at 0, INF at max, otherwise time
fn format_hold(value: i32) -> String {
    match value {
        0 => "OFF".to_string(),
        127 => "INF".to_string(),
        _ => format!("{}%", value * 100 / 127),
    }
}

/// Format bit rate reduction: OFF at 0, otherwise bits
fn format_brr(value: i32) -> String {
    if value == 0 {
        "OFF".to_string()
    } else {
        format!("{}bit", 16 - (value / 8).min(15))
    }
}

/// Format percentage: 0% to 100%
fn format_percent(value: i32) -> String {
    format!("{}%", value * 100 / 127)
}

/// Format retrigger: OFF at 0, otherwise value
fn format_rtrg(value: i32) -> String {
    if value == 0 {
        "OFF".to_string()
    } else {
        format!("{}", value)
    }
}

/// Format retrigger timing: note divisions
fn format_rtim(value: i32) -> String {
    match value {
        0..=15 => "1/64".to_string(),
        16..=31 => "1/32".to_string(),
        32..=47 => "1/16".to_string(),
        48..=63 => "1/8".to_string(),
        64..=79 => "1/4".to_string(),
        80..=95 => "1/2".to_string(),
        96..=111 => "1/1".to_string(),
        _ => "2/1".to_string(),
    }
}

#[derive(Params)]
pub struct RamRecordParams {
    // Buffer selector (R1-R4)
    #[id = "rec_buf"]
    pub buffer: IntParam,
    // Sidebar - Output/Effects
    #[id = "lev"]
    pub lev: FloatParam,
    #[id = "srr"]
    pub srr: IntParam,
    #[id = "dist"]
    pub dist: IntParam,
    // Main grid
    #[id = "mlev"]
    pub mlev: IntParam,
    #[id = "mbal"]
    pub mbal: IntParam,
    #[id = "ilev"]
    pub ilev: IntParam,
    #[id = "ibal"]
    pub ibal: IntParam,
    #[id = "cue1"]
    pub cue1: IntParam,
    #[id = "cue2"]
    pub cue2: IntParam,
    #[id = "rec_len"]
    pub rec_len: IntParam,
    #[id = "rec_rate"]
    pub rec_rate: IntParam,
}

impl Default for RamRecordParams {
    fn default() -> Self {
        Self {
            buffer: IntParam::new("Buffer", 0, IntRange::Linear { min: 0, max: 3 }),
            lev: FloatParam::new(
                "Level",
                0.0,
                FloatRange::Linear {
                    min: -60.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            srr: IntParam::new("SRR", 0, IntRange::Linear { min: 0, max: 127 }),
            dist: IntParam::new("Distortion", 0, IntRange::Linear { min: 0, max: 127 }),
            mlev: IntParam::new("Main Level", 64, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_level_offset)),
            mbal: IntParam::new("Main Balance", 64, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_balance)),
            ilev: IntParam::new("Input Level", 64, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_level_offset)),
            ibal: IntParam::new("Input Balance", 64, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_balance)),
            cue1: IntParam::new("CUE1", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_cue)),
            cue2: IntParam::new("CUE2", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_cue)),
            rec_len: IntParam::new("Rec Length", 64, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_rec_len)),
            rec_rate: IntParam::new("Rec Rate", 127, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_rate)),
        }
    }
}

#[derive(Params)]
pub struct RamPlayParams {
    // Buffer selector (R1-R4)
    #[id = "play_buf"]
    pub buffer: IntParam,
    // Sidebar - Output/Effects
    #[id = "play_lev"]
    pub lev: FloatParam,
    #[id = "play_srr"]
    pub srr: IntParam,
    #[id = "play_dist"]
    pub dist: IntParam,
    // Main grid
    #[id = "strt"]
    pub strt: IntParam,
    #[id = "end"]
    pub end: IntParam,
    #[id = "pitch"]
    pub pitch: IntParam,
    #[id = "hold"]
    pub hold: IntParam,
    #[id = "dec"]
    pub dec: IntParam,
    #[id = "rtrg"]
    pub rtrg: IntParam,
    #[id = "rtim"]
    pub rtim: IntParam,
}

impl Default for RamPlayParams {
    fn default() -> Self {
        Self {
            buffer: IntParam::new("Buffer", 0, IntRange::Linear { min: 0, max: 3 }),
            lev: FloatParam::new(
                "Level",
                0.0,
                FloatRange::Linear {
                    min: -60.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            srr: IntParam::new("SRR", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_brr)),
            dist: IntParam::new("Distortion", 0, IntRange::Linear { min: 0, max: 127 }),
            strt: IntParam::new("Start", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_percent)),
            end: IntParam::new("End", 127, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_percent)),
            pitch: IntParam::new("Pitch", 64, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_pitch)),
            hold: IntParam::new("Hold", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_hold)),
            dec: IntParam::new("Decay", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_decay)),
            rtrg: IntParam::new("Retrigger", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_rtrg)),
            rtim: IntParam::new("Retrig Time", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_rtim)),
        }
    }
}

#[derive(Params)]
pub struct UltrawaveParams {
    /// The output's level in dB.
    #[id = "gain"]
    pub gain: FloatParam,

    /// Channel selector (0-7)
    #[id = "chan"]
    pub channel: IntParam,

    #[nested(group = "RAM Record")]
    pub record: RamRecordParams,

    #[nested(group = "RAM Play")]
    pub play: RamPlayParams,

    // Filter Parameters
    #[id = "fltf"]
    pub fltf: IntParam,
    #[id = "fltq"]
    pub fltq: IntParam,
    #[id = "fltw"]
    pub fltw: IntParam,

    // Output
    #[id = "vol"]
    pub vol: IntParam,
    #[id = "pan"]
    pub pan: IntParam,
}

impl Default for UltrawaveParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new(
                "Gain",
                0.0,
                FloatRange::Linear {
                    min: -80.0,
                    max: 0.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            channel: IntParam::new("Channel", 0, IntRange::Linear { min: 0, max: 7 }),

            record: RamRecordParams::default(),
            play: RamPlayParams::default(),

            // Filter (0-127)
            fltf: IntParam::new("Filter Freq", 64, IntRange::Linear { min: 0, max: 127 }),
            fltq: IntParam::new("Filter Q", 0, IntRange::Linear { min: 0, max: 127 }),
            fltw: IntParam::new("Filter Width", 0, IntRange::Linear { min: 0, max: 127 }),

            // Output (0-127)
            vol: IntParam::new("Volume", 100, IntRange::Linear { min: 0, max: 127 }),
            pan: IntParam::new("Pan", 64, IntRange::Linear { min: 0, max: 127 }),
        }
    }
}
