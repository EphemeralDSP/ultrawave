use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");

    let git_hash = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|hash| !hash.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=ULTRAWAVE_GIT_HASH={git_hash}");
}
