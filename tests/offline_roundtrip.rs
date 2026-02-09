use std::sync::Arc;
use ultrawave::machines::ram_play::{RamPlay, RamPlayParams};
use ultrawave::machines::ram_record::{RamRecord, RamRecordParams};
use ultrawave::machines::shared::{get_global_registry, BufferRegistry};
use ultrawave::offline::{run_offline_play, run_offline_record, PlayConfig, RecordConfig};

const SR: f32 = 44100.0;

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
fn record_then_play_roundtrip() {
    let registry = Arc::new(BufferRegistry::new());
    let mut rec = RamRecord::with_registry(SR, registry.clone());
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

    rec.start_recording(&rec_params, true, 0);
    for (l, r) in &sine {
        rec.record_sample(*l, *r, *l, *r, &rec_params, 0);
    }
    rec.stop_recording(0);

    assert!(registry.buffer_len(0) > 0);

    let mut player = RamPlay::with_registry(SR, registry.clone());
    let play_params = RamPlayParams {
        strt: 0,
        end: 127,
        pitch: 64,
        vol: 127,
        ..Default::default()
    };

    player.trigger(&play_params, 0);

    // Collect audio from playback
    let buffer_len = registry.buffer_len(0);
    let render_len = buffer_len + 4410; // buffer + 0.1s tail

    let mut has_nonzero = false;
    let mut has_silence_after = false;

    for i in 0..render_len {
        let sample = player.process(0);
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
fn channel_isolation() {
    let registry = Arc::new(BufferRegistry::new());
    let mut rec = RamRecord::with_registry(SR, registry.clone());
    let params = RamRecordParams {
        mlev: 127,
        mbal: 64,
        ilev: 127,
        ibal: 64,
        rec_len: 127,
        rec_rate: 127,
        tempo_bpm: 120.0,
    };

    let sine = generate_sine(440.0, 0.05, SR);

    rec.start_recording(&params, true, 2);
    for (l, r) in &sine {
        rec.record_sample(*l, *r, *l, *r, &params, 2);
    }
    rec.stop_recording(2);

    assert!(registry.buffer_len(2) > 0, "channel 2 should have data");
    for ch in [0, 1, 3] {
        assert_eq!(registry.buffer_len(ch), 0, "channel {ch} should be empty");
    }
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

    // Clear global registry before using it
    get_global_registry().clear_all();

    // Record
    run_offline_record(&RecordConfig {
        input: input_wav.clone(),
        output: Some(buffer_wav.clone()),
        channel: 0,
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
        channel: 0,
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

    // Clear global registry before using it
    get_global_registry().clear_all();

    // Step 1: Record vinyl909.wav into the buffer, dump to record_output.wav
    run_offline_record(&RecordConfig {
        input: input_wav,
        output: Some(record_output.clone()),
        channel: 0,
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
        channel: 0,
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

    // The tail (after buffer ends + some margin) should be silent
    let tail_start = record_samples.len() + 2205; // buffer length + 50ms margin
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
