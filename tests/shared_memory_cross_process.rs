use std::env;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use ultrawave::machines::shared::{get_global_registry, CHANNEL_COUNT};

#[test]
fn shared_memory_child_entrypoint() {
    let mode = match env::var("ULTRAWAVE_IPC_MODE") {
        Ok(mode) => mode,
        Err(_) => return,
    };

    let registry = get_global_registry();
    match mode.as_str() {
        "writer" => {
            registry.clear_all();
            for channel in 0..CHANNEL_COUNT {
                let base = (channel as i16) * 100;
                let samples = vec![base + 1, base + 2, base + 3, base + 4];
                assert!(registry.replace_buffer(channel, samples));
            }
        }
        "reader" => {
            for channel in 0..CHANNEL_COUNT {
                let base = (channel as i16) * 100;
                let expected = vec![base + 1, base + 2, base + 3, base + 4];
                let actual = registry
                    .read_buffer(channel)
                    .expect("channel should exist in shared memory");
                assert_eq!(actual, expected, "channel {channel} mismatch");
            }
        }
        _ => panic!("unknown ULTRAWAVE_IPC_MODE: {mode}"),
    }
}

#[test]
fn shared_memory_persists_across_processes() {
    let exe = env::current_exe().expect("current_exe should resolve");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos() as u64;
    let key = format!(
        "/uwt{:x}{:x}",
        std::process::id(),
        stamp & 0xFFFF_FFFF
    );

    let writer = Command::new(&exe)
        .arg("--exact")
        .arg("shared_memory_child_entrypoint")
        .env("ULTRAWAVE_SHM_KEY", &key)
        .env("ULTRAWAVE_IPC_MODE", "writer")
        .output()
        .expect("writer child should launch");
    assert!(
        writer.status.success(),
        "writer failed: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&writer.stdout),
        String::from_utf8_lossy(&writer.stderr)
    );

    let reader = Command::new(&exe)
        .arg("--exact")
        .arg("shared_memory_child_entrypoint")
        .env("ULTRAWAVE_SHM_KEY", &key)
        .env("ULTRAWAVE_IPC_MODE", "reader")
        .output()
        .expect("reader child should launch");
    assert!(
        reader.status.success(),
        "reader failed: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&reader.stdout),
        String::from_utf8_lossy(&reader.stderr)
    );
}
