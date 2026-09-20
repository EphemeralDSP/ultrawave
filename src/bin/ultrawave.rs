use nice_plug::prelude::*;
use ultrawave::plugin::UltrawavePlugin;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();

    if let Some(config) = ultrawave::offline::parse_record_args(&args) {
        ultrawave::NO_GUI.store(true, std::sync::atomic::Ordering::Relaxed);
        ultrawave::offline::run_offline_record(&config).unwrap_or_else(|e| {
            eprintln!("offline record error: {e}");
            std::process::exit(1);
        });
        return;
    }

    if let Some(config) = ultrawave::offline::parse_play_args(&args) {
        ultrawave::offline::run_offline_play(&config).unwrap_or_else(|e| {
            eprintln!("offline play error: {e}");
            std::process::exit(1);
        });
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--no-gui") {
        args.remove(pos);
        ultrawave::NO_GUI.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let has_period_size = args
            .iter()
            .any(|arg| arg == "--period-size" || arg.starts_with("--period-size="));
        if !has_period_size {
            args.push("--period-size".to_string());
            args.push("2048".to_string());
        }
    }

    // Avoid JACK probe noise on Windows unless the user explicitly chose a backend.
    #[cfg(target_os = "windows")]
    {
        let has_backend = args
            .iter()
            .any(|arg| arg == "--backend" || arg.starts_with("--backend="));
        if !has_backend {
            args.push("--backend".to_string());
            args.push("wasapi".to_string());
        }
    }

    nice_export_standalone_with_args::<UltrawavePlugin, _>(args);
}
