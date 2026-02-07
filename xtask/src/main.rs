use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "install" => install_plugins(),
            "pluginval" | "validate-vst3" => run_validate_vst3(&args[2..]),
            "validate-clap" => run_validate_clap(&args[2..]),
            "validate-all" => run_validate_all(&args[2..]),
            "help" | "--help" | "-h" => {
                print_help();
                ExitCode::SUCCESS
            }
            _ => {
                let _ = nih_plug_xtask::main();
                ExitCode::SUCCESS
            }
        }
    } else {
        let _ = nih_plug_xtask::main();
        ExitCode::SUCCESS
    }
}

fn print_help() {
    println!(
        r#"Ultrawave xtask commands

USAGE:
    cargo xtask <COMMAND> [OPTIONS]

COMMANDS:
    bundle              Bundle plugins (from nih_plug_xtask)
    install             Build + install both plugins to platform plugin directories

VALIDATION COMMANDS:
    validate-vst3       Validate VST3 plugin using pluginval
    validate-clap       Validate CLAP plugin using clap-validator
    validate-all        Run all validators (VST3 + CLAP)
    pluginval           Alias for validate-vst3

VALIDATION OPTIONS:
    --strictness <1-10>     pluginval strictness level (default: 5)
    --tests <pattern>       clap-validator test filter
    --skip-bundle           Skip bundling if plugin already exists
    --verbose               Show detailed validation output

EXAMPLES:
    cargo xtask bundle ultrawave-record --release
    cargo xtask bundle ultrawave-play --release
    cargo xtask validate-vst3 --strictness 5
    cargo xtask validate-clap
    cargo xtask validate-all
    cargo xtask install
    cargo xtask install --system
"#
    );
}

fn ensure_bundled(skip_bundle: bool) -> bool {
    let vst3_path = PathBuf::from("target/bundled/ultrawave.vst3");
    let clap_path = PathBuf::from("target/bundled/ultrawave.clap");

    if skip_bundle && vst3_path.exists() && clap_path.exists() {
        return true;
    }

    println!("Building and bundling plugins...");
    let status = Command::new("cargo")
        .args(["xtask", "bundle", "ultrawave", "--release"])
        .status()
        .expect("Failed to run cargo xtask bundle");

    if !status.success() {
        eprintln!("Bundle failed");
        return false;
    }
    true
}

fn run_validate_vst3(args: &[String]) -> ExitCode {
    let strictness = args
        .iter()
        .position(|a| a == "--strictness")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("5");

    let skip_bundle = args.iter().any(|a| a == "--skip-bundle");
    let verbose = args.iter().any(|a| a == "--verbose");

    let vst3_path = PathBuf::from("target/bundled/ultrawave.vst3");

    if !skip_bundle || !vst3_path.exists() {
        println!("Building and bundling VST3...");
        let status = Command::new("cargo")
            .args(["xtask", "bundle", "ultrawave", "--release"])
            .status()
            .expect("Failed to run cargo xtask bundle");

        if !status.success() {
            eprintln!("Bundle failed");
            return ExitCode::FAILURE;
        }
    }

    if !vst3_path.exists() {
        eprintln!("VST3 bundle not found at {:?}", vst3_path);
        return ExitCode::FAILURE;
    }

    println!();
    println!("=========================================");
    println!("VST3 Plugin Validation (pluginval)");
    println!("=========================================");
    println!("Plugin: {:?}", vst3_path);
    println!("Strictness: {} (1=lenient, 10=strict)", strictness);
    println!();

    let mut cmd_args = vec![
        "--validate-in-process".to_string(),
        "--strictness-level".to_string(),
        strictness.to_string(),
    ];

    if verbose {
        cmd_args.push("--verbose".to_string());
    }

    cmd_args.push(vst3_path.to_string_lossy().to_string());

    println!("Running: pluginval {}", cmd_args.join(" "));
    println!();

    let status = Command::new("pluginval")
        .args(&cmd_args)
        .status()
        .expect("Failed to run pluginval. Is it installed and in PATH?");

    println!();
    if status.success() {
        println!("=========================================");
        println!("✓ VST3 VALIDATION PASSED");
        println!("=========================================");
        ExitCode::SUCCESS
    } else {
        println!("=========================================");
        println!("✗ VST3 VALIDATION FAILED");
        println!("=========================================");
        ExitCode::FAILURE
    }
}

fn run_validate_clap(args: &[String]) -> ExitCode {
    let tests = args
        .iter()
        .position(|a| a == "--tests")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());

    let skip_bundle = args.iter().any(|a| a == "--skip-bundle");
    let verbose = args.iter().any(|a| a == "--verbose");

    let clap_path = PathBuf::from("target/bundled/ultrawave.clap");

    if !skip_bundle || !clap_path.exists() {
        println!("Building and bundling CLAP...");
        let status = Command::new("cargo")
            .args(["xtask", "bundle", "ultrawave", "--release"])
            .status()
            .expect("Failed to run cargo xtask bundle");

        if !status.success() {
            eprintln!("Bundle failed");
            return ExitCode::FAILURE;
        }
    }

    if !clap_path.exists() {
        eprintln!("CLAP bundle not found at {:?}", clap_path);
        return ExitCode::FAILURE;
    }

    println!();
    println!("=========================================");
    println!("CLAP Plugin Validation (clap-validator)");
    println!("=========================================");
    println!("Plugin: {:?}", clap_path);
    if let Some(t) = tests {
        println!("Tests: {}", t);
    } else {
        println!("Tests: all");
    }
    println!();

    let mut cmd_args = vec!["validate".to_string()];

    if verbose {
        cmd_args.push("--verbose".to_string());
    }

    if let Some(t) = tests {
        cmd_args.push("--only".to_string());
        cmd_args.push(t.to_string());
    }

    cmd_args.push(clap_path.to_string_lossy().to_string());

    println!("Running: clap-validator {}", cmd_args.join(" "));
    println!();

    let status = Command::new("clap-validator")
        .args(&cmd_args)
        .status()
        .expect("Failed to run clap-validator. Is it installed and in PATH?");

    println!();
    if status.success() {
        println!("=========================================");
        println!("✓ CLAP VALIDATION PASSED");
        println!("=========================================");
        ExitCode::SUCCESS
    } else {
        println!("=========================================");
        println!("✗ CLAP VALIDATION FAILED");
        println!("=========================================");
        ExitCode::FAILURE
    }
}

fn run_validate_all(args: &[String]) -> ExitCode {
    let skip_bundle = args.iter().any(|a| a == "--skip-bundle");

    if !ensure_bundled(skip_bundle) {
        return ExitCode::FAILURE;
    }

    println!();
    println!("#########################################");
    println!("# RUNNING ALL PLUGIN VALIDATIONS       #");
    println!("#########################################");
    println!();

    let mut all_args = args.to_vec();
    if !all_args.contains(&"--skip-bundle".to_string()) {
        all_args.push("--skip-bundle".to_string());
    }

    let vst3_result = run_validate_vst3(&all_args);
    println!();

    let clap_result = run_validate_clap(&all_args);
    println!();

    println!("#########################################");
    println!("# VALIDATION SUMMARY                   #");
    println!("#########################################");

    let vst3_ok = vst3_result == ExitCode::SUCCESS;
    let clap_ok = clap_result == ExitCode::SUCCESS;

    println!(
        "VST3 (pluginval):      {}",
        if vst3_ok { "✓ PASSED" } else { "✗ FAILED" }
    );
    println!(
        "CLAP (clap-validator): {}",
        if clap_ok { "✓ PASSED" } else { "✗ FAILED" }
    );
    println!();

    if vst3_ok && clap_ok {
        println!("All validations passed!");
        ExitCode::SUCCESS
    } else {
        println!("Some validations failed.");
        ExitCode::FAILURE
    }
}

fn install_plugins() -> ExitCode {
    let args: Vec<String> = env::args().skip(2).collect();
    let skip_bundle = args.iter().any(|a| a == "--skip-bundle");
    let system_install = args.iter().any(|a| a == "--system");

    let plugins = [
        ("ultrawave-record", "ultrawave_record"),
        ("ultrawave-play", "ultrawave_play"),
    ];
    let bundle_dir = PathBuf::from("target/bundled");

    if !skip_bundle {
        for (primary, fallback) in plugins {
            println!("Building and bundling {}...", primary);
            if !bundle_plugin(primary, fallback) {
                eprintln!("Bundle failed for {} ({})", primary, fallback);
                return ExitCode::FAILURE;
            }
        }
    }

    let (vst3_dest, clap_dest) = plugin_destinations(system_install);

    if fs::create_dir_all(&vst3_dest).is_err() {
        eprintln!("Failed to create VST3 destination at {:?}", vst3_dest);
        return ExitCode::FAILURE;
    }

    if fs::create_dir_all(&clap_dest).is_err() {
        eprintln!("Failed to create CLAP destination at {:?}", clap_dest);
        return ExitCode::FAILURE;
    }

    let mut had_errors = false;
    for (primary, fallback) in plugins {
        // Install VST3 bundle
        if let Some(vst3_src) = find_artifact(&bundle_dir, primary, fallback, "vst3") {
            let dest = vst3_dest.join(
                vst3_src
                    .file_name()
                    .expect("missing file name for VST3 source"),
            );
            println!("Installing VST3 to {:?}", dest);
            if let Err(err) = install_artifact(&vst3_src, &dest) {
                eprintln!("Failed to install VST3 for {}: {}", primary, err);
                had_errors = true;
            } else {
                println!("✓ VST3 installed for {}", primary);
            }
        } else {
            eprintln!(
                "VST3 bundle not found for {} (checked {} and {})",
                primary, primary, fallback
            );
            had_errors = true;
        }

        // Install CLAP bundle/binary
        if let Some(clap_src) = find_artifact(&bundle_dir, primary, fallback, "clap") {
            let dest = clap_dest.join(
                clap_src
                    .file_name()
                    .expect("missing file name for CLAP source"),
            );
            println!("Installing CLAP to {:?}", dest);
            if let Err(err) = install_artifact(&clap_src, &dest) {
                eprintln!("Failed to install CLAP for {}: {}", primary, err);
                had_errors = true;
            } else {
                println!("✓ CLAP installed for {}", primary);
            }
        } else {
            eprintln!(
                "CLAP bundle not found for {} (checked {} and {})",
                primary, primary, fallback
            );
            had_errors = true;
        }
    }

    if had_errors {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn bundle_plugin(primary: &str, fallback: &str) -> bool {
    let status = Command::new("cargo")
        .args(["xtask", "bundle", primary, "--release"])
        .status()
        .expect("Failed to run cargo xtask bundle");

    if status.success() {
        return true;
    }

    let fallback_status = Command::new("cargo")
        .args(["xtask", "bundle", fallback, "--release"])
        .status()
        .expect("Failed to run cargo xtask bundle");

    fallback_status.success()
}

fn find_artifact(bundle_dir: &Path, primary: &str, fallback: &str, ext: &str) -> Option<PathBuf> {
    let primary_path = bundle_dir.join(format!("{primary}.{ext}"));
    if primary_path.exists() {
        return Some(primary_path);
    }

    let fallback_path = bundle_dir.join(format!("{fallback}.{ext}"));
    if fallback_path.exists() {
        return Some(fallback_path);
    }

    None
}

fn plugin_destinations(system_install: bool) -> (PathBuf, PathBuf) {
    #[cfg(target_os = "macos")]
    {
        if system_install {
            (
                PathBuf::from("/Library/Audio/Plug-Ins/VST3"),
                PathBuf::from("/Library/Audio/Plug-Ins/CLAP"),
            )
        } else {
            let home = env::var("HOME").expect("HOME environment variable not set");
            (
                PathBuf::from(&home).join("Library/Audio/Plug-Ins/VST3"),
                PathBuf::from(&home).join("Library/Audio/Plug-Ins/CLAP"),
            )
        }
    }

    #[cfg(target_os = "linux")]
    {
        if system_install {
            (
                PathBuf::from("/usr/lib/vst3"),
                PathBuf::from("/usr/lib/clap"),
            )
        } else {
            let home = env::var("HOME").expect("HOME environment variable not set");
            (
                PathBuf::from(&home).join(".vst3"),
                PathBuf::from(&home).join(".clap"),
            )
        }
    }

    #[cfg(target_os = "windows")]
    {
        let base = if system_install {
            env::var("COMMONPROGRAMFILES")
                .unwrap_or_else(|_| "C:\\Program Files\\Common Files".into())
        } else {
            env::var("LOCALAPPDATA").unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".into())
        };
        (
            PathBuf::from(&base).join("VST3"),
            PathBuf::from(&base).join("CLAP"),
        )
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        panic!("Unsupported platform for plugin installation");
    }
}

fn install_artifact(src: &Path, dst: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        if dst.exists() {
            fs::remove_dir_all(dst)?;
        }
        copy_dir_recursive(src, dst)
    } else {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src, dst)?;
        Ok(())
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
