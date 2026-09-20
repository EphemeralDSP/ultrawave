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
            _ => match nice_plug_xtask::main() {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("xtask failed: {err}");
                    ExitCode::FAILURE
                }
            },
        }
    } else {
        match nice_plug_xtask::main() {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("xtask failed: {err}");
                ExitCode::FAILURE
            }
        }
    }
}

fn print_help() {
    println!(
        r#"Ultrawave xtask commands

USAGE:
    cargo xtask <COMMAND> [OPTIONS]

COMMANDS:
    bundle              Bundle plugins (from nice-plug-xtask)
    install             Build + install Ultrawave to platform plugin directories

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
    cargo xtask bundle ultrawave-plugin --release
    cargo xtask validate-vst3 --strictness 5
    cargo xtask validate-clap
    cargo xtask validate-all
    cargo xtask install
    cargo xtask install --system
"#
    );
}

fn ensure_bundled(skip_bundle: bool) -> bool {
    let plugins = ["ultrawave-plugin"];
    if skip_bundle {
        for plugin in plugins {
            for ext in ["vst3", "clap"] {
                let path = PathBuf::from(format!("target/bundled/{plugin}.{ext}"));
                if !path.exists() {
                    eprintln!("--skip-bundle requires existing {}", path.display());
                    return false;
                }
            }
        }
        return true;
    }
    for plugin in plugins {
        println!("Building and bundling {plugin}...");
        match Command::new("cargo")
            .args(["xtask", "bundle", plugin, "--release"])
            .status()
        {
            Ok(status) if status.success() => {}
            Ok(_) => {
                eprintln!("Bundle failed for {plugin}");
                return false;
            }
            Err(err) => {
                eprintln!("Could not bundle {plugin}: {err}");
                return false;
            }
        }
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

    if args.iter().any(|a| a == "--strictness")
        && args.iter().position(|a| a == "--strictness").unwrap() + 1 == args.len()
    {
        eprintln!("missing value for --strictness");
        return ExitCode::FAILURE;
    }
    if !ensure_bundled(skip_bundle) {
        return ExitCode::FAILURE;
    }
    {
        let plugin = "ultrawave-plugin";
        let mut command = Command::new("scripts/validate-vst3.sh");
        command.args([
            "--plugin",
            &format!("target/bundled/{plugin}.vst3"),
            "--strictness",
            strictness,
        ]);
        if verbose {
            command.arg("--verbose");
        }
        match command.status() {
            Ok(status) if status.success() => {}
            _ => return ExitCode::FAILURE,
        }
    }
    ExitCode::SUCCESS
}

fn run_validate_clap(args: &[String]) -> ExitCode {
    let tests = args
        .iter()
        .position(|a| a == "--tests")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());

    let skip_bundle = args.iter().any(|a| a == "--skip-bundle");
    let verbose = args.iter().any(|a| a == "--verbose");

    if args.iter().any(|a| a == "--tests")
        && args.iter().position(|a| a == "--tests").unwrap() + 1 == args.len()
    {
        eprintln!("missing value for --tests");
        return ExitCode::FAILURE;
    }
    if !ensure_bundled(skip_bundle) {
        return ExitCode::FAILURE;
    }
    {
        let plugin = "ultrawave-plugin";
        let mut command = Command::new("scripts/validate-clap.sh");
        command.args(["--plugin", &format!("target/bundled/{plugin}.clap")]);
        if let Some(t) = tests {
            command.args(["--tests", t]);
        }
        if verbose {
            command.arg("--verbose");
        }
        match command.status() {
            Ok(status) if status.success() => {}
            _ => return ExitCode::FAILURE,
        }
    }
    ExitCode::SUCCESS
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

    let plugins = ["ultrawave-plugin"];
    let bundle_dir = PathBuf::from("target/bundled");

    if !skip_bundle {
        for primary in plugins {
            println!("Building and bundling {}...", primary);
            if !bundle_plugin(primary) {
                eprintln!("Bundle failed for {}", primary);
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
    for primary in plugins {
        // Install VST3 bundle
        if let Some(vst3_src) = find_artifact(&bundle_dir, primary, "vst3") {
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
            eprintln!("VST3 bundle not found for {}", primary);
            had_errors = true;
        }

        // Install CLAP bundle/binary
        if let Some(clap_src) = find_artifact(&bundle_dir, primary, "clap") {
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
            eprintln!("CLAP bundle not found for {}", primary);
            had_errors = true;
        }
    }

    if had_errors {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn bundle_plugin(primary: &str) -> bool {
    let status = Command::new("cargo")
        .args(["xtask", "bundle", primary, "--release"])
        .status()
        .expect("Failed to run cargo xtask bundle");

    status.success()
}

fn find_artifact(bundle_dir: &Path, primary: &str, ext: &str) -> Option<PathBuf> {
    let primary_path = bundle_dir.join(format!("{primary}.{ext}"));
    if primary_path.exists() {
        return Some(primary_path);
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
