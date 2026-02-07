use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Configuration for the render test runner.
pub struct RenderTestConfig {
    /// Directory containing .rpp files and their matching reference .wav files.
    pub test_dir: PathBuf,
    /// Path or name of the Reaper executable.
    pub reaper_bin: String,
    /// Maximum allowed RMS difference between rendered and expected audio (0.0–1.0).
    pub threshold_rms: f64,
    /// Maximum allowed peak sample difference (0.0–1.0).
    pub threshold_peak: f64,
    /// Print detailed per-file comparison info.
    pub verbose: bool,
}

struct TestResult {
    name: String,
    passed: bool,
    rms_diff: f64,
    peak_diff: f64,
    detail: String,
}

/// Main entry point for the render-test command.
pub fn run(config: RenderTestConfig) -> ExitCode {
    if !config.test_dir.is_dir() {
        eprintln!(
            "Test directory does not exist: {}",
            config.test_dir.display()
        );
        eprintln!("Create it and add .rpp / .wav pairs to get started.");
        return ExitCode::FAILURE;
    }

    // Collect all .rpp files
    let rpp_files = match collect_rpp_files(&config.test_dir) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("Failed to scan test directory: {}", e);
            return ExitCode::FAILURE;
        }
    };

    if rpp_files.is_empty() {
        println!("No .rpp files found in {}", config.test_dir.display());
        println!("Add Reaper project files with matching reference .wav files to run render tests.");
        return ExitCode::SUCCESS;
    }

    println!();
    println!("=========================================");
    println!("  Reaper Render Regression Tests");
    println!("=========================================");
    println!("Test directory: {}", config.test_dir.display());
    println!("Reaper binary:  {}", config.reaper_bin);
    println!("Threshold RMS:  {:.6}", config.threshold_rms);
    println!("Threshold Peak: {:.6}", config.threshold_peak);
    println!("Projects found: {}", rpp_files.len());
    println!();

    let mut results: Vec<TestResult> = Vec::new();

    for rpp_path in &rpp_files {
        let stem = rpp_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        println!("--- {} ---", stem);

        // 1. Find expected reference wav: <stem>.wav next to the .rpp
        let expected_wav = rpp_path.with_extension("wav");
        if !expected_wav.is_file() {
            let result = TestResult {
                name: stem.clone(),
                passed: false,
                rms_diff: f64::NAN,
                peak_diff: f64::NAN,
                detail: format!(
                    "Missing reference file: {}",
                    expected_wav.display()
                ),
            };
            println!("  SKIP: {}", result.detail);
            results.push(result);
            continue;
        }

        // 2. Determine where Reaper will write the rendered output
        let render_output = match find_render_output_path(rpp_path) {
            Some(p) => p,
            None => {
                let result = TestResult {
                    name: stem.clone(),
                    passed: false,
                    rms_diff: f64::NAN,
                    peak_diff: f64::NAN,
                    detail: "Could not determine render output path from RPP. \
                             Ensure RENDER_FILE is set in the project."
                        .to_string(),
                };
                println!("  SKIP: {}", result.detail);
                results.push(result);
                continue;
            }
        };

        if config.verbose {
            println!("  Render output: {}", render_output.display());
            println!("  Reference:     {}", expected_wav.display());
        }

        // 3. Remove stale render output if it exists
        if render_output.is_file() {
            if let Err(e) = std::fs::remove_file(&render_output) {
                eprintln!(
                    "  WARNING: Could not remove stale output {}: {}",
                    render_output.display(),
                    e
                );
            }
        }

        // 4. Render the project
        println!("  Rendering...");
        if let Err(e) = render_project(&config.reaper_bin, rpp_path) {
            let result = TestResult {
                name: stem.clone(),
                passed: false,
                rms_diff: f64::NAN,
                peak_diff: f64::NAN,
                detail: format!("Render failed: {}", e),
            };
            println!("  FAIL: {}", result.detail);
            results.push(result);
            continue;
        }

        // 5. Verify render output was created
        if !render_output.is_file() {
            let result = TestResult {
                name: stem.clone(),
                passed: false,
                rms_diff: f64::NAN,
                peak_diff: f64::NAN,
                detail: format!(
                    "Render completed but output not found at: {}",
                    render_output.display()
                ),
            };
            println!("  FAIL: {}", result.detail);
            results.push(result);
            continue;
        }

        // 6. Compare rendered output with reference
        let result = compare_wav_files(&stem, &render_output, &expected_wav, &config);
        if result.passed {
            println!("  PASS (rms={:.6}, peak={:.6})", result.rms_diff, result.peak_diff);
        } else {
            println!("  FAIL: {}", result.detail);
        }

        // 7. Clean up rendered output
        if render_output.is_file() {
            let _ = std::fs::remove_file(&render_output);
        }

        results.push(result);
    }

    // Summary
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.iter().filter(|r| !r.passed).count();
    let total = results.len();

    println!();
    println!("=========================================");
    println!("  Results: {} passed, {} failed, {} total", passed, failed, total);
    println!("=========================================");

    if config.verbose || failed > 0 {
        for r in &results {
            let status = if r.passed { "PASS" } else { "FAIL" };
            if r.passed {
                println!(
                    "  [{}] {} (rms={:.6}, peak={:.6})",
                    status, r.name, r.rms_diff, r.peak_diff
                );
            } else {
                println!("  [{}] {} - {}", status, r.name, r.detail);
            }
        }
    }

    println!();

    if failed > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Collect all .rpp files from the given directory (non-recursive).
fn collect_rpp_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read_dir: {}", e))?;

    let mut rpp_files: Vec<PathBuf> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rpp") {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    rpp_files.sort();
    Ok(rpp_files)
}

/// Parse a Reaper .rpp file to determine where it will render output.
///
/// Looks for RENDER_FILE (output directory/path) and RENDER_PATTERN (filename pattern).
/// Returns the fully resolved path to the expected rendered WAV file.
fn find_render_output_path(rpp_path: &Path) -> Option<PathBuf> {
    let content = std::fs::read_to_string(rpp_path).ok()?;
    let rpp_dir = rpp_path.parent()?;

    let mut render_file: Option<String> = None;
    let mut render_pattern: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("RENDER_FILE") {
            // Format: RENDER_FILE "path/to/output"
            // or:     RENDER_FILE "/absolute/path"
            render_file = extract_quoted_value(trimmed);
        }

        if trimmed.starts_with("RENDER_PATTERN") {
            // Format: RENDER_PATTERN "filename_pattern"
            render_pattern = extract_quoted_value(trimmed);
        }
    }

    // Determine the output directory
    let render_file_str = render_file.unwrap_or_default();
    let output_dir = if render_file_str.is_empty() {
        // If RENDER_FILE is empty, output goes to the project's directory
        rpp_dir.to_path_buf()
    } else {
        let rf_path = PathBuf::from(&render_file_str);
        if rf_path.is_absolute() {
            rf_path
        } else {
            rpp_dir.join(rf_path)
        }
    };

    // Determine the filename
    let pattern = render_pattern.unwrap_or_default();
    let filename = if pattern.is_empty() {
        // Default to the project name
        let stem = rpp_path.file_stem()?.to_string_lossy().to_string();
        format!("{}.wav", stem)
    } else {
        // Reaper's RENDER_PATTERN can contain wildcards like $project, $track, etc.
        // For our purposes, do basic substitution of the most common ones.
        let stem = rpp_path.file_stem()?.to_string_lossy().to_string();
        let resolved = pattern
            .replace("$project", &stem)
            .replace("$Project", &stem);
        // If the pattern doesn't have an extension, append .wav
        if PathBuf::from(&resolved).extension().is_none() {
            format!("{}.wav", resolved)
        } else {
            resolved
        }
    };

    // If output_dir looks like a full file path (has a wav/aiff/mp3 extension),
    // treat it as the complete output path rather than a directory.
    if let Some(ext) = output_dir.extension().and_then(|e| e.to_str()) {
        let audio_extensions = ["wav", "aiff", "aif", "mp3", "ogg", "flac", "w64"];
        if audio_extensions.iter().any(|&ae| ext.eq_ignore_ascii_case(ae)) {
            return Some(output_dir);
        }
    }

    Some(output_dir.join(filename))
}

/// Extract a quoted string value from a Reaper config line.
/// e.g. `RENDER_FILE "some/path"` -> Some("some/path")
fn extract_quoted_value(line: &str) -> Option<String> {
    let first_quote = line.find('"')?;
    let rest = &line[first_quote + 1..];
    let end_quote = rest.find('"')?;
    Some(rest[..end_quote].to_string())
}

/// Invoke Reaper in headless render mode.
fn render_project(reaper_bin: &str, rpp_path: &Path) -> Result<(), String> {
    let rpp_abs = rpp_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve RPP path: {}", e))?;

    let status = Command::new(reaper_bin)
        .arg("-renderproject")
        .arg(&rpp_abs)
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!(
                    "Reaper not found at '{}'. \
                     Install Reaper or pass --reaper /path/to/reaper",
                    reaper_bin
                )
            } else {
                format!("Failed to launch Reaper: {}", e)
            }
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Reaper exited with status: {}",
            status.code().unwrap_or(-1)
        ))
    }
}

/// Compare two WAV files sample-by-sample.
fn compare_wav_files(
    name: &str,
    actual_path: &Path,
    expected_path: &Path,
    config: &RenderTestConfig,
) -> TestResult {
    let actual_reader = match hound::WavReader::open(actual_path) {
        Ok(r) => r,
        Err(e) => {
            return TestResult {
                name: name.to_string(),
                passed: false,
                rms_diff: f64::NAN,
                peak_diff: f64::NAN,
                detail: format!("Failed to open rendered WAV: {}", e),
            };
        }
    };

    let expected_reader = match hound::WavReader::open(expected_path) {
        Ok(r) => r,
        Err(e) => {
            return TestResult {
                name: name.to_string(),
                passed: false,
                rms_diff: f64::NAN,
                peak_diff: f64::NAN,
                detail: format!("Failed to open reference WAV: {}", e),
            };
        }
    };

    let actual_spec = actual_reader.spec();
    let expected_spec = expected_reader.spec();

    // Check format compatibility
    if actual_spec.channels != expected_spec.channels {
        return TestResult {
            name: name.to_string(),
            passed: false,
            rms_diff: f64::NAN,
            peak_diff: f64::NAN,
            detail: format!(
                "Channel mismatch: rendered={}, expected={}",
                actual_spec.channels, expected_spec.channels
            ),
        };
    }

    if actual_spec.sample_rate != expected_spec.sample_rate {
        return TestResult {
            name: name.to_string(),
            passed: false,
            rms_diff: f64::NAN,
            peak_diff: f64::NAN,
            detail: format!(
                "Sample rate mismatch: rendered={}, expected={}",
                actual_spec.sample_rate, expected_spec.sample_rate
            ),
        };
    }

    // Read all samples as f32
    let actual_samples = match read_samples_as_f32(actual_reader) {
        Ok(s) => s,
        Err(e) => {
            return TestResult {
                name: name.to_string(),
                passed: false,
                rms_diff: f64::NAN,
                peak_diff: f64::NAN,
                detail: format!("Failed to read rendered samples: {}", e),
            };
        }
    };

    let expected_samples = match read_samples_as_f32(expected_reader) {
        Ok(s) => s,
        Err(e) => {
            return TestResult {
                name: name.to_string(),
                passed: false,
                rms_diff: f64::NAN,
                peak_diff: f64::NAN,
                detail: format!("Failed to read reference samples: {}", e),
            };
        }
    };

    // Use the longer length, treating missing samples as silence
    let len = actual_samples.len().max(expected_samples.len());

    if len == 0 {
        return TestResult {
            name: name.to_string(),
            passed: true,
            rms_diff: 0.0,
            peak_diff: 0.0,
            detail: "Both files are empty".to_string(),
        };
    }

    // Length difference warning
    let length_diff = (actual_samples.len() as i64 - expected_samples.len() as i64).unsigned_abs();
    let length_diff_pct = (length_diff as f64 / len as f64) * 100.0;

    // Compute differences
    let mut sum_sq_diff: f64 = 0.0;
    let mut peak_diff: f64 = 0.0;

    for i in 0..len {
        let a = if i < actual_samples.len() {
            actual_samples[i] as f64
        } else {
            0.0
        };
        let e = if i < expected_samples.len() {
            expected_samples[i] as f64
        } else {
            0.0
        };

        let diff = (a - e).abs();
        sum_sq_diff += diff * diff;
        if diff > peak_diff {
            peak_diff = diff;
        }
    }

    let rms_diff = (sum_sq_diff / len as f64).sqrt();

    let mut detail_parts: Vec<String> = Vec::new();

    if length_diff > 0 {
        detail_parts.push(format!(
            "Length difference: {} samples ({:.1}%) [rendered={}, expected={}]",
            length_diff,
            length_diff_pct,
            actual_samples.len(),
            expected_samples.len()
        ));
    }

    let rms_ok = rms_diff <= config.threshold_rms;
    let peak_ok = peak_diff <= config.threshold_peak;
    let passed = rms_ok && peak_ok;

    if !rms_ok {
        detail_parts.push(format!(
            "RMS difference {:.6} exceeds threshold {:.6}",
            rms_diff, config.threshold_rms
        ));
    }
    if !peak_ok {
        detail_parts.push(format!(
            "Peak difference {:.6} exceeds threshold {:.6}",
            peak_diff, config.threshold_peak
        ));
    }

    if config.verbose {
        detail_parts.push(format!(
            "Samples: rendered={}, expected={}, compared={}",
            actual_samples.len(),
            expected_samples.len(),
            len
        ));
        detail_parts.push(format!(
            "Format: {}ch {}Hz {}bit",
            actual_spec.channels, actual_spec.sample_rate, actual_spec.bits_per_sample
        ));
    }

    let detail = if detail_parts.is_empty() {
        "OK".to_string()
    } else {
        detail_parts.join("; ")
    };

    TestResult {
        name: name.to_string(),
        passed,
        rms_diff,
        peak_diff,
        detail,
    }
}

/// Read all samples from a WAV file, normalizing to f32 in [-1.0, 1.0].
fn read_samples_as_f32(mut reader: hound::WavReader<std::io::BufReader<std::fs::File>>) -> Result<Vec<f32>, String> {
    let spec = reader.spec();

    match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<f32>, _>>()
            .map_err(|e| format!("Error reading float samples: {}", e)),

        hound::SampleFormat::Int => {
            let max_val = (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_val))
                .collect::<Result<Vec<f32>, _>>()
                .map_err(|e| format!("Error reading int samples: {}", e))
        }
    }
}

/// Parse command-line arguments and build the config.
pub fn parse_args(args: &[String]) -> RenderTestConfig {
    let test_dir = args
        .iter()
        .position(|a| a == "--dir")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("tests/reaper_renders"));

    let reaper_bin = args
        .iter()
        .position(|a| a == "--reaper")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(default_reaper_bin);

    let threshold_rms = args
        .iter()
        .position(|a| a == "--threshold-rms")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.001);

    let threshold_peak = args
        .iter()
        .position(|a| a == "--threshold-peak")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.01);

    let verbose = args.iter().any(|a| a == "--verbose");

    RenderTestConfig {
        test_dir,
        reaper_bin,
        threshold_rms,
        threshold_peak,
        verbose,
    }
}

/// Pick a sensible default Reaper binary path per platform.
fn default_reaper_bin() -> String {
    if cfg!(target_os = "macos") {
        // Standard macOS install location
        let app_path = "/Applications/REAPER.app/Contents/MacOS/REAPER";
        if PathBuf::from(app_path).is_file() {
            return app_path.to_string();
        }
    }

    if cfg!(target_os = "windows") {
        // Common Windows paths
        for path in &[
            r"C:\Program Files\REAPER (x64)\reaper.exe",
            r"C:\Program Files\REAPER\reaper.exe",
        ] {
            if PathBuf::from(path).is_file() {
                return path.to_string();
            }
        }
    }

    // Fallback: assume it's in PATH
    "reaper".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_quoted_value() {
        assert_eq!(
            extract_quoted_value(r#"RENDER_FILE "output""#),
            Some("output".to_string())
        );
        assert_eq!(
            extract_quoted_value(r#"RENDER_FILE "/absolute/path/out.wav""#),
            Some("/absolute/path/out.wav".to_string())
        );
        assert_eq!(
            extract_quoted_value(r#"RENDER_PATTERN "myrender""#),
            Some("myrender".to_string())
        );
        assert_eq!(extract_quoted_value("RENDER_FILE"), None);
        assert_eq!(
            extract_quoted_value(r#"RENDER_FILE """#),
            Some("".to_string())
        );
    }

    #[test]
    fn test_default_args() {
        let config = parse_args(&[]);
        assert_eq!(config.test_dir, PathBuf::from("tests/reaper_renders"));
        assert!((config.threshold_rms - 0.001).abs() < f64::EPSILON);
        assert!((config.threshold_peak - 0.01).abs() < f64::EPSILON);
        assert!(!config.verbose);
    }

    #[test]
    fn test_custom_args() {
        let args: Vec<String> = vec![
            "--dir",
            "my/tests",
            "--reaper",
            "/usr/bin/reaper",
            "--threshold-rms",
            "0.005",
            "--threshold-peak",
            "0.05",
            "--verbose",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let config = parse_args(&args);
        assert_eq!(config.test_dir, PathBuf::from("my/tests"));
        assert_eq!(config.reaper_bin, "/usr/bin/reaper");
        assert!((config.threshold_rms - 0.005).abs() < f64::EPSILON);
        assert!((config.threshold_peak - 0.05).abs() < f64::EPSILON);
        assert!(config.verbose);
    }
}
