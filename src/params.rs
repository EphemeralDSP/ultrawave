use nih_plug::prelude::*;
use std::sync::Arc;

fn clamp_i32(value: i32, min: i32, max: i32) -> i32 {
    value.clamp(min, max)
}

fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
    value.clamp(min, max)
}

fn parse_i32_clamped(string: &str, min: i32, max: i32) -> Option<i32> {
    string
        .trim()
        .parse::<i32>()
        .ok()
        .map(|value| clamp_i32(value, min, max))
}

fn parse_f32_clamped(string: &str, min: f32, max: f32) -> Option<f32> {
    string
        .trim()
        .parse::<f32>()
        .ok()
        .map(|value| clamp_f32(value, min, max))
}

fn parse_from_formatter_lookup(
    string: &str,
    min: i32,
    max: i32,
    formatter: fn(i32) -> String,
) -> Option<i32> {
    let trimmed = string.trim();
    (min..=max).find(|value| formatter(*value).eq_ignore_ascii_case(trimmed))
}

fn parse_buffer(string: &str) -> Option<i32> {
    parse_i32_clamped(string, 0, 3)
}

fn parse_level_db(string: &str) -> Option<f32> {
    let trimmed = string.trim();
    let stripped = trimmed.trim_end_matches(&[' ', 'd', 'D', 'b', 'B']);
    parse_f32_clamped(stripped, -60.0, 12.0)
}

fn parse_0_127(string: &str) -> Option<i32> {
    parse_i32_clamped(string, 0, 127)
}

fn parse_level_offset(string: &str) -> Option<i32> {
    parse_i32_clamped(string, -64, 63).map(|offset| clamp_i32(offset + 64, 0, 127))
}

fn parse_balance(string: &str) -> Option<i32> {
    let trimmed = string.trim();
    if trimmed.eq_ignore_ascii_case("L-R") {
        return Some(64);
    }

    if let Some(rest) = trimmed
        .strip_prefix('L')
        .or_else(|| trimmed.strip_prefix('l'))
    {
        return parse_i32_clamped(rest, 0, 64).map(|amount| clamp_i32(64 - amount, 0, 127));
    }

    if let Some(rest) = trimmed
        .strip_prefix('R')
        .or_else(|| trimmed.strip_prefix('r'))
    {
        return parse_i32_clamped(rest, 0, 64).map(|amount| clamp_i32(64 + amount, 0, 127));
    }

    parse_0_127(trimmed)
}

fn parse_cue(string: &str) -> Option<i32> {
    let trimmed = string.trim();
    if trimmed.eq_ignore_ascii_case("-inf") {
        Some(0)
    } else {
        parse_0_127(trimmed)
    }
}

fn parse_rec_len(string: &str) -> Option<i32> {
    parse_from_formatter_lookup(string, 0, 127, format_rec_len).or_else(|| parse_0_127(string))
}

fn parse_rate(string: &str) -> Option<i32> {
    let trimmed = string.trim();
    if trimmed.eq_ignore_ascii_case("LO") {
        Some(0)
    } else if trimmed.eq_ignore_ascii_case("HI") {
        Some(127)
    } else {
        parse_0_127(trimmed)
    }
}

fn parse_pitch(string: &str) -> Option<i32> {
    if let Some(value) = parse_from_formatter_lookup(string, 0, 127, format_pitch) {
        return Some(value);
    }

    parse_f32_clamped(string, -64.0, 64.0).map(|semitones| {
        let value = (semitones * 5.33 + 64.0).round() as i32;
        clamp_i32(value, 0, 127)
    })
}

fn parse_decay(string: &str) -> Option<i32> {
    if let Some(value) = parse_from_formatter_lookup(string, 0, 127, format_decay) {
        return Some(value);
    }

    let trimmed = string.trim();
    let ms_part = trimmed.trim_end_matches(&[' ', 'm', 'M', 's', 'S']);
    ms_part.parse::<i32>().ok().map(|ms| {
        let value = ((ms - 10) as f32 / 2.36).round() as i32;
        clamp_i32(value, 0, 127)
    })
}

fn parse_hold(string: &str) -> Option<i32> {
    if let Some(value) = parse_from_formatter_lookup(string, 0, 127, format_hold) {
        return Some(value);
    }

    let trimmed = string.trim();
    if trimmed.eq_ignore_ascii_case("OFF") {
        return Some(0);
    }
    if trimmed.eq_ignore_ascii_case("INF") {
        return Some(127);
    }

    if let Some(percent_str) = trimmed.strip_suffix('%') {
        return parse_i32_clamped(percent_str, 0, 100)
            .map(|percent| clamp_i32((percent as f32 / 100.0 * 127.0).round() as i32, 0, 127));
    }

    parse_0_127(trimmed)
}

fn parse_brr(string: &str) -> Option<i32> {
    if let Some(value) = parse_from_formatter_lookup(string, 0, 127, format_brr) {
        return Some(value);
    }

    let trimmed = string.trim();
    if trimmed.eq_ignore_ascii_case("OFF") {
        return Some(0);
    }

    let bit_part = trimmed.trim_end_matches(&[' ', 'b', 'B', 'i', 'I', 't', 'T']);
    if let Ok(bits) = bit_part.parse::<i32>() {
        let bits = clamp_i32(bits, 1, 16);
        let canonical = if bits >= 16 { 1 } else { (16 - bits) * 8 };
        return Some(clamp_i32(canonical, 0, 127));
    }

    parse_0_127(trimmed)
}

fn parse_percent(string: &str) -> Option<i32> {
    if let Some(value) = parse_from_formatter_lookup(string, 0, 127, format_percent) {
        return Some(value);
    }

    let trimmed = string.trim();
    if let Some(percent_str) = trimmed.strip_suffix('%') {
        return parse_i32_clamped(percent_str, 0, 100)
            .map(|percent| clamp_i32((percent as f32 / 100.0 * 127.0).round() as i32, 0, 127));
    }

    parse_0_127(trimmed)
}

fn parse_rtrg(string: &str) -> Option<i32> {
    let trimmed = string.trim();
    if trimmed.eq_ignore_ascii_case("OFF") {
        Some(0)
    } else {
        parse_0_127(trimmed)
    }
}

fn parse_rtim(string: &str) -> Option<i32> {
    parse_from_formatter_lookup(string, 0, 127, format_rtim).or_else(|| parse_0_127(string))
}

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

/// Format record length: beat-synced durations
fn format_rec_len(value: i32) -> String {
    match value {
        0..=15 => "1/8".to_string(),
        16..=31 => "1/4".to_string(),
        32..=47 => "1/2".to_string(),
        48..=63 => "1 BAR".to_string(),
        64..=79 => "2 BAR".to_string(),
        80..=95 => "4 BAR".to_string(),
        96..=111 => "8 BAR".to_string(),
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
            buffer: IntParam::new("Buffer", 0, IntRange::Linear { min: 0, max: 3 })
                .with_string_to_value(Arc::new(parse_buffer)),
            lev: FloatParam::new(
                "Level",
                0.0,
                FloatRange::Linear {
                    min: -60.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1))
            .with_string_to_value(Arc::new(parse_level_db)),
            srr: IntParam::new("SRR", 0, IntRange::Linear { min: 0, max: 127 })
                .with_string_to_value(Arc::new(parse_0_127)),
            dist: IntParam::new("Distortion", 0, IntRange::Linear { min: 0, max: 127 })
                .with_string_to_value(Arc::new(parse_0_127)),
            mlev: IntParam::new("Main Level", 64, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_level_offset))
                .with_string_to_value(Arc::new(parse_level_offset)),
            mbal: IntParam::new("Main Balance", 64, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_balance))
                .with_string_to_value(Arc::new(parse_balance)),
            ilev: IntParam::new("Input Level", 64, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_level_offset))
                .with_string_to_value(Arc::new(parse_level_offset)),
            ibal: IntParam::new("Input Balance", 64, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_balance))
                .with_string_to_value(Arc::new(parse_balance)),
            cue1: IntParam::new("CUE1", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_cue))
                .with_string_to_value(Arc::new(parse_cue)),
            cue2: IntParam::new("CUE2", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_cue))
                .with_string_to_value(Arc::new(parse_cue)),
            rec_len: IntParam::new("Rec Length", 48, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_rec_len))
                .with_string_to_value(Arc::new(parse_rec_len)),
            rec_rate: IntParam::new("Rec Rate", 127, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_rate))
                .with_string_to_value(Arc::new(parse_rate)),
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
            buffer: IntParam::new("Buffer", 0, IntRange::Linear { min: 0, max: 3 })
                .with_string_to_value(Arc::new(parse_buffer)),
            lev: FloatParam::new(
                "Level",
                0.0,
                FloatRange::Linear {
                    min: -60.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1))
            .with_string_to_value(Arc::new(parse_level_db)),
            srr: IntParam::new("SRR", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_brr))
                .with_string_to_value(Arc::new(parse_brr)),
            dist: IntParam::new("Distortion", 0, IntRange::Linear { min: 0, max: 127 })
                .with_string_to_value(Arc::new(parse_0_127)),
            strt: IntParam::new("Start", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_percent))
                .with_string_to_value(Arc::new(parse_percent)),
            end: IntParam::new("End", 127, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_percent))
                .with_string_to_value(Arc::new(parse_percent)),
            pitch: IntParam::new("Pitch", 64, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_pitch))
                .with_string_to_value(Arc::new(parse_pitch)),
            hold: IntParam::new("Hold", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_hold))
                .with_string_to_value(Arc::new(parse_hold)),
            dec: IntParam::new("Decay", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_decay))
                .with_string_to_value(Arc::new(parse_decay)),
            rtrg: IntParam::new("Retrigger", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_rtrg))
                .with_string_to_value(Arc::new(parse_rtrg)),
            rtim: IntParam::new("Retrig Time", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_rtim))
                .with_string_to_value(Arc::new(parse_rtim)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_roundtrip_stable(
        min: i32,
        max: i32,
        formatter: fn(i32) -> String,
        parser: fn(&str) -> Option<i32>,
    ) {
        for value in min..=max {
            let formatted = formatter(value);
            let parsed =
                parser(&formatted).unwrap_or_else(|| panic!("failed to parse {formatted}"));
            assert!((min..=max).contains(&parsed));
            assert_eq!(formatter(parsed), formatted);
        }
    }

    #[test]
    fn record_formatters_roundtrip_stable() {
        assert_roundtrip_stable(0, 127, format_level_offset, parse_level_offset);
        assert_roundtrip_stable(0, 127, format_balance, parse_balance);
        assert_roundtrip_stable(0, 127, format_cue, parse_cue);
        assert_roundtrip_stable(0, 127, format_rec_len, parse_rec_len);
        assert_roundtrip_stable(0, 127, format_rate, parse_rate);
    }

    #[test]
    fn play_formatters_roundtrip_stable() {
        assert_roundtrip_stable(0, 127, format_pitch, parse_pitch);
        assert_roundtrip_stable(0, 127, format_decay, parse_decay);
        assert_roundtrip_stable(0, 127, format_hold, parse_hold);
        assert_roundtrip_stable(0, 127, format_brr, parse_brr);
        assert_roundtrip_stable(0, 127, format_percent, parse_percent);
        assert_roundtrip_stable(0, 127, format_rtrg, parse_rtrg);
        assert_roundtrip_stable(0, 127, format_rtim, parse_rtim);
    }

    #[test]
    fn parsers_clamp_numeric_ranges() {
        assert_eq!(parse_buffer("-1"), Some(0));
        assert_eq!(parse_buffer("99"), Some(3));
        assert_eq!(parse_0_127("-5"), Some(0));
        assert_eq!(parse_0_127("999"), Some(127));
        assert_eq!(parse_level_db("-999 dB"), Some(-60.0));
        assert_eq!(parse_level_db("999 dB"), Some(12.0));
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
