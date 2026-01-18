use std::env;
use std::fs;
use std::path::PathBuf;
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
    install             Install bundled plugins to system directories

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
    cargo xtask bundle ultrawave --release
    cargo xtask validate-vst3 --strictness 5
    cargo xtask validate-clap
    cargo xtask validate-all
    cargo xtask install
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
    let bundle_dir = PathBuf::from("target/bundled");

    // Windows plugin paths
    let vst3_dest = PathBuf::from(
        env::var("COMMONPROGRAMFILES").unwrap_or_else(|_| "C:\\Program Files\\Common Files".into()),
    )
    .join("VST3");
    let clap_dest = PathBuf::from(
        env::var("COMMONPROGRAMFILES").unwrap_or_else(|_| "C:\\Program Files\\Common Files".into()),
    )
    .join("CLAP");

    // Install VST3
    let vst3_src = bundle_dir.join("ultrawave.vst3");
    if vst3_src.exists() {
        let dest = vst3_dest.join("ultrawave.vst3");
        println!("Installing VST3 to {:?}", dest);
        if dest.exists() {
            fs::remove_dir_all(&dest).expect("Failed to remove old VST3");
        }
        copy_dir_recursive(&vst3_src, &dest).expect("Failed to install VST3");
        println!("✓ VST3 installed");
    } else {
        eprintln!("VST3 bundle not found. Run `cargo xtask bundle ultrawave --release` first.");
    }

    // Install CLAP
    let clap_src = bundle_dir.join("ultrawave.clap");
    if clap_src.exists() {
        fs::create_dir_all(&clap_dest).ok();
        let dest = clap_dest.join("ultrawave.clap");
        println!("Installing CLAP to {:?}", dest);
        fs::copy(&clap_src, &dest).expect("Failed to install CLAP");
        println!("✓ CLAP installed");
    } else {
        eprintln!("CLAP bundle not found. Run `cargo xtask bundle ultrawave --release` first.");
    }

    ExitCode::SUCCESS
}

fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
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
