use std::sync::Arc;
use ultrawave::machines::memory::SampleMemory;
use ultrawave::machines::ram_play::{RamPlay, RamPlayParams};
use ultrawave::machines::ram_record::{RamRecord, RamRecordParams};
use ultrawave::offline::{
    parse_play_args, parse_record_args, run_offline_play, run_offline_record, PlayConfig,
    RecordConfig,
};

#[path = "support/wet_reference.rs"]
mod wet_reference;

const SR: f32 = 44100.0;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn unified_cli_subcommands_select_exactly_one_offline_action() {
    let record = args(&["ultrawave", "record", "--input", "in.wav"]);
    assert!(parse_record_args(&record).is_some());
    assert!(parse_play_args(&record).is_none());

    let play = args(&[
        "ultrawave",
        "play",
        "--input",
        "in.wav",
        "--output",
        "out.wav",
    ]);
    assert!(parse_record_args(&play).is_none());
    assert!(parse_play_args(&play).is_some());

    let ambiguous = args(&["ultrawave", "--input", "in.wav", "--output", "out.wav"]);
    assert!(parse_record_args(&ambiguous).is_none());
    assert!(parse_play_args(&ambiguous).is_none());
}

#[test]
fn independent_recorders_do_not_cancel_each_other() {
    let params = RamRecordParams {
        mlev: 127,
        ilev: 0,
        ..Default::default()
    };
    let first_registry = Arc::new(SampleMemory::new());
    let second_registry = Arc::new(SampleMemory::new());
    let mut first = RamRecord::new(SR, first_registry.clone());
    let mut second = RamRecord::new(SR, second_registry.clone());
    first.start_recording(&params);
    second.start_recording(&params);
    first.record_sample(0.25, 0.25, 0.0, 0.0, &params);
    second.record_sample(-0.5, -0.5, 0.0, 0.0, &params);
    assert_eq!(first_registry.read_buffer().unwrap(), [512]);
    assert_eq!(second_registry.read_buffer().unwrap(), [-1024]);
}

#[test]
fn signed_12bit_capture_preserves_level_and_every_sample() {
    let registry = Arc::new(SampleMemory::new());
    let mut recorder = RamRecord::new(SR, registry.clone());
    let params = RamRecordParams {
        mlev: 127,
        ilev: 0,
        rec_len: 127,
        ..Default::default()
    };
    // Includes clipping, both signs, sub-LSB values, and a nonzero last sample.
    let input = [-1.5, -0.5, -0.0003, 0.0001, 0.0003, 0.25, 1.5, 0.75];
    let expected_codes = [-2048, -1024, -1, 0, 1, 512, 2047, 1536];
    recorder.start_recording(&params);
    for sample in input {
        recorder.record_sample(sample, sample, 0.0, 0.0, &params);
    }
    recorder.stop_recording();
    assert_eq!(registry.read_buffer().unwrap(), expected_codes);

    let mut player = RamPlay::new(SR, registry);
    player.trigger(&RamPlayParams {
        vol: 127,
        ..Default::default()
    });
    for code in expected_codes {
        assert_eq!(player.process(), code as f32 / 2048.0);
    }
    assert!(!player.is_playing());
    assert_eq!(player.process(), 0.0);
}

fn generate_sine(freq_hz: f32, duration_secs: f32, sample_rate: f32) -> Vec<(f32, f32)> {
    let num_samples = (duration_secs * sample_rate) as usize;
    (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate;
            let s = (2.0 * std::f32::consts::PI * freq_hz * t).sin() * 0.8;
            (s, s)
        })
        .collect()
}

#[test]
fn offline_playback_removes_dc_and_matches_measured_low_frequency_loss() {
    let dir = std::env::temp_dir().join(format!("ultrawave_low_cut_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("input.wav");
    let output = dir.join("output.wav");
    for (frequency, expected_db) in [
        (0.0, 0.0),
        (30.0, -1.21),
        (100.0, -0.124),
        (1000.0, -0.0013),
    ] {
        let mut wav = hound::WavWriter::create(
            &input,
            hound::WavSpec {
                channels: 1,
                sample_rate: 44100,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        let mut reference_energy = 0.0;
        for frame in 0..44100 {
            let sample = if frequency == 0.0 {
                8192
            } else {
                // Already on the 12-bit grid so quantization is not confused
                // with the fixed playback filter's response.
                ((512.0 * (std::f64::consts::TAU * frequency * frame as f64 / 44100.0).sin())
                    .round() as i16)
                    * 16
            };
            wav.write_sample(sample).unwrap();
            if frame >= 22050 {
                reference_energy += (sample as f64 / 32768.0).powi(2);
            }
        }
        wav.finalize().unwrap();
        run_offline_play(&PlayConfig {
            input: input.clone(),
            output: output.clone(),
            output_secs: Some(1.2),
            tempo_bpm: 120.0,
            params: RamPlayParams {
                vol: 127,
                ..Default::default()
            },
        })
        .unwrap();
        let rendered: Vec<_> = hound::WavReader::open(&output)
            .unwrap()
            .into_samples::<f32>()
            .map(Result::unwrap)
            .collect();
        if frequency == 0.0 {
            assert!(
                rendered[0] > 0.24,
                "must pass the leading edge, not mute DC input"
            );
            assert!(
                rendered[22050..44100].iter().all(|s| s.abs() < 1e-6),
                "DC must decay during playback"
            );
            assert!(
                rendered[44100] < -0.24,
                "filter history must survive the end of a voice"
            );
            assert!(rendered.last().unwrap().abs() < 1e-6, "tail must decay");
        } else {
            let energy: f64 = rendered[22050..44100]
                .iter()
                .map(|s| (*s as f64).powi(2))
                .sum();
            let db = 10.0 * (energy / reference_energy).log10();
            assert!(
                (db - expected_db).abs() < 0.025,
                "{frequency} Hz: {db} dB, expected {expected_db}"
            );
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn record_then_play_roundtrip() {
    let registry = Arc::new(SampleMemory::new());
    let mut rec = RamRecord::new(SR, registry.clone());
    let rec_params = RamRecordParams {
        mlev: 127,
        mbal: 64,
        ilev: 127,
        ibal: 64,
        rec_len: 127,
        rec_rate: 127,
        tempo_bpm: 120.0,
    };

    let sine = generate_sine(1000.0, 0.1, SR);

    rec.start_recording(&rec_params);
    for (l, r) in &sine {
        rec.record_sample(*l, *r, *l, *r, &rec_params);
    }
    rec.stop_recording();

    assert!(registry.buffer_len() > 0);

    let mut player = RamPlay::new(SR, registry.clone());
    let play_params = RamPlayParams {
        strt: 0,
        end: 127,
        pitch: 64,
        vol: 127,
        ..Default::default()
    };

    player.trigger(&play_params);

    // Collect audio from playback
    let buffer_len = registry.buffer_len();
    let render_len = buffer_len + 4410; // buffer + 0.1s tail

    let mut has_nonzero = false;
    let mut has_silence_after = false;

    for i in 0..render_len {
        let sample = player.process();
        if i < buffer_len && sample.abs() > 1e-6 {
            has_nonzero = true;
        }
        if i > buffer_len + 100 && sample.abs() < 1e-6 {
            has_silence_after = true;
        }
    }

    assert!(has_nonzero, "playback should produce non-silent output");
    assert!(
        has_silence_after,
        "playback should go silent after buffer ends"
    );
}

#[test]
fn wav_file_roundtrip() {
    let dir = std::env::temp_dir().join("ultrawave_test_roundtrip");
    let _ = std::fs::create_dir_all(&dir);
    let input_wav = dir.join("input.wav");
    let buffer_wav = dir.join("buffer.wav");
    let output_wav = dir.join("output.wav");

    // Write a test WAV (mono 44100 Hz sine)
    {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&input_wav, spec).unwrap();
        let num_samples = 4410; // 0.1s
        for i in 0..num_samples {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.8;
            writer.write_sample((s * 32767.0) as i16).unwrap();
        }
        writer.finalize().unwrap();
    }

    // Record
    run_offline_record(&RecordConfig {
        input: input_wav.clone(),
        output: Some(buffer_wav.clone()),
    })
    .expect("offline record should succeed");

    // Verify the buffer WAV exists and has samples
    {
        let reader = hound::WavReader::open(&buffer_wav).unwrap();
        let samples: Vec<i16> = reader.into_samples::<i16>().map(|s| s.unwrap()).collect();
        assert!(!samples.is_empty(), "buffer WAV should have samples");
        assert!(
            samples.iter().any(|&s| s != 0),
            "buffer WAV should have non-zero samples"
        );
    }

    // Play
    run_offline_play(&PlayConfig {
        input: buffer_wav.clone(),
        output: output_wav.clone(),
        output_secs: Some(0.2),
        tempo_bpm: 120.0,
        params: RamPlayParams {
            vol: 127,
            ..Default::default()
        },
    })
    .expect("offline play should succeed");

    // Verify the output WAV exists and has audio
    {
        let reader = hound::WavReader::open(&output_wav).unwrap();
        let samples: Vec<f32> = reader.into_samples::<f32>().map(|s| s.unwrap()).collect();
        assert!(!samples.is_empty(), "output WAV should have samples");
        assert!(
            samples.iter().any(|&s| s.abs() > 1e-6),
            "output WAV should have non-silent audio"
        );
    }

    let reverse_wav = dir.join("reverse.wav");
    run_offline_play(&PlayConfig {
        input: buffer_wav.clone(),
        output: reverse_wav.clone(),
        output_secs: Some(0.2),
        tempo_bpm: 120.0,
        params: RamPlayParams {
            strt: 127,
            end: 0,
            vol: 127,
            ..Default::default()
        },
    })
    .unwrap();
    let recorded: Vec<_> = hound::WavReader::open(&buffer_wav)
        .unwrap()
        .into_samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();
    let reversed: Vec<_> = hound::WavReader::open(reverse_wav)
        .unwrap()
        .into_samples::<f32>()
        .map(Result::unwrap)
        .collect();
    // Reverse the stored sample, then apply the causal playback filter. Reversing
    // the already-filtered forward render would incorrectly reverse its phase.
    let expected = wet_reference::wet_reference(
        recorded
            .into_iter()
            .rev()
            .chain(std::iter::repeat(0.0))
            .take(reversed.len()),
    );
    for (frame, (actual, expected)) in reversed.iter().zip(expected).enumerate() {
        assert!(
            (actual - expected).abs() < 2e-6,
            "reverse frame {frame}: {actual} != {expected}"
        );
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn vinyl909_record_then_play() {
    let input_wav = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("vinyl909.wav");
    assert!(input_wav.exists(), "tests/vinyl909.wav must exist");

    let dir = std::env::temp_dir().join("ultrawave_test_vinyl909");
    let _ = std::fs::create_dir_all(&dir);
    let record_output = dir.join("record_output.wav");
    let play_output = dir.join("play_output.wav");

    // Step 1: Record vinyl909.wav into the buffer, dump to record_output.wav
    run_offline_record(&RecordConfig {
        input: input_wav,
        output: Some(record_output.clone()),
    })
    .expect("offline record of vinyl909.wav should succeed");

    // Verify record_output.wav
    let record_samples: Vec<i16> = {
        let reader = hound::WavReader::open(&record_output).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, 44100, "record output should be 44100 Hz");
        assert_eq!(spec.channels, 1, "record output should be mono");
        reader.into_samples::<i16>().map(|s| s.unwrap()).collect()
    };
    assert!(
        !record_samples.is_empty(),
        "record output should have samples"
    );
    assert!(
        record_samples.iter().any(|&s| s != 0),
        "record output should have non-zero samples"
    );
    // vinyl909.wav is ~10337 stereo frames; record should capture all of them
    assert!(
        record_samples.len() >= 10000,
        "record output should capture most of the input ({} samples)",
        record_samples.len()
    );

    // Step 2: Play record_output.wav back through the play engine
    let input_duration_secs = record_samples.len() as f64 / 44100.0;
    run_offline_play(&PlayConfig {
        input: record_output,
        output: play_output.clone(),
        output_secs: Some(input_duration_secs + 0.5),
        tempo_bpm: 120.0,
        params: RamPlayParams {
            vol: 127,
            ..Default::default()
        },
    })
    .expect("offline play of record_output.wav should succeed");

    // Verify play_output.wav
    let play_samples: Vec<f32> = {
        let reader = hound::WavReader::open(&play_output).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, 44100, "play output should be 44100 Hz");
        assert_eq!(spec.channels, 1, "play output should be mono");
        reader.into_samples::<f32>().map(|s| s.unwrap()).collect()
    };
    assert!(!play_samples.is_empty(), "play output should have samples");

    // The first portion (covering the buffer) should contain non-silent audio
    let active_region = &play_samples[..record_samples.len().min(play_samples.len())];
    let peak = active_region.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        peak > 0.01,
        "play output should have audible content (peak={peak})"
    );

    // The reported 250ms filter tail must have decayed to silence.
    let tail_start = record_samples.len() + 11025;
    if tail_start < play_samples.len() {
        let tail = &play_samples[tail_start..];
        let tail_peak = tail.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            tail_peak < 1e-6,
            "play output tail should be silent (tail_peak={tail_peak})"
        );
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}
