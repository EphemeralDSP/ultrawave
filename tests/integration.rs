#[path = "support/wet_reference.rs"]
mod wet_reference;

#[test]
fn offline_duration_covers_finite_trains_and_requires_a_bound_for_infinite_trains() {
    use std::process::Command;
    let dir = std::env::temp_dir().join(format!("ultrawave-duration-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("pulse.wav");
    let output = dir.join("render.wav");
    let mut writer = hound::WavWriter::create(
        &input,
        hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    writer.write_sample(8192i16).unwrap();
    writer.finalize().unwrap();
    let render = |options: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_ultrawave"))
            .arg("play")
            .arg("--input")
            .arg(&input)
            .arg("--output")
            .arg(&output)
            .args(options)
            .output()
            .unwrap()
    };
    // RTIM 127 is a sixteenth note. Raw 96 means 104 hits, not 96.
    for (raw, tempo, total, interval, explicit) in [
        ("12", "120", 12, 5512.5, false),
        ("96", "300", 104, 2205.0, false),
        ("127", "120", 8, 5512.5, true),
    ] {
        let mut options = vec!["--retrigger", raw, "--retrig-time", "127", "--tempo", tempo];
        if explicit {
            options.extend(["--output-secs", "1"]);
        }
        let result = render(&options);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let samples: Vec<_> = hound::WavReader::open(&output)
            .unwrap()
            .into_samples::<f32>()
            .map(Result::unwrap)
            .collect();
        let starts: Vec<_> = (0..total)
            .map(|hit| (hit as f64 * interval).round() as usize)
            .collect();
        let last = *starts.last().unwrap();
        assert!(
            samples.len() > last,
            "raw {raw}: render cut off the last hit at {last}, length {}",
            samples.len()
        );
        if explicit {
            assert_eq!(samples.len(), 44100, "explicit bounds remain exact");
        } else {
            let expected_frames = last + 1 + 44100; // Final one-frame hit plus the existing one-second tail.
            assert!((samples.len() as isize - expected_frames as isize).abs() <= 1);
        }
        let mut expected = vec![0.0; samples.len()];
        for start in starts {
            expected[start] = 0.25;
        }
        for (actual, expected) in samples
            .into_iter()
            .zip(wet_reference::wet_reference(expected))
        {
            assert!((actual - expected).abs() < 2e-6);
        }
    }
    let previous = std::fs::read(&output).unwrap();
    let result = render(&["--retrigger", "127"]);
    assert!(
        !result.status.success(),
        "infinite repeats need an explicit render duration"
    );
    assert!(String::from_utf8_lossy(&result.stderr).contains("--output-secs"));
    assert_eq!(
        std::fs::read(&output).unwrap(),
        previous,
        "invalid render must not overwrite output"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn offline_default_duration_uses_retained_ram_not_the_discarded_file_tail() {
    use ultrawave::machines::ram_play::RamPlayParams;
    use ultrawave::offline::{run_offline_play, PlayConfig};
    let dir =
        std::env::temp_dir().join(format!("ultrawave-retained-length-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("long.wav");
    let output = dir.join("render.wav");
    let mut writer = hound::WavWriter::create(
        &input,
        hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    // Eleven seconds in the file, but only ten fit in RAM at this rate.
    for _ in 0..485100 {
        writer.write_sample(8192i16).unwrap();
    }
    writer.finalize().unwrap();
    run_offline_play(&PlayConfig {
        input,
        output: output.clone(),
        output_secs: None,
        tempo_bpm: 120.0,
        params: RamPlayParams {
            vol: 127,
            ..Default::default()
        },
    })
    .unwrap();
    let reader = hound::WavReader::open(&output).unwrap();
    assert_eq!(
        reader.duration(),
        485100,
        "ten-second take plus one-second tail"
    );
    drop(reader);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn offline_binaries_preserve_wav_levels_and_render_distinct_character() {
    use std::process::Command;
    let dir = std::env::temp_dir().join(format!("ultrawave-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("input.wav");
    let recorded = dir.join("recorded.wav");
    let output = dir.join("output.wav");
    let mut writer = hound::WavWriter::create(
        &input,
        hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    let input_pcm = [4096i16, 12288, -4096, -12288, 6144];
    for value in input_pcm {
        writer.write_sample(value).unwrap();
    }
    writer.finalize().unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_ultrawave"))
        .arg("record")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&recorded)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let pcm: Vec<_> = hound::WavReader::open(&recorded)
        .unwrap()
        .into_samples::<i16>()
        .map(Result::unwrap)
        .collect();
    assert_eq!(pcm, input_pcm, "recording must not double the input");

    for (options, expected) in [
        (vec![], vec![0.125, 0.375, -0.125, -0.375, 0.1875]),
        (vec!["--brr", "127"], vec![0.0, 0.5, 0.0, -0.5, 0.0]),
        (
            vec!["--srr", "0.5"],
            vec![0.0, 0.375, 0.375, -0.375, -0.375],
        ),
        (vec!["--pitch", "76"], vec![0.125, -0.125, 0.1875, 0.0, 0.0]),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_ultrawave"))
            .arg("play")
            .arg("--input")
            .arg(&recorded)
            .arg("--output")
            .arg(&output)
            .args(["--output-secs", "0.001"])
            .args(options)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let rendered: Vec<_> = hound::WavReader::open(&output)
            .unwrap()
            .into_samples::<f32>()
            .map(Result::unwrap)
            .collect();
        let expected = wet_reference::wet_reference(
            expected
                .into_iter()
                .chain(std::iter::repeat(0.0))
                .take(rendered.len()),
        );
        for (frame, (actual, expected)) in rendered.iter().zip(expected).enumerate() {
            assert!(
                (actual - expected).abs() < 2e-6,
                "character frame {frame}: {actual} != {expected}"
            );
        }
    }
    for (tempo, starts) in [("60", [0, 11025]), ("120", [0, 5513])] {
        let result = Command::new(env!("CARGO_BIN_EXE_ultrawave"))
            .arg("play")
            .arg("--input")
            .arg(&recorded)
            .arg("--output")
            .arg(&output)
            .args([
                "--tempo",
                tempo,
                "--retrigger",
                "2",
                "--retrig-time",
                "127",
                "--output-secs",
                "1",
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let rendered: Vec<_> = hound::WavReader::open(&output)
            .unwrap()
            .into_samples::<f32>()
            .map(Result::unwrap)
            .collect();
        let mut expected = vec![0.0; 44100];
        for start in starts {
            expected[start..start + 5].copy_from_slice(&[0.125, 0.375, -0.125, -0.375, 0.1875]);
        }
        let expected = wet_reference::wet_reference(expected);
        assert_eq!(rendered.len(), expected.len());
        for (frame, (actual, expected)) in rendered.iter().zip(expected).enumerate() {
            assert!(
                (actual - expected).abs() < 2e-6,
                "tempo={tempo}, frame={frame}: {actual} != {expected}"
            );
        }
    }
    for options in [
        vec!["--brr", "128"],
        vec!["--srr", "NaN"],
        vec!["--output-secs", "inf"],
        vec!["--tempo", "0"],
        vec!["--tempo", "NaN"],
        vec!["--tempo", "inf"],
        vec!["--brr"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_ultrawave"))
            .arg("play")
            .arg("--input")
            .arg(&recorded)
            .arg("--output")
            .arg(&output)
            .args(options)
            .output()
            .unwrap();
        assert!(!result.status.success());
    }
    std::fs::remove_dir_all(dir).unwrap();
}
