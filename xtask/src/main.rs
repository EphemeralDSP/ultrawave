use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 && args[1] == "install" {
        install_plugins();
    } else {
        let _ = nih_plug_xtask::main();
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
