use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 && args[1] == "install" {
        install_plugins();
    } else if args.len() > 1 && args[1] == "pluginval" {
        run_pluginval(&args[2..]);
    } else {
        let _ = nih_plug_xtask::main();
    }
}

fn run_pluginval(args: &[String]) {
    let strictness = args.iter()
        .position(|a| a == "--strictness")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("5");
    
    let skip_bundle = args.iter().any(|a| a == "--skip-bundle");
    
    let vst3_path = PathBuf::from("target/bundled/ultrawave.vst3");
    
    if !skip_bundle || !vst3_path.exists() {
        println!("Building and bundling VST3...");
        let status = Command::new("cargo")
            .args(["xtask", "bundle", "ultrawave", "--release"])
            .status()
            .expect("Failed to run cargo xtask bundle");
        
        if !status.success() {
            eprintln!("Bundle failed");
            std::process::exit(1);
        }
    }
    
    if !vst3_path.exists() {
        eprintln!("VST3 bundle not found at {:?}", vst3_path);
        std::process::exit(1);
    }
    
    println!("Running pluginval with strictness {}...", strictness);
    let status = Command::new("pluginval")
        .args([
            "--strictness-level", strictness,
            "--validate", vst3_path.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run pluginval. Is it installed and in PATH?");
    
    if status.success() {
        println!("✓ pluginval passed");
    } else {
        eprintln!("✗ pluginval failed");
        std::process::exit(1);
    }
}

fn install_plugins() {
    let bundle_dir = PathBuf::from("target/bundled");
    
    // Windows plugin paths
    let vst3_dest = PathBuf::from(env::var("COMMONPROGRAMFILES").unwrap_or_else(|_| "C:\\Program Files\\Common Files".into()))
        .join("VST3");
    let clap_dest = PathBuf::from(env::var("COMMONPROGRAMFILES").unwrap_or_else(|_| "C:\\Program Files\\Common Files".into()))
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
