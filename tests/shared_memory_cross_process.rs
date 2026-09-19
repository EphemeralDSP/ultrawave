use std::env;
use std::ffi::CString;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

use ultrawave::machines::ram_record::{RamRecord, RamRecordParams};
use ultrawave::machines::shared::{get_global_registry, register_plugin_instance, CHANNEL_COUNT};

struct ShmCleanup(String);

impl ShmCleanup {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Self(format!("/uws{:x}{:x}", std::process::id(), stamp as u64));
        std::fs::create_dir(fixture.directory()).unwrap();
        fixture
    }

    fn directory(&self) -> PathBuf {
        env::temp_dir().join(self.0.trim_start_matches('/'))
    }

    fn command(&self, mode: &str) -> Command {
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .args(["--exact", "shared_memory_child_entrypoint"])
            .env("ULTRAWAVE_SHM_KEY", &self.0)
            .env("ULTRAWAVE_IPC_MODE", mode)
            .env("ULTRAWAVE_SYNC_DIR", self.directory());
        command
    }
}

impl Drop for ShmCleanup {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let name = CString::new(self.0.as_str()).unwrap();
            // SAFETY: the test generated this valid POSIX shared-memory name.
            let _ = unsafe { libc::shm_unlink(name.as_ptr()) };
            // Only this test's namespace, after all child guards have exited.
            let _ = std::fs::remove_dir_all(format!(
                "/tmp/ultrawave-{}/{}",
                unsafe { libc::geteuid() },
                self.0.trim_start_matches('/')
            ));
            let _ = std::fs::remove_dir_all(self.directory());
        }
    }
}

struct TestChild(Child);
impl TestChild {
    fn success(&mut self) {
        wait_for("child exit", || self.0.try_wait().unwrap().is_some());
        assert!(self.0.wait().unwrap().success(), "child assertions failed");
    }
}

impl Drop for TestChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_for(label: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready() {
        assert!(Instant::now() < deadline, "timed out: {label}");
        thread::sleep(Duration::from_millis(5));
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

    let sync = PathBuf::from(env::var("ULTRAWAVE_SYNC_DIR").unwrap_or_default());
    if mode == "join" {
        std::fs::write(
            sync.join(format!("before-{}", env::var("ULTRAWAVE_CHANNEL").unwrap())),
            b"",
        )
        .unwrap();
        wait_for("simultaneous start", || sync.join("go").exists());
    }
    let registry = get_global_registry();
    let _instance = (mode != "reader").then(|| register_plugin_instance().unwrap());
    match mode.as_str() {
        "partial-holder" => {
            let mut recorder = RamRecord::with_registry(44100.0, registry.clone());
            recorder.start_recording(&record_params(), true, 0);
            assert!(recorder.is_recording(0));
            recorder.record_mono(0.25, 0);
            assert!(registry.replace_buffer_at_rate(2, vec![17, -91, 777], 48000.0));
            std::fs::write(sync.join("holder"), b"").unwrap();
            loop {
                thread::sleep(Duration::from_secs(1));
            }
        }
        "survivor" => {
            let mut recorder = RamRecord::with_registry(44100.0, registry.clone());
            recorder.start_recording(&record_params(), true, 1);
            assert!(recorder.is_recording(1));
            recorder.record_mono(-0.375, 1);
            std::fs::write(sync.join("survivor"), b"").unwrap();
            wait_for("paused owner", || sync.join("paused").exists());
            // Several maintenance passes elapse while the other host is stopped.
            // A lease must not expire just because the holder cannot run.
            thread::sleep(Duration::from_millis(350));
            assert!(!registry.clear_buffer(0));
            assert_eq!(registry.read_buffer(0).unwrap(), [512]);
            std::fs::write(sync.join("pause-verified"), b"").unwrap();
            wait_for("already-attached recovery", || registry.buffer_len(0) == 0);
            recorder.start_recording(&record_params(), true, 0);
            assert!(recorder.is_recording(0));
            recorder.record_mono(0.75, 0);
            recorder.record_mono(-0.5, 1);
            assert_eq!(registry.read_buffer(0).unwrap(), [1536]);
            assert_eq!(registry.read_buffer(1).unwrap(), [-768, -1024]);
            assert_eq!(registry.read_buffer(2).unwrap(), [17, -91, 777]);
            assert_eq!(registry.snapshot(2).unwrap().1, 48000.0);
            recorder.stop_all_recording();
        }
        "join" => {
            let channel: usize = env::var("ULTRAWAVE_CHANNEL").unwrap().parse().unwrap();
            assert!(registry.replace_buffer(channel, vec![100 + channel as i16]));
            std::fs::write(sync.join(format!("ready-{channel}")), b"").unwrap();
            wait_for("all registrations", || {
                (0..CHANNEL_COUNT).all(|i| sync.join(format!("ready-{i}")).exists())
            });
            for i in 0..CHANNEL_COUNT {
                assert_eq!(registry.read_buffer(i).unwrap(), [100 + i as i16]);
            }
            // Keep leases alive until every peer has read the buffers.
            std::fs::write(sync.join(format!("read-{channel}")), b"").unwrap();
            wait_for("all readers", || {
                (0..CHANNEL_COUNT).all(|i| sync.join(format!("read-{i}")).exists())
            });
        }
        "reopen" => {
            let mut recorder = RamRecord::with_registry(44100.0, registry.clone());
            assert!(registry.replace_buffer(0, vec![512]));
            let second = register_plugin_instance().unwrap();
            drop(_instance);
            assert_eq!(
                registry.read_buffer(0).unwrap(),
                [512],
                "another instance is still open"
            );
            drop(second);
            assert!(!registry.write_sample(0, 900), "shared writes need a lease");
            let _reopened = register_plugin_instance().unwrap();
            assert!(registry.read_buffer(0).unwrap().is_empty());
            recorder.start_recording(&record_params(), true, 0);
            recorder.record_mono(0.5, 0);
            assert_eq!(
                registry.read_buffer(0).unwrap(),
                [1024],
                "recorder uses new incarnation"
            );
            recorder.stop_all_recording();
        }
        "lifecycle-write" => {
            assert!(registry.replace_buffer(0, vec![512, -1024, 1536]));
            // Exit without destructors, as a host abort would. Completed data
            // has no active writer, so dead-writer recovery alone cannot fix it.
            std::process::exit(0);
        }
        "lifecycle-empty" => {
            for channel in 0..CHANNEL_COUNT {
                assert!(
                    registry.read_buffer(channel).unwrap().is_empty(),
                    "stale RAM after host exit"
                );
            }
        }
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

    let mut holder = TestChild(
        Command::new(&exe)
            .args(["--exact", "shared_memory_child_entrypoint"])
            .env("ULTRAWAVE_SHM_KEY", &key)
            .env("ULTRAWAVE_IPC_MODE", "holder")
            .env("ULTRAWAVE_READY_FILE", &ready)
            .spawn()
            .expect("holder child should launch"),
    );
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

    holder.0.kill().expect("holder should be killed abnormally");
    holder.0.wait().unwrap();
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

#[cfg(unix)]
#[test]
fn first_plugin_after_host_exit_without_destructors_gets_empty_ram() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let key = format!("/uwc{:x}{:x}", std::process::id(), stamp as u64);
    let _cleanup = ShmCleanup(key.clone());
    for mode in ["lifecycle-write", "lifecycle-empty"] {
        let result = Command::new(env::current_exe().unwrap())
            .args(["--exact", "shared_memory_child_entrypoint"])
            .env("ULTRAWAVE_SHM_KEY", &key)
            .env("ULTRAWAVE_IPC_MODE", mode)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&result.stdout)
        );
    }
}

#[cfg(unix)]
#[test]
fn attached_survivor_recovers_crashed_writer_but_preserves_live_and_completed_takes() {
    let fixture = ShmCleanup::new();
    let mut holder = TestChild(fixture.command("partial-holder").spawn().unwrap());
    wait_for("holder ready", || {
        fixture.directory().join("holder").exists()
    });
    let mut survivor = TestChild(fixture.command("survivor").spawn().unwrap());
    wait_for("survivor attached", || {
        fixture.directory().join("survivor").exists()
    });
    // SAFETY: signal only the still-owned test child; never a user DAW.
    assert_eq!(
        unsafe { libc::kill(holder.0.id() as libc::pid_t, libc::SIGSTOP) },
        0
    );
    std::fs::write(fixture.directory().join("paused"), b"").unwrap();
    wait_for("paused owner preserved", || {
        fixture.directory().join("pause-verified").exists()
    });
    holder.0.kill().unwrap();
    holder.0.wait().unwrap();
    survivor.success();
}

#[cfg(unix)]
#[test]
fn simultaneous_first_registrations_do_not_clear_another_new_instances_audio() {
    for _ in 0..10 {
        let fixture = ShmCleanup::new();
        let mut children: Vec<_> = (0..CHANNEL_COUNT)
            .map(|channel| {
                TestChild(
                    fixture
                        .command("join")
                        .env("ULTRAWAVE_CHANNEL", channel.to_string())
                        .spawn()
                        .unwrap(),
                )
            })
            .collect();
        wait_for("children at barrier", || {
            (0..CHANNEL_COUNT).all(|i| fixture.directory().join(format!("before-{i}")).exists())
        });
        std::fs::write(fixture.directory().join("go"), b"").unwrap();
        for child in &mut children {
            child.success();
        }
    }
}

#[cfg(unix)]
#[test]
fn final_local_instance_drop_and_reopen_starts_a_new_session() {
    let fixture = ShmCleanup::new();
    TestChild(fixture.command("reopen").spawn().unwrap()).success();
}
