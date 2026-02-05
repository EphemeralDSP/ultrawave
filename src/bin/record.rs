use nih_plug::prelude::*;
use ultrawave::standalone::StandaloneRecord;

fn main() {
    // WASAPI can deliver larger buffers than nih-plug's default 512-period size.
    // If the user didn't specify a period size, pick a safer default to avoid panics.
    let args: Vec<String> = std::env::args().collect();

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

    nih_export_standalone_with_args::<StandaloneRecord, _>(args);
}
