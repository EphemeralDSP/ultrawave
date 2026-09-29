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

fn parse_rec_len(string: &str) -> Option<i32> {
    parse_from_formatter_lookup(string, 0, 127, format_rec_len).or_else(|| parse_0_127(string))
}

fn parse_rate(string: &str) -> Option<i32> {
    parse_from_formatter_lookup(string, 0, 127, format_rate).or_else(|| parse_0_127(string))
}

fn format_amount(value: i32) -> String {
    format!("{:.1}%", value as f32 * 100.0 / 127.0)
}

fn parse_amount(string: &str) -> Option<i32> {
    if let Some(percent) = string.trim().strip_suffix('%') {
        percent
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .map(|v| (v.clamp(0.0, 100.0) * 127.0 / 100.0).round() as i32)
    } else {
        parse_0_127(string)
    }
}

fn format_srr(value: f32) -> String {
    format!("{:.1}%", value / 0.9629 * 100.0)
}

fn parse_srr(string: &str) -> Option<f32> {
    let value = string
        .trim()
        .trim_end_matches('%')
        .trim()
        .parse::<f32>()
        .ok()?;
    value
        .is_finite()
        .then(|| value.clamp(0.0, 100.0) / 100.0 * 0.9629)
}

fn format_bits(value: i32) -> String {
    format!("{} bits", crate::dsp::sample_engine::playback_bits(value))
}

fn parse_bits(string: &str) -> Option<i32> {
    parse_from_formatter_lookup(string, 0, 127, format_bits).or_else(|| parse_0_127(string))
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
    parse_from_formatter_lookup(string, 0, 127, format_rtrg).or_else(|| parse_0_127(string))
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

/// Hosts without editor context show the rate divisor; the editor shows Hz.
fn format_rate(value: i32) -> String {
    format!(
        "÷{:.0}",
        crate::machines::ram_record::capture_rate_divisor(value).ceil()
    )
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

/// Total hits including the first, not additional repeats or the raw MD value.
fn format_rtrg(value: i32) -> String {
    match value {
        0 | 1 => "1 hit".to_string(),
        127 => "INF".to_string(),
        _ => format!(
            "{} hits",
            crate::machines::ram_play::retrig_total_hits(value).unwrap()
        ),
    }
}

/// Host parameter text cannot observe tempo. The editor shows actual milliseconds.
fn format_rtim(value: i32) -> String {
    format!("MD {value}")
}

#[derive(Params)]
pub struct UltrawaveParams {
    #[id = "record_note"]
    pub record_note: IntParam,
    #[id = "play_note"]
    pub play_note: IntParam,
    #[id = "master_mode"]
    pub master_mode: BoolParam,
    #[id = "playback"]
    pub playback: BoolParam,
    #[id = "rearm"]
    pub rearm: BoolParam,
    #[nested(group = "Capture")]
    pub record: RamRecordParams,
    #[nested(group = "Play")]
    pub play: RamPlayParams,
}

impl Default for UltrawaveParams {
    fn default() -> Self {
        Self {
            record_note: IntParam::new("Record Note", 48, IntRange::Linear { min: 0, max: 127 })
                .with_string_to_value(Arc::new(parse_0_127)),
            play_note: IntParam::new("Play Note", 50, IntRange::Linear { min: 0, max: 127 })
                .with_string_to_value(Arc::new(parse_0_127)),
            master_mode: BoolParam::new("Resample", true),
            playback: BoolParam::new("Playback", false),
            rearm: BoolParam::new("Re-arm", false),
            record: RamRecordParams::default(),
            play: RamPlayParams::default(),
        }
    }
}

#[derive(Params)]
pub struct RamRecordParams {
    /// Rising edge starts a timed capture, falling edge does not truncate it.
    #[id = "capture"]
    pub capture: BoolParam,
    /// Capture once per arming; the runtime latch and RAM are not persisted.
    #[id = "one_shot"]
    pub one_shot: BoolParam,
    // Capture sound
    #[id = "lev"]
    pub lev: FloatParam,
    #[id = "srr"]
    pub srr: FloatParam,
    #[id = "dist"]
    pub dist: IntParam,
    // Sources and duration
    #[id = "mlev"]
    pub mlev: IntParam,
    #[id = "mbal"]
    pub mbal: IntParam,
    #[id = "ilev"]
    pub ilev: IntParam,
    #[id = "ibal"]
    pub ibal: IntParam,
    #[id = "rec_len"]
    pub rec_len: IntParam,
    #[id = "rec_rate"]
    pub rec_rate: IntParam,
}

impl Default for RamRecordParams {
    fn default() -> Self {
        Self {
            capture: BoolParam::new("Capture", false),
            one_shot: BoolParam::new("Record Once", false),
            lev: FloatParam::new(
                "Capture Level",
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
                "Capture SRR",
                0.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 0.9629,
                    factor: 14.0,
                },
            )
            .with_value_to_string(Arc::new(format_srr))
            .with_string_to_value(Arc::new(parse_srr)),
            dist: IntParam::new(
                "Capture Distortion",
                0,
                IntRange::Linear { min: 0, max: 127 },
            )
            .with_value_to_string(Arc::new(format_amount))
            .with_string_to_value(Arc::new(parse_amount)),
            mlev: IntParam::new(
                "Main Capture Level",
                127,
                IntRange::Linear { min: 0, max: 127 },
            )
            .with_value_to_string(Arc::new(format_amount))
            .with_string_to_value(Arc::new(parse_amount)),
            mbal: IntParam::new(
                "Track Input Balance",
                64,
                IntRange::Linear { min: 0, max: 127 },
            )
            .with_value_to_string(Arc::new(format_balance))
            .with_string_to_value(Arc::new(parse_balance)),
            ilev: IntParam::new("Sidechain Level", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_amount))
                .with_string_to_value(Arc::new(parse_amount)),
            ibal: IntParam::new(
                "Sidechain Balance",
                64,
                IntRange::Linear { min: 0, max: 127 },
            )
            .with_value_to_string(Arc::new(format_balance))
            .with_string_to_value(Arc::new(parse_balance)),
            rec_len: IntParam::new("Capture Length", 48, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_rec_len))
                .with_string_to_value(Arc::new(parse_rec_len)),
            rec_rate: IntParam::new("Capture Rate", 127, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_rate))
                .with_string_to_value(Arc::new(parse_rate)),
        }
    }
}

#[derive(Params)]
pub struct RamPlayParams {
    // Playback sound
    #[id = "play_lev"]
    pub lev: FloatParam,
    #[id = "play_srr"]
    pub srr: FloatParam,
    #[id = "play_dist"]
    pub dist: IntParam,
    // Region, envelope and repeats
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
            lev: FloatParam::new(
                "Playback Level",
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
                "Playback SRR",
                0.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 0.9629,
                    factor: 14.0,
                },
            )
            .with_value_to_string(Arc::new(format_srr))
            .with_string_to_value(Arc::new(parse_srr)),
            dist: IntParam::new(
                "Playback Distortion",
                0,
                IntRange::Linear { min: 0, max: 127 },
            )
            .with_value_to_string(Arc::new(format_amount))
            .with_string_to_value(Arc::new(parse_amount)),
            brr: IntParam::new("Bit Reduction", 0, IntRange::Linear { min: 0, max: 127 })
                .with_value_to_string(Arc::new(format_bits))
                .with_string_to_value(Arc::new(parse_bits)),
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
        assert_roundtrip_stable(0, 127, format_rec_len, parse_rec_len);
        assert_roundtrip_stable(0, 127, format_rate, parse_rate);
        assert_roundtrip_stable(0, 127, format_amount, parse_amount);
        assert_roundtrip_stable(0, 127, format_bits, parse_bits);
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
        assert_eq!(format_rtim(127), "MD 127");
        assert_eq!(format_rtim(109), "MD 109");
        assert_eq!(format_rtim(0), "MD 0");
        assert_eq!(format_rtrg(0), "1 hit");
        assert_eq!(format_rtrg(1), "1 hit");
        assert_eq!(format_rtrg(2), "2 hits");
        assert_eq!(format_rtrg(96), "104 hits");
        assert_eq!(format_rtrg(126), "156 hits");
        assert_eq!(parse_rtrg("156 hits"), Some(126));
        assert_eq!(parse_rtrg("126"), Some(126));
        assert_eq!(format_rtrg(127), "INF");
        assert_eq!(parse_rtrg("INF"), Some(127));
        for value in 0..=127 {
            assert_eq!(parse_rtim(&format_rtim(value)), Some(value));
        }
        assert_eq!(parse_rtim("500 ms"), None);
        assert_eq!(format_decay(0), "OFF");
    }

    #[test]
    fn amounts_and_bits_show_units_without_changing_raw_parameters() {
        assert_eq!(format_amount(91), "71.7%");
        assert_eq!(parse_amount("71.7%"), Some(91));
        assert_eq!(parse_amount("91"), Some(91));
        for value in 0..=127 {
            assert_eq!(parse_amount(&format_amount(value)), Some(value));
        }
        assert_eq!(format_srr(0.48145), "50.0%");
        assert!((parse_srr("50.0%").unwrap() - 0.48145).abs() < 1e-6);
        assert_eq!(format_bits(12), "12 bits");
        assert_eq!(format_bits(13), "11 bits");
        assert_eq!(format_bits(126), "3 bits");
        assert_eq!(format_bits(127), "2 bits");
        assert_eq!(format_rate(127), "÷1");
        assert_eq!(format_rate(126), "÷2");
        for invalid in ["NaN%", "inf%", "wrong"] {
            assert_eq!(parse_amount(invalid), None);
            assert_eq!(parse_srr(invalid), None);
        }
    }
}
