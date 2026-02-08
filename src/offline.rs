use crate::machines::ram_play::{RamPlay, RamPlayParams};
use crate::machines::ram_record::{RamRecord, RamRecordParams};
use crate::machines::shared::{
    get_global_registry, register_plugin_instance, unregister_plugin_instance,
};
use std::path::{Path, PathBuf};

const SAMPLE_RATE: u32 = 44100;
const SAMPLE_RATE_F: f32 = SAMPLE_RATE as f32;

// ---------------------------------------------------------------------------
// Config structs
// ---------------------------------------------------------------------------

pub struct RecordConfig {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub channel: usize,
}

pub struct PlayConfig {
    pub input: PathBuf,
    pub output: PathBuf,
    pub output_secs: Option<f64>,
    pub channel: usize,
}

// ---------------------------------------------------------------------------
// WAV helpers (private)
// ---------------------------------------------------------------------------

fn read_wav_stereo_frames(path: &Path) -> Result<(Vec<(f32, f32)>, u32), String> {
    let reader = hound::WavReader::open(path)
        .map_err(|e| format!("failed to open {}: {e}", path.display()))?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels as usize;

    let raw_samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max = (1u32 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("read error: {e}"))?
        }
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("read error: {e}"))?,
    };

    let frames: Vec<(f32, f32)> = match channels {
        1 => raw_samples.iter().map(|&s| (s, s)).collect(),
        _ => raw_samples
            .chunks(channels)
            .map(|ch| (ch[0], if ch.len() > 1 { ch[1] } else { ch[0] }))
            .collect(),
    };

    Ok((frames, sample_rate))
}

fn read_wav_as_i16_buffer(path: &Path) -> Result<(Vec<i16>, u32), String> {
    let (frames, sr) = read_wav_stereo_frames(path)?;
    let samples: Vec<i16> = frames
        .iter()
        .map(|(l, r)| {
            let mono = (l + r) * 0.5;
            // 12-bit quantization matching RamRecord
            (mono * 2048.0).round().clamp(-2048.0, 2047.0) as i16
        })
        .collect();
    Ok((samples, sr))
}

fn write_wav_i16(path: &Path, samples: &[i16], sample_rate: u32) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|e| format!("create error: {e}"))?;
    for &s in samples {
        writer
            .write_sample(s)
            .map_err(|e| format!("write error: {e}"))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("finalize error: {e}"))?;
    Ok(())
}

fn write_wav_f32(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|e| format!("create error: {e}"))?;
    for &s in samples {
        writer
            .write_sample(s)
            .map_err(|e| format!("write error: {e}"))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("finalize error: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Offline record
// ---------------------------------------------------------------------------

pub fn run_offline_record(config: &RecordConfig) -> Result<(), String> {
    let (frames, sr) = read_wav_stereo_frames(&config.input)?;
    if sr != SAMPLE_RATE {
        return Err(format!(
            "sample rate mismatch: input is {sr} Hz, expected {SAMPLE_RATE} Hz"
        ));
    }

    register_plugin_instance();

    let mut rec = RamRecord::new(SAMPLE_RATE_F);
    let params = RamRecordParams {
        mlev: 127,
        mbal: 64,
        ilev: 127,
        ibal: 64,
        rec_len: 127,  // INF
        rec_rate: 127, // HI (1:1)
    };

    rec.start_recording(&params, true, config.channel);

    for (l, r) in &frames {
        rec.record_sample(*l, *r, *l, *r, &params, config.channel);
    }

    rec.stop_recording(config.channel);

    if let Some(output) = &config.output {
        let buffer = get_global_registry()
            .read_buffer(config.channel)
            .ok_or("failed to read buffer")?;
        // Scale 12-bit buffer values (-2048..2047) to 16-bit range so the WAV
        // represents correct audio levels and round-trips through read_wav_as_i16_buffer.
        let scaled: Vec<i16> = buffer.iter().map(|&s| s.saturating_mul(16)).collect();
        write_wav_i16(output, &scaled, SAMPLE_RATE)?;
    }

    unregister_plugin_instance();
    Ok(())
}

// ---------------------------------------------------------------------------
// Offline play
// ---------------------------------------------------------------------------

pub fn run_offline_play(config: &PlayConfig) -> Result<(), String> {
    let (samples, sr) = read_wav_as_i16_buffer(&config.input)?;
    if sr != SAMPLE_RATE {
        return Err(format!(
            "sample rate mismatch: input is {sr} Hz, expected {SAMPLE_RATE} Hz"
        ));
    }

    let input_duration_secs = samples.len() as f64 / SAMPLE_RATE as f64;
    let output_secs = config.output_secs.unwrap_or(input_duration_secs + 1.0);
    let output_samples = (output_secs * SAMPLE_RATE as f64) as usize;

    register_plugin_instance();

    get_global_registry().replace_buffer(config.channel, samples);

    let mut player = RamPlay::new(SAMPLE_RATE_F);
    let params = RamPlayParams {
        strt: 0,
        end: 127,
        pitch: 64,
        hold: 0,
        dec: 0,
        rtrg: 0,
        rtim: 0,
        srr: 0,
        vol: 127,
    };

    player.trigger(&params, config.channel);

    let mut output = Vec::with_capacity(output_samples);
    for _ in 0..output_samples {
        output.push(player.process(config.channel));
    }

    write_wav_f32(&config.output, &output, SAMPLE_RATE)?;

    unregister_plugin_instance();
    Ok(())
}

// ---------------------------------------------------------------------------
// CLI arg parsing
// ---------------------------------------------------------------------------

fn get_arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn parse_channel(args: &[String]) -> Result<usize, String> {
    match get_arg(args, "--channel") {
        Some(s) => {
            let ch: usize = s.parse().map_err(|_| format!("invalid channel: {s}"))?;
            if ch > 3 {
                return Err(format!("channel must be 0-3, got {ch}"));
            }
            Ok(ch)
        }
        None => Ok(0),
    }
}

pub fn parse_record_args(args: &[String]) -> Option<RecordConfig> {
    if !has_flag(args, "--input") {
        return None;
    }
    let input = get_arg(args, "--input").expect("--input requires a path");
    let output = get_arg(args, "--output").map(PathBuf::from);
    let channel = parse_channel(args).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });
    Some(RecordConfig {
        input: PathBuf::from(input),
        output,
        channel,
    })
}

pub fn parse_play_args(args: &[String]) -> Option<PlayConfig> {
    if !has_flag(args, "--input") || !has_flag(args, "--output") {
        return None;
    }
    let input = get_arg(args, "--input").expect("--input requires a path");
    let output = get_arg(args, "--output").expect("--output requires a path");
    let output_secs = get_arg(args, "--output-secs").map(|s| {
        s.parse::<f64>().unwrap_or_else(|_| {
            eprintln!("error: invalid --output-secs: {s}");
            std::process::exit(1);
        })
    });
    let channel = parse_channel(args).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });
    Some(PlayConfig {
        input: PathBuf::from(input),
        output: PathBuf::from(output),
        output_secs,
        channel,
    })
}
