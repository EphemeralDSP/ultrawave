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

    let result = Command::new(env!("CARGO_BIN_EXE_ultrawave_record"))
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
        let result = Command::new(env!("CARGO_BIN_EXE_ultrawave_play"))
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
        assert_eq!(&rendered[..5], expected);
        assert!(rendered[5..].iter().all(|s| *s == 0.0));
    }
    for options in [
        vec!["--brr", "128"],
        vec!["--srr", "NaN"],
        vec!["--output-secs", "inf"],
        vec!["--brr"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_ultrawave_play"))
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
