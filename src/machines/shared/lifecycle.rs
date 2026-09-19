//! Non-real-time lifecycle control. Kernel-held file locks are the liveness
//! authority; shared session IDs identify incarnations, never PIDs or timeouts.
use super::*;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(super) const SESSION_SLOTS: usize = 64;

// Never unlink these files while a participant may hold one: a recreated file
// would create a second lock domain. Empty sidecars intentionally persist.
pub(super) struct LockFiles {
    directory: PathBuf,
    #[cfg(unix)]
    _directory_lease: File,
}

impl LockFiles {
    pub(super) fn new(key: &str) -> io::Result<Self> {
        #[cfg(unix)]
        let (root, name) = {
            // Use a common local path, independent of host HOME/TMPDIR. Regular
            // files support flock on both Linux and macOS; POSIX shm FDs need not.
            // SAFETY: geteuid has no arguments or memory preconditions.
            let uid = unsafe { libc::geteuid() };
            let name = key.strip_prefix('/').unwrap_or(key);
            if name.is_empty() || name.contains('/') || matches!(name, "." | "..") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid RAM key",
                ));
            }
            (
                PathBuf::from(format!("/tmp/ultrawave-{uid}")),
                name.to_owned(),
            )
        };
        #[cfg(windows)]
        let (root, name) = (
            env::temp_dir().join("ultrawave"),
            key.bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
        );
        private_directory(&root)?;
        let directory = root.join(name);
        private_directory(&directory)?;
        #[cfg(unix)]
        let directory_lease = {
            use std::os::unix::fs::MetadataExt;
            // tmpfiles.d(5): a locked directory and its subtree are excluded
            // from aging. Protect the gate too, even between maintenance polls.
            let file = File::open(&directory)?;
            file.lock_shared()?;
            let held = file.metadata()?;
            let named = fs::symlink_metadata(&directory)?;
            if held.ino() != named.ino() || held.dev() != named.dev() {
                return Err(io::Error::other("RAM lock directory changed while opening"));
            }
            file
        };
        Ok(Self {
            directory,
            #[cfg(unix)]
            _directory_lease: directory_lease,
        })
    }

    // Open a fresh description each time: dup/try_clone would share the same
    // flock and would NOT serialize two callers in the same process.
    pub(super) fn lock(&self, name: &str, blocking: bool) -> io::Result<Option<File>> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        // std opens non-inheritable descriptors. Closing the last descriptor
        // (including on process death) releases its lock automatically.
        let file = options.open(self.directory.join(name))?;
        if blocking {
            file.lock()?;
        } else {
            match file.try_lock() {
                Ok(()) => (),
                Err(TryLockError::WouldBlock) => return Ok(None),
                Err(TryLockError::Error(error)) => return Err(error),
            }
        }
        Ok(Some(file))
    }
}

fn private_directory(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(path) {
        Ok(()) => (),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error),
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "RAM lock path is not a directory",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: geteuid has no arguments or memory preconditions.
        if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "RAM lock directory is not private",
            ));
        }
    }
    Ok(())
}

/// Keep this guard alive from plugin initialization until audio processing has
/// stopped. Final drop joins the recovery worker before releasing the lease,
/// so no background thread can outlive the plugin's loaded library.
pub struct InstanceLease {
    registry: Arc<BufferRegistry>,
    id: u32,
    maintenance: Option<Maintenance>,
    _slot: Option<File>,
}

impl Drop for InstanceLease {
    fn drop(&mut self) {
        // A new registration may race this destructor after Weak::upgrade
        // fails. Do not clear that newer session's ID.
        let _ = self.registry.session_id.compare_exchange(
            self.id,
            0,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        drop(self.maintenance.take());
    }
}

struct Maintenance {
    stop: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl Maintenance {
    fn start(memory: Arc<SharedMemory>) -> io::Result<Self> {
        let (stop, receiver) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("ultrawave-ram".into())
            .spawn(move || {
                let mut reported_error = false;
                while let Err(RecvTimeoutError::Timeout) =
                    receiver.recv_timeout(Duration::from_millis(100))
                {
                    let result = memory.locks.lock("gate", false).and_then(|gate| {
                        if let Some(_gate) = gate {
                            memory.recover_sessions()?;
                        }
                        Ok(())
                    });
                    if let Err(error) = &result {
                        if !reported_error {
                            eprintln!("Ultrawave RAM recovery delayed: {error}");
                        }
                    }
                    reported_error = result.is_err();
                }
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Maintenance {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl BufferRegistry {
    /// Non-RT only. Local instances share one lease. The first registration
    /// after all leases disappear clears RAM atomically with respect to other
    /// registrations. Shared writes require a live guard; local/offline RAM does not.
    pub fn register_instance(self: &Arc<Self>) -> io::Result<Arc<InstanceLease>> {
        let mut cached = self
            .lease
            .lock()
            .map_err(|_| io::Error::other("RAM lifecycle poisoned"))?;
        if let Some(lease) = cached.upgrade() {
            return Ok(lease);
        }
        let (id, slot, maintenance) = match &self.backend {
            RegistryBackend::Local(_) => {
                self.clear_all();
                (0, None, None)
            }
            RegistryBackend::Shared(memory) => {
                let _gate = memory.locks.lock("gate", true)?;
                let live = memory.recover_sessions()?;
                let mut free = None;
                for (index, session) in memory.layout().sessions.iter().enumerate() {
                    if session.load(Ordering::Acquire) == 0 {
                        if let Some(file) = memory.locks.lock(&format!("session-{index}"), false)? {
                            free = Some((session, file));
                            break;
                        }
                    }
                }
                let (session, file) =
                    free.ok_or_else(|| io::Error::other("all RAM session slots are in use"))?;
                let id = memory
                    .layout()
                    .next_session_id
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                    .map_err(|_| io::Error::other("RAM session IDs exhausted"))?;
                if live == 0 {
                    for channel in &memory.layout().channels {
                        clear_abandoned(channel);
                    }
                }
                session.store(id, Ordering::Release);
                let maintenance = Maintenance::start(memory.clone())?;
                (id, Some(file), Some(maintenance))
            }
        };
        self.session_id.store(id, Ordering::Release);
        let lease = Arc::new(InstanceLease {
            registry: self.clone(),
            id,
            maintenance,
            _slot: slot,
        });
        *cached = Arc::downgrade(&lease);
        Ok(lease)
    }
}

impl SharedMemory {
    /// Caller holds the lifecycle gate. An exclusive slot lock proves that its
    /// previous session is gone, even after PID reuse or an arbitrarily long pause.
    fn recover_sessions(&self) -> io::Result<usize> {
        let mut live = 0;
        for (index, session) in self.layout().sessions.iter().enumerate() {
            let id = session.load(Ordering::Acquire);
            if id == 0 {
                continue;
            }
            if let Some(_dead) = self.locks.lock(&format!("session-{index}"), false)? {
                for channel in &self.layout().channels {
                    if (channel.writer.load(Ordering::Acquire) >> 32) as u32 == id {
                        clear_abandoned(channel);
                    }
                }
                session.store(0, Ordering::Release);
            } else {
                live += 1;
            }
        }
        Ok(live)
    }
}

/// Only for a proven-dead writer, or an epoch with no live participants. Keep
/// odd revisions invalid until every metadata field is coherent, then release
/// writer ownership last. Completed takes on other channels are untouched.
fn clear_abandoned(channel: &SharedChannelLayout) {
    let odd = channel.revision.load(Ordering::Acquire) | 1;
    channel.revision.store(odd, Ordering::Release);
    channel.length.store(0, Ordering::Release);
    channel.write_position.store(0, Ordering::Release);
    channel
        .sample_rate_bits
        .store(DEFAULT_SAMPLE_RATE.to_bits(), Ordering::Release);
    channel
        .revision
        .store(odd.wrapping_add(1), Ordering::Release);
    channel.writer.store(0, Ordering::Release);
}
