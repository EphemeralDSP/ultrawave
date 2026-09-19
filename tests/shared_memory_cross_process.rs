use std::env;
use std::ffi::CString;
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use ultrawave::machines::ram_record::{RamRecord, RamRecordParams};
use ultrawave::machines::shared::{get_global_registry, CHANNEL_COUNT};

struct ShmCleanup(String);

impl Drop for ShmCleanup {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let name = CString::new(self.0.as_str()).unwrap();
            // SAFETY: the test generated this valid POSIX shared-memory name.
            let _ = unsafe { libc::shm_unlink(name.as_ptr()) };
        }
    }
}

fn record_params() -> RamRecordParams {
    RamRecordParams {
        mlev: 127,
        ilev: 0,
        ..Default::default()
    }
}

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
        "holder" => {
            let params = record_params();
            let mut recorders = Vec::new();
            for channel in 0..CHANNEL_COUNT {
                let mut recorder = RamRecord::with_registry(44_100.0, registry.clone());
                recorder.start_recording(&params, true, channel);
                assert!(recorder.is_recording(channel));
                recorder.record_sample(0.25, 0.25, 0.0, 0.0, &params, channel);
                recorders.push(recorder);
            }
            std::fs::write(env::var("ULTRAWAVE_READY_FILE").unwrap(), b"ready").unwrap();
            loop {
                thread::sleep(Duration::from_secs(1));
            }
        }
        "contender" => {
            let params = record_params();
            for channel in 0..CHANNEL_COUNT {
                let mut recorder = RamRecord::with_registry(44_100.0, registry.clone());
                recorder.start_recording(&params, true, channel);
                assert!(!recorder.is_recording(channel));
                assert!(!registry.replace_buffer(channel, vec![100 + channel as i16]));
                assert!(!registry.clear_buffer(channel));
            }
        }
        "recover" => {
            let params = record_params();
            for channel in 0..CHANNEL_COUNT {
                let mut recorder = RamRecord::with_registry(44_100.0, registry.clone());
                recorder.start_recording(&params, true, channel);
                assert!(recorder.is_recording(channel));
                let value = (channel as f32 + 1.0) / 8.0;
                recorder.record_sample(value, value, 0.0, 0.0, &params, channel);
                recorder.stop_all_recording();
                assert_eq!(
                    registry.read_buffer(channel).unwrap(),
                    [((channel + 1) * 256) as i16]
                );
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
    let key = format!("/uwt{:x}{:x}", std::process::id(), stamp & 0xFFFF_FFFF);
    let _cleanup = ShmCleanup(key.clone());

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

#[cfg(unix)]
#[test]
fn exclusive_writer_is_recovered_after_abnormal_process_exit() {
    let exe = env::current_exe().expect("current_exe should resolve");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let key = format!("/uwx{:x}{:x}", std::process::id(), stamp as u64);
    let _cleanup = ShmCleanup(key.clone());
    let ready = env::temp_dir().join(format!("ultrawave-ready-{}-{stamp}", std::process::id()));

    let mut holder = Command::new(&exe)
        .args(["--exact", "shared_memory_child_entrypoint"])
        .env("ULTRAWAVE_SHM_KEY", &key)
        .env("ULTRAWAVE_IPC_MODE", "holder")
        .env("ULTRAWAVE_READY_FILE", &ready)
        .spawn()
        .expect("holder child should launch");
    for _ in 0..200 {
        if ready.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(ready.exists(), "holder never acquired all channels");

    let contender = Command::new(&exe)
        .args(["--exact", "shared_memory_child_entrypoint"])
        .env("ULTRAWAVE_SHM_KEY", &key)
        .env("ULTRAWAVE_IPC_MODE", "contender")
        .output()
        .expect("contender child should launch");
    assert!(
        contender.status.success(),
        "contender failed: {}",
        String::from_utf8_lossy(&contender.stderr)
    );

    holder.kill().expect("holder should be killed abnormally");
    holder.wait().unwrap();
    let recovered = Command::new(&exe)
        .args(["--exact", "shared_memory_child_entrypoint"])
        .env("ULTRAWAVE_SHM_KEY", &key)
        .env("ULTRAWAVE_IPC_MODE", "recover")
        .output()
        .expect("recovery child should launch");
    assert!(
        recovered.status.success(),
        "recovery failed: {}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    let _ = std::fs::remove_file(ready);
}
