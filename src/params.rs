use nice_plug::prelude::*;
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

fn parse_bank(string: &str) -> Option<i32> {
    let value = string.trim();
    if value.eq_ignore_ascii_case("OFF") {
        Some(0)
    } else {
        parse_i32_clamped(value, 0, 16)
    }
}

fn format_bank(value: i32) -> String {
    if value == 0 {
        "OFF".into()
    } else {
        value.to_string()
    }
}

fn default_bank() -> i32 {
    std::env::var("ULTRAWAVE_BANK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
        .clamp(0, 16)
}

fn parse_level_db(string: &str) -> Option<f32> {
    let trimmed = string.trim();
    let stripped = trimmed.trim_end_matches([' ', 'd', 'D', 'b', 'B']);
    parse_f32_clamped(stripped, -60.0, 12.0)
}

fn parse_0_127(string: &str) -> Option<i32> {
    parse_i32_clamped(string, 0, 127)
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
    parse_f32_clamped(string.trim().trim_end_matches("st").trim(), -64.0, 63.0)
        .map(|semitones| (semitones.round() as i32 + 64).clamp(0, 127))
}

fn parse_milliseconds(string: &str, max_ms: f32) -> Option<i32> {
    let trimmed = string.trim();
    if trimmed.eq_ignore_ascii_case("OFF") {
        return Some(0);
    }
    parse_f32_clamped(
        trimmed.trim_end_matches([' ', 'm', 'M', 's', 'S']),
        0.0,
        max_ms,
    )
    .filter(|v| v.is_finite())
    .map(|ms| (ms / max_ms * 127.0).round() as i32)
}

fn parse_decay(string: &str) -> Option<i32> {
    parse_milliseconds(string, 4000.0)
}

fn parse_hold(string: &str) -> Option<i32> {
    parse_milliseconds(string, 2000.0)
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
    } else if trimmed.eq_ignore_ascii_case("INF") {
        Some(127)
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

/// Format CUE level: -inf at zero, otherwise a linear 0–127 gain control.
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

/// Format pitch: semitones from center (64)
fn format_pitch(value: i32) -> String {
    format!("{:+} st", value - 64)
}

/// Format decay: ms value based on 0-127 range
fn format_decay(value: i32) -> String {
    if value == 0 {
        "OFF".into()
    } else {
        format!("{} ms", (value as f32 * 4000.0 / 127.0).round() as i32)
    }
}

/// Hold duration; the envelope bypasses only when both HOLD and DEC are zero.
fn format_hold(value: i32) -> String {
    format!("{} ms", (value as f32 * 2000.0 / 127.0).round() as i32)
}

/// Format percentage: 0% to 100%
fn format_percent(value: i32) -> String {
    format!("{}%", value * 100 / 127)
}

/// Format additional repeats; the maximum is an infinite train.
fn format_rtrg(value: i32) -> String {
    match value {
        0 => "OFF".to_string(),
        127 => "INF".to_string(),
        _ => value.to_string(),
    }
}

/// Musical note lengths at known landmarks, quarter-note beats in between.
fn format_rtim(value: i32) -> String {
    match value {
        0 => "OFF".to_string(),
        32 => "1/128".to_string(),
        44 => "1/64T".to_string(),
        64 => "1/64".to_string(),
        77 => "1/32T".to_string(),
        96 => "1/32".to_string(),
        109 => "1/16T".to_string(),
        127 => "1/16".to_string(),
        _ => format!(
            "{:.4} bt",
            crate::machines::ram_play::retrig_interval_beats(value)
        ),
    }
}

#[derive(Params)]
pub struct RamRecordParams {
    #[id = "bank"]
    pub bank: IntParam,
    // Buffer selector (R1-R4)
    #[id = "rec_buf"]
    pub buffer: IntParam,
    /// Rising edge starts a timed capture, falling edge does not truncate it.
    #[id = "capture"]
    pub capture: BoolParam,
    /// Capture once per arming; the runtime latch and RAM are not persisted.
    #[id = "one_shot"]
    pub one_shot: BoolParam,
    // Sidebar - Output/Effects
    #[id = "lev"]
    pub lev: FloatParam,
    #[id = "srr"]
    pub srr: FloatParam,
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
            bank: IntParam::new("Bank", default_bank(), IntRange::Linear { min: 0, max: 16 })
                .with_value_to_string(Arc::new(format_bank))
                .with_string_to_value(Arc::new(parse_bank))
                .non_automatable(),
            buffer: IntParam::new("Buffer", 0, IntRange::Linear { min: 0, max: 3 })
                .with_string_to_value(Arc::new(parse_buffer)),
            capture: BoolParam::new("Capture", false),
            one_shot: BoolParam::new("One Shot", false),
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
            srr: FloatParam::new(
                "SRR",
                0.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 0.9629,
                    factor: 14.0,
                },
            )
            .with_value_to_string(formatters::v2s_f32_rounded(3)),
            dist: IntParam::new("Distortion", 0, IntRange::Linear { min: 0, max: 127 })
                .with_string_to_value(Arc::new(parse_0_127)),
            mlev: IntParam::new("Main Level", 127, IntRange::Linear { min: 0, max: 127 })
                .with_string_to_value(Arc::new(parse_0_127)),
            mbal: IntParam::new("Main Balance", 64, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_balance))
                .with_string_to_value(Arc::new(parse_balance)),
            ilev: IntParam::new("Sidechain Level", 0, IntRange::Linear { min: 0, max: 127 })
                .with_string_to_value(Arc::new(parse_0_127)),
            ibal: IntParam::new(
                "Sidechain Balance",
                64,
                IntRange::Linear { min: 0, max: 127 },
            )
            .with_value_to_string(Arc::new(format_balance))
            .with_string_to_value(Arc::new(parse_balance)),
            cue1: IntParam::new("Dry Monitor", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_cue))
                .with_string_to_value(Arc::new(parse_cue)),
            cue2: IntParam::new("Capture Monitor", 0, IntRange::Linear { min: 0, max: 127 })
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
    #[id = "bank"]
    pub bank: IntParam,
    // Buffer selector (R1-R4)
    #[id = "play_buf"]
    pub buffer: IntParam,
    // Sidebar - Output/Effects
    #[id = "play_lev"]
    pub lev: FloatParam,
    #[id = "play_srr"]
    pub srr: FloatParam,
    #[id = "play_dist"]
    pub dist: IntParam,
    // Main grid
    #[id = "brr"]
    pub brr: IntParam,
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
            bank: IntParam::new("Bank", default_bank(), IntRange::Linear { min: 0, max: 16 })
                .with_value_to_string(Arc::new(format_bank))
                .with_string_to_value(Arc::new(parse_bank))
                .non_automatable(),
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
            srr: FloatParam::new(
                "SRR",
                0.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 0.9629,
                    factor: 14.0,
                },
            )
            .with_value_to_string(formatters::v2s_f32_rounded(3)),
            dist: IntParam::new("Distortion", 0, IntRange::Linear { min: 0, max: 127 })
                .with_string_to_value(Arc::new(parse_0_127)),
            brr: IntParam::new("Bit Reduction", 0, IntRange::Linear { min: 0, max: 127 })
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
        assert_roundtrip_stable(0, 127, format_percent, parse_percent);
        assert_roundtrip_stable(0, 127, format_rtrg, parse_rtrg);
        assert_roundtrip_stable(0, 127, format_rtim, parse_rtim);
    }

    #[test]
    fn parsers_clamp_numeric_ranges() {
        assert_eq!(parse_bank("0"), Some(0));
        assert_eq!(parse_bank("off"), Some(0));
        assert_roundtrip_stable(0, 16, format_bank, parse_bank);
        assert_eq!(parse_buffer("-1"), Some(0));
        assert_eq!(parse_buffer("99"), Some(3));
        assert_eq!(parse_0_127("-5"), Some(0));
        assert_eq!(parse_0_127("999"), Some(127));
        assert_eq!(parse_level_db("-999 dB"), Some(-60.0));
        assert_eq!(parse_level_db("999 dB"), Some(12.0));
    }

    #[test]
    fn displayed_units_match_the_sampler() {
        assert_eq!(format_pitch(76), "+12 st");
        assert_eq!(parse_pitch("-12 st"), Some(52));
        assert_eq!(format_decay(127), "4000 ms");
        assert_eq!(parse_decay("2000 ms"), Some(64));
        assert_eq!(format_hold(127), "2000 ms");
        assert_eq!(format_rtim(127), "1/16");
        assert_eq!(format_rtim(109), "1/16T");
        assert_eq!(format_rtim(0), "OFF");
        assert_eq!(format_rtrg(127), "INF");
        assert_eq!(parse_rtrg("INF"), Some(127));
        for value in 0..=127 {
            assert_eq!(parse_rtim(&format_rtim(value)), Some(value));
        }
        assert_eq!(parse_rtim("500 ms"), None);
        assert_eq!(format_decay(0), "OFF");
    }
}
