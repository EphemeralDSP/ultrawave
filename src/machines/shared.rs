use std::env;
use std::ffi::CString;
use std::io;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicI16, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

mod lifecycle;
pub use lifecycle::InstanceLease;
use lifecycle::{LockFiles, SESSION_SLOTS};

pub const CHANNEL_COUNT: usize = 4;

/// Maximum recording length in seconds.
const MAX_RECORD_SECONDS: f32 = 10.0;

/// Sample rate used for buffer allocation (44.1kHz standard).
const DEFAULT_SAMPLE_RATE: f32 = 44100.0;

/// Number of samples allocated per buffer (10s @ 44.1kHz).
pub const MAX_BUFFER_SAMPLES: usize = (DEFAULT_SAMPLE_RATE * MAX_RECORD_SECONDS) as usize;

const SHARED_MAGIC: u32 = 0x5557_524D; // "UWRM"
const SHARED_VERSION: u32 = 3;
const INIT_UNINITIALIZED: u32 = 0;
const INIT_IN_PROGRESS: u32 = 1;
const INIT_READY: u32 = 2;

#[cfg(unix)]
const DEFAULT_SHM_KEY: &str = "/ultrawave_global_buffers_v3";
#[cfg(windows)]
const DEFAULT_SHM_KEY: &str = "Local\\UltrawaveGlobalBuffersV3";

#[repr(C, align(64))]
struct SharedChannelLayout {
    write_position: AtomicUsize,
    length: AtomicUsize,
    sample_rate_bits: AtomicU32,
    revision: AtomicU64,
    writer: AtomicU64,
    samples: [AtomicI16; MAX_BUFFER_SAMPLES],
}

#[repr(C)]
struct SharedLayout {
    magic: AtomicU32,
    version: AtomicU32,
    init_state: AtomicU32,
    next_writer_id: AtomicU32,
    next_session_id: AtomicU32,
    sessions: [AtomicU32; SESSION_SLOTS],
    channels: [SharedChannelLayout; CHANNEL_COUNT],
}

struct SharedMemory {
    mapping: platform::SharedMapping,
    locks: LockFiles,
}

impl SharedMemory {
    fn open() -> io::Result<Self> {
        let key = shared_memory_key();
        Self::open_key(&key)
    }

    fn open_key(key: &str) -> io::Result<Self> {
        let locks = LockFiles::new(key)?;
        // Covers file sizing AND initialization. A crashed initializer releases
        // the gate, so its successor can finish even a zero-length mapping.
        let _gate = locks.lock("gate", true)?;
        let mapping = platform::SharedMapping::open(key)?;
        let memory = Self { mapping, locks };
        memory.ensure_initialized()?;
        Ok(memory)
    }

    fn layout(&self) -> &SharedLayout {
        // SAFETY: The mapping is created for exactly `SharedLayout` bytes and
        // lives as long as `self`.
        unsafe { &*self.mapping.as_ptr() }
    }

    fn ensure_initialized(&self) -> io::Result<()> {
        let layout = self.layout();
        match layout.init_state.load(Ordering::Acquire) {
            INIT_READY
                if layout.magic.load(Ordering::Acquire) == SHARED_MAGIC
                    && layout.version.load(Ordering::Acquire) == SHARED_VERSION =>
            {
                Ok(())
            }
            INIT_UNINITIALIZED | INIT_IN_PROGRESS => {
                // Holding the OS gate proves no initializer is still running.
                layout.init_state.store(INIT_IN_PROGRESS, Ordering::Release);
                self.initialize_layout();
                Ok(())
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "incompatible Ultrawave RAM layout; use a distinct ULTRAWAVE_SHM_KEY",
            )),
        }
    }

    fn initialize_layout(&self) {
        let layout = self.layout();
        for channel in &layout.channels {
            channel.length.store(0, Ordering::Release);
            channel.write_position.store(0, Ordering::Release);
            channel
                .sample_rate_bits
                .store(DEFAULT_SAMPLE_RATE.to_bits(), Ordering::Release);
            channel.revision.store(0, Ordering::Release);
            channel.writer.store(0, Ordering::Release);
        }
        for session in &layout.sessions {
            session.store(0, Ordering::Release);
        }
        layout.next_session_id.store(1, Ordering::Relaxed);
        layout.next_writer_id.store(1, Ordering::Relaxed);
        layout.magic.store(SHARED_MAGIC, Ordering::Release);
        layout.version.store(SHARED_VERSION, Ordering::Release);
        layout.init_state.store(INIT_READY, Ordering::Release);
    }
}

/// Local lock-free buffer implementation used for explicit `BufferRegistry::new()`.
/// The plugin/global path uses `SharedMemory`.
struct LocalBuffer(Box<SharedChannelLayout>);

impl LocalBuffer {
    fn new() -> Self {
        // SAFETY: All fields are integer atomics, for which the all-zero bit
        // pattern is valid. Allocation is performed directly on the heap so
        // this ~0.9 MiB value is never materialized on the thread stack.
        let buffer = unsafe { Box::<SharedChannelLayout>::new_zeroed().assume_init() };
        buffer
            .sample_rate_bits
            .store(DEFAULT_SAMPLE_RATE.to_bits(), Ordering::Relaxed);
        Self(buffer)
    }
}

struct LocalRegistry {
    buffers: [LocalBuffer; CHANNEL_COUNT],
    next_writer_id: AtomicU32,
}

impl LocalRegistry {
    fn new() -> Self {
        Self {
            buffers: [
                LocalBuffer::new(),
                LocalBuffer::new(),
                LocalBuffer::new(),
                LocalBuffer::new(),
            ],
            next_writer_id: AtomicU32::new(1),
        }
    }
}

enum RegistryBackend {
    Local(LocalRegistry),
    Shared(Arc<SharedMemory>),
}

/// Global registry for shared buffers R1-R4.
pub struct BufferRegistry {
    backend: RegistryBackend,
    session_id: AtomicU32,
    lease: Mutex<Weak<InstanceLease>>,
}

impl BufferRegistry {
    /// IDs come from RAM, not a dylib-local static: CLAP and VST3 may coexist.
    pub(crate) fn writer_token(&self) -> u64 {
        let (session, counter) = match &self.backend {
            RegistryBackend::Local(local) => (1, &local.next_writer_id),
            RegistryBackend::Shared(shared) => (
                self.session_id.load(Ordering::Acquire),
                &shared.layout().next_writer_id,
            ),
        };
        if session == 0 {
            return 0;
        }
        // Never wrap into an existing owner's identity. Zero rejects the write.
        let Ok(id) =
            counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        else {
            return 0;
        };
        ((session as u64) << 32) | id as u64
    }

    /// Creates an isolated local registry (primarily for tests and offline flows).
    pub fn new() -> Self {
        Self {
            backend: RegistryBackend::Local(LocalRegistry::new()),
            session_id: AtomicU32::new(0),
            lease: Mutex::new(Weak::new()),
        }
    }

    fn new_shared() -> io::Result<Self> {
        Ok(Self {
            backend: RegistryBackend::Shared(Arc::new(SharedMemory::open()?)),
            session_id: AtomicU32::new(0),
            lease: Mutex::new(Weak::new()),
        })
    }

    fn new_global() -> Self {
        if cfg!(test) {
            return Self::new();
        }
        match Self::new_shared() {
            Ok(registry) => registry,
            Err(err) => {
                eprintln!("Ultrawave shared memory unavailable, falling back to local RAM: {err}");
                Self::new()
            }
        }
    }

    fn channel(&self, channel: usize) -> Option<&SharedChannelLayout> {
        match &self.backend {
            RegistryBackend::Local(local) => local.buffers.get(channel).map(|buffer| &*buffer.0),
            RegistryBackend::Shared(shared) => shared.layout().channels.get(channel),
        }
    }

    fn shared_len(channel: &SharedChannelLayout) -> usize {
        channel.length.load(Ordering::Acquire)
    }

    fn with_writer(
        &self,
        channel: usize,
        action: impl FnOnce(&SharedChannelLayout) -> bool,
    ) -> bool {
        let Some(ch) = self.channel(channel) else {
            return false;
        };
        let token = self.writer_token();
        if token == 0
            || ch
                .writer
                .compare_exchange(0, token, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
        {
            return false;
        }
        let result = action(ch);
        ch.writer.store(0, Ordering::Release);
        result
    }

    fn append(channel: &SharedChannelLayout, sample: i16) -> bool {
        let position = channel.write_position.load(Ordering::Relaxed);
        if position >= MAX_BUFFER_SAMPLES {
            return false;
        }
        channel.samples[position].store(sample, Ordering::Release);
        channel
            .write_position
            .store(position + 1, Ordering::Relaxed);
        channel.length.store(position + 1, Ordering::Release);
        true
    }

    fn shared_replace_buffer(channel: &SharedChannelLayout, samples: &[i16], sample_rate: f32) {
        channel.revision.fetch_add(1, Ordering::AcqRel);
        let len = samples.len().min(MAX_BUFFER_SAMPLES);
        for (idx, sample) in samples.iter().take(len).enumerate() {
            channel.samples[idx].store(*sample, Ordering::Release);
        }
        channel.length.store(len, Ordering::Release);
        channel.write_position.store(len, Ordering::Release);
        channel
            .sample_rate_bits
            .store(sample_rate.max(1.0).to_bits(), Ordering::Release);
        channel.revision.fetch_add(1, Ordering::Release);
    }

    fn shared_clear_buffer(channel: &SharedChannelLayout) {
        channel.revision.fetch_add(1, Ordering::AcqRel);
        channel.length.store(0, Ordering::Release);
        channel.write_position.store(0, Ordering::Release);
        channel.revision.fetch_add(1, Ordering::Release);
    }

    /// Get a clone of the buffer data for reading.
    pub fn read_buffer(&self, channel: usize) -> Option<Vec<i16>> {
        let (len, _, revision) = self.snapshot(channel)?;
        let ch = self.channel(channel)?;
        let samples = ch.samples[..len]
            .iter()
            .map(|s| s.load(Ordering::Acquire))
            .collect();
        (ch.revision.load(Ordering::Acquire) == revision).then_some(samples)
    }

    /// Write a sample to the buffer.
    pub fn write_sample(&self, channel: usize, sample: i16) -> bool {
        self.with_writer(channel, |ch| Self::append(ch, sample))
    }

    /// Replace entire buffer contents.
    pub fn replace_buffer(&self, channel: usize, samples: Vec<i16>) -> bool {
        self.replace_buffer_at_rate(channel, samples, DEFAULT_SAMPLE_RATE)
    }

    pub fn replace_buffer_at_rate(
        &self,
        channel: usize,
        samples: Vec<i16>,
        sample_rate: f32,
    ) -> bool {
        if !sample_rate.is_finite() || sample_rate <= 0.0 {
            return false;
        }
        self.with_writer(channel, |ch| {
            Self::shared_replace_buffer(ch, &samples, sample_rate);
            true
        })
    }

    /// Get current buffer length.
    pub fn buffer_len(&self, channel: usize) -> usize {
        self.channel(channel).map_or(0, Self::shared_len)
    }

    /// Clear a specific buffer.
    pub fn clear_buffer(&self, channel: usize) -> bool {
        self.with_writer(channel, |ch| {
            Self::shared_clear_buffer(ch);
            true
        })
    }

    /// Clear all buffers.
    pub fn clear_all(&self) {
        for channel in 0..CHANNEL_COUNT {
            self.clear_buffer(channel);
        }
    }

    /// Reset write position and truncate the readable buffer.
    pub fn reset_write_position(&self, channel: usize) -> bool {
        self.with_writer(channel, |ch| {
            ch.revision.fetch_add(1, Ordering::AcqRel);
            ch.write_position.store(0, Ordering::Relaxed);
            ch.length.store(0, Ordering::Release);
            ch.revision.fetch_add(1, Ordering::Release);
            true
        })
    }

    pub(crate) fn claim_writer(
        &self,
        channel: usize,
        token: u64,
        _clear: bool,
        sample_rate: f32,
    ) -> bool {
        let Some(ch) = self.channel(channel) else {
            return false;
        };
        if token == 0
            || ch
                .writer
                .compare_exchange(0, token, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return false;
        }
        ch.revision.fetch_add(1, Ordering::AcqRel);
        ch.write_position.store(0, Ordering::Relaxed);
        // A new take always starts with an empty readable prefix. Retaining a
        // stale tail while overwriting it would violate the reader revision.
        ch.length.store(0, Ordering::Release);
        ch.sample_rate_bits
            .store(sample_rate.max(1.0).to_bits(), Ordering::Release);
        ch.revision.fetch_add(1, Ordering::Release);
        true
    }

    pub(crate) fn write_sample_owned(&self, channel: usize, token: u64, sample: i16) -> bool {
        let Some(ch) = self.channel(channel) else {
            return false;
        };
        ch.writer.load(Ordering::Acquire) == token && Self::append(ch, sample)
    }

    pub(crate) fn release_writer(&self, channel: usize, token: u64) {
        if let Some(ch) = self.channel(channel) {
            let _ = ch
                .writer
                .compare_exchange(token, 0, Ordering::Release, Ordering::Relaxed);
        }
    }

    /// O(1) consistent metadata snapshot for real-time readers.
    pub fn snapshot(&self, channel: usize) -> Option<(usize, f32, u64)> {
        let ch = self.channel(channel)?;
        let revision = ch.revision.load(Ordering::Acquire);
        if revision & 1 != 0 {
            return None;
        }
        let len = ch.length.load(Ordering::Acquire).min(MAX_BUFFER_SAMPLES);
        let rate = f32::from_bits(ch.sample_rate_bits.load(Ordering::Acquire));
        (ch.revision.load(Ordering::Acquire) == revision).then_some((len, rate, revision))
    }

    #[inline]
    pub fn read_sample_pair(
        &self,
        channel: usize,
        index: usize,
        revision: u64,
        latched_len: usize,
    ) -> Option<(i16, i16)> {
        let ch = self.channel(channel)?;
        if ch.revision.load(Ordering::Acquire) != revision {
            return None;
        }
        let len = ch.length.load(Ordering::Acquire).min(latched_len);
        if index >= len {
            return None;
        }
        let a = ch.samples[index].load(Ordering::Acquire);
        let b = ch.samples[(index + 1).min(len - 1)].load(Ordering::Acquire);
        (ch.revision.load(Ordering::Acquire) == revision).then_some((a, b))
    }
}

impl Default for BufferRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn shared_memory_key() -> String {
    let key = env::var("ULTRAWAVE_SHM_KEY")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_SHM_KEY.to_string());

    #[cfg(unix)]
    {
        if key.starts_with('/') {
            key
        } else {
            format!("/{key}")
        }
    }
    #[cfg(windows)]
    {
        key
    }
}

// Cache weakly so final plugin/editor teardown also unmaps RAM and closes its
// descriptor. A strong static would leak those resources on library unload.
static GLOBAL_REGISTRY: Mutex<Weak<BufferRegistry>> = Mutex::new(Weak::new());

/// Get the global registry during setup or on the UI thread, never in process().
/// Audio machines retain their own Arc and access RAM directly.
pub fn get_global_registry() -> Arc<BufferRegistry> {
    let mut cached = GLOBAL_REGISTRY.lock().expect("RAM registry poisoned");
    if let Some(registry) = cached.upgrade() {
        return registry;
    }
    let registry = Arc::new(BufferRegistry::new_global());
    *cached = Arc::downgrade(&registry);
    registry
}

/// Register a plugin instance lifecycle.
///
/// When the first instance is created after a full teardown, RAM buffers are
/// cleared to guarantee volatile/non-persistent behavior across host reloads.
pub fn register_plugin_instance() -> io::Result<Arc<InstanceLease>> {
    get_global_registry().register_instance()
}

#[cfg(unix)]
mod platform {
    use super::*;
    use libc::{
        close, fstat, ftruncate, mmap, munmap, off_t, shm_open, stat, MAP_FAILED, MAP_SHARED,
        O_CREAT, O_EXCL, O_RDWR, PROT_READ, PROT_WRITE,
    };
    use std::os::fd::RawFd;

    pub(super) struct SharedMapping {
        ptr: NonNull<SharedLayout>,
        len: usize,
        fd: RawFd,
    }

    // SAFETY: The mapping points to shared memory containing only atomics and
    // fixed-size storage. Access synchronization is handled via atomic
    // operations in `BufferRegistry`.
    unsafe impl Send for SharedMapping {}
    // SAFETY: See `Send` rationale above.
    unsafe impl Sync for SharedMapping {}

    impl SharedMapping {
        pub(super) fn open(name: &str) -> io::Result<Self> {
            let c_name = CString::new(name)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid shm name"))?;

            // SAFETY: `c_name` is a valid C string and flags/mode are valid.
            let mut fd = unsafe {
                shm_open(
                    c_name.as_ptr(),
                    O_CREAT | O_EXCL | O_RDWR,
                    0o600 as libc::c_uint,
                )
            };
            if fd < 0 {
                let err = io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::EEXIST) {
                    // SAFETY: `c_name` is a valid C string and flags are valid.
                    fd = unsafe { shm_open(c_name.as_ptr(), O_RDWR, 0o600 as libc::c_uint) };
                    if fd < 0 {
                        let open_err = io::Error::last_os_error();
                        return Err(io::Error::new(
                            open_err.kind(),
                            format!("shm_open(existing {name}) failed: {open_err}"),
                        ));
                    }
                } else {
                    return Err(io::Error::new(
                        err.kind(),
                        format!("shm_open(create {name}) failed: {err}"),
                    ));
                }
            }

            let len = std::mem::size_of::<SharedLayout>();
            // Caller holds the lifecycle gate, including on first creation.
            // Zero length is an interrupted shm_open → ftruncate, not a live
            // incompatible layout. Never resize an existing nonzero mapping.
            // SAFETY: zeroed stat is a valid output buffer; fd is open and owned.
            let mut info: stat = unsafe { std::mem::zeroed() };
            if unsafe { fstat(fd, &mut info) } != 0 {
                let error = io::Error::last_os_error();
                // SAFETY: this error path relinquishes the owned shm descriptor.
                unsafe { close(fd) };
                return Err(error);
            }
            if info.st_size == 0 {
                // SAFETY: `fd` is valid and `len` is the mapping size.
                let trunc_result = unsafe { ftruncate(fd, len as off_t) };
                if trunc_result != 0 {
                    let err = io::Error::last_os_error();
                    // SAFETY: `fd` came from `shm_open`.
                    let _ = unsafe { close(fd) };
                    return Err(io::Error::new(
                        err.kind(),
                        format!("ftruncate({name}, {len}) failed: {err}"),
                    ));
                }
            } else if info.st_size != len as off_t {
                // SAFETY: `fd` came from `shm_open`.
                let _ = unsafe { close(fd) };
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "incompatible shared mapping size for {name}: {} != {len}",
                        info.st_size
                    ),
                ));
            }

            // SAFETY: valid fd, shared + read/write mapping, offset 0.
            let raw_ptr = unsafe {
                mmap(
                    std::ptr::null_mut(),
                    len,
                    PROT_READ | PROT_WRITE,
                    MAP_SHARED,
                    fd,
                    0,
                )
            };
            if raw_ptr == MAP_FAILED {
                let err = io::Error::last_os_error();
                // SAFETY: `fd` came from `shm_open`.
                let _ = unsafe { close(fd) };
                return Err(io::Error::new(
                    err.kind(),
                    format!("mmap({name}, {len}) failed: {err}"),
                ));
            }

            let ptr = NonNull::new(raw_ptr as *mut SharedLayout)
                .ok_or_else(|| io::Error::other("mmap returned null"))?;

            Ok(Self { ptr, len, fd })
        }

        pub(super) fn as_ptr(&self) -> *mut SharedLayout {
            self.ptr.as_ptr()
        }
    }

    impl Drop for SharedMapping {
        fn drop(&mut self) {
            // SAFETY: pointer/length were returned by mmap in `open`.
            let _ = unsafe { munmap(self.ptr.as_ptr() as *mut _, self.len) };
            // SAFETY: `fd` came from `shm_open`.
            let _ = unsafe { close(self.fd) };
        }
    }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Memory::{
        CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_ALL_ACCESS, PAGE_READWRITE,
    };

    pub(super) struct SharedMapping {
        ptr: NonNull<SharedLayout>,
        handle: HANDLE,
    }

    // SAFETY: The mapping points to shared memory containing only atomics and
    // fixed-size storage. Access synchronization is handled via atomic
    // operations in `BufferRegistry`.
    unsafe impl Send for SharedMapping {}
    // SAFETY: See `Send` rationale above.
    unsafe impl Sync for SharedMapping {}

    impl SharedMapping {
        pub(super) fn open(name: &str) -> io::Result<Self> {
            let len = std::mem::size_of::<SharedLayout>() as u64;
            let size_high = (len >> 32) as u32;
            let size_low = (len & 0xFFFF_FFFF) as u32;

            let mut wide_name: Vec<u16> = OsStr::new(name).encode_wide().collect();
            wide_name.push(0);

            // SAFETY: arguments follow Win32 contract; INVALID_HANDLE_VALUE
            // requests page-file backed shared memory.
            let handle = unsafe {
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    std::ptr::null_mut(),
                    PAGE_READWRITE,
                    size_high,
                    size_low,
                    wide_name.as_ptr(),
                )
            };
            if handle == 0 {
                return Err(io::Error::from_raw_os_error(unsafe {
                    GetLastError() as i32
                }));
            }

            // SAFETY: `handle` is valid from CreateFileMappingW.
            let raw_ptr = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, len as usize) };
            if raw_ptr.is_null() {
                // SAFETY: `handle` is valid.
                unsafe { CloseHandle(handle) };
                return Err(io::Error::from_raw_os_error(unsafe {
                    GetLastError() as i32
                }));
            }

            let ptr = NonNull::new(raw_ptr as *mut SharedLayout).ok_or_else(|| {
                io::Error::new(io::ErrorKind::Other, "MapViewOfFile returned null")
            })?;

            Ok(Self { ptr, handle })
        }

        pub(super) fn as_ptr(&self) -> *mut SharedLayout {
            self.ptr.as_ptr()
        }
    }

    impl Drop for SharedMapping {
        fn drop(&mut self) {
            // SAFETY: pointer and handle were returned by Win32 APIs.
            unsafe {
                UnmapViewOfFile(self.ptr.as_ptr() as *const _);
                CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    // A fork in another test inherits our lock FDs until exec. Keep immediate
    // final-close assertions separate from subprocess startup. The integration
    // tests exercise genuinely concurrent hosts in distinct processes.
    #[cfg(unix)]
    static PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_buffer_creation() {
        let registry = BufferRegistry::new();
        for i in 0..CHANNEL_COUNT {
            assert_eq!(registry.buffer_len(i), 0);
        }
    }

    #[test]
    fn test_write_sample() {
        let registry = BufferRegistry::new();
        assert!(registry.write_sample(0, 1024));
        assert_eq!(registry.buffer_len(0), 1);
    }

    #[test]
    fn test_read_buffer() {
        let registry = BufferRegistry::new();
        registry.write_sample(0, 100);
        registry.write_sample(0, 200);
        let buffer = registry.read_buffer(0).unwrap();
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer[0], 100);
        assert_eq!(buffer[1], 200);
    }

    #[test]
    fn test_replace_buffer() {
        let registry = BufferRegistry::new();
        let samples = vec![1, 2, 3, 4, 5];
        assert!(registry.replace_buffer(0, samples.clone()));
        assert_eq!(registry.buffer_len(0), 5);
        let read = registry.read_buffer(0).unwrap();
        assert_eq!(read, samples);
    }

    #[test]
    fn test_clear_buffer() {
        let registry = BufferRegistry::new();
        registry.write_sample(1, 500);
        assert!(registry.clear_buffer(1));
        assert_eq!(registry.buffer_len(1), 0);
    }

    #[test]
    fn test_clear_all() {
        let registry = BufferRegistry::new();
        for i in 0..CHANNEL_COUNT {
            registry.write_sample(i, 100 * i as i16);
        }
        registry.clear_all();
        for i in 0..CHANNEL_COUNT {
            assert_eq!(registry.buffer_len(i), 0);
        }
    }

    #[test]
    fn test_invalid_channel() {
        let registry = BufferRegistry::new();
        assert!(!registry.write_sample(4, 100));
        assert_eq!(registry.buffer_len(5), 0);
        assert!(registry.read_buffer(10).is_none());
    }

    #[test]
    fn test_global_singleton() {
        let reg1 = get_global_registry();
        let reg2 = get_global_registry();
        reg1.clear_all();

        reg1.write_sample(0, 999);
        let buffer = reg2.read_buffer(0).unwrap();
        assert_eq!(buffer[0], 999);
    }

    #[test]
    fn test_reset_write_position() {
        let registry = BufferRegistry::new();
        registry.write_sample(2, 10);
        registry.write_sample(2, 20);
        assert!(registry.reset_write_position(2));
        assert_eq!(registry.buffer_len(2), 0);
        assert!(registry.write_sample(2, 30));
        assert_eq!(registry.read_buffer(2).unwrap(), [30]);
    }

    #[cfg(unix)]
    #[test]
    fn shared_backend_initializes_and_recovers_odd_dead_writer() {
        let _isolated = PROCESS_TEST_LOCK.lock().unwrap();
        let key = format!("/uw-unit-{}-{:p}", std::process::id(), &());
        let memory = Arc::new(SharedMemory::open_key(&key).unwrap());
        let registry = Arc::new(BufferRegistry {
            backend: RegistryBackend::Shared(memory.clone()),
            session_id: AtomicU32::new(0),
            lease: Mutex::new(Weak::new()),
        });
        let lease = registry.register_instance().unwrap();
        let token = registry.writer_token();
        let channel = &memory.layout().channels[0];
        channel.length.store(12, Ordering::Release);
        channel.write_position.store(7, Ordering::Release);
        channel.revision.store(3, Ordering::Release);
        channel.writer.store(token, Ordering::Release);

        drop(lease);
        assert_eq!(
            Arc::strong_count(&memory),
            2,
            "recovery worker joined before lease drop returns"
        );
        let lease = registry.register_instance().unwrap();
        assert_ne!(
            registry.writer_token() >> 32,
            token >> 32,
            "new incarnation, same PID"
        );
        assert_eq!(channel.writer.load(Ordering::Acquire), 0);
        assert_eq!(channel.length.load(Ordering::Acquire), 0);
        assert_eq!(channel.write_position.load(Ordering::Acquire), 0);
        assert_eq!(channel.revision.load(Ordering::Acquire) & 1, 0);

        drop(lease);
        drop(registry);
        drop(memory);
        let _ = std::fs::remove_dir_all(format!(
            "/tmp/ultrawave-{}/{}",
            unsafe { libc::geteuid() },
            &key[1..]
        ));
        let name = CString::new(key).unwrap();
        // SAFETY: name is a valid, uniquely owned POSIX shared-memory key.
        assert_eq!(unsafe { libc::shm_unlink(name.as_ptr()) }, 0);
    }

    #[test]
    fn test_instances_share_all_channels() {
        let shared_registry = Arc::new(BufferRegistry::new());
        let record_instance = Arc::clone(&shared_registry);
        let play_instance = Arc::clone(&shared_registry);

        for channel in 0..CHANNEL_COUNT {
            assert!(record_instance.replace_buffer(
                channel,
                vec![
                    (channel as i16) + 1,
                    (channel as i16) + 10,
                    (channel as i16) + 100
                ]
            ));
        }

        for channel in 0..CHANNEL_COUNT {
            let buffer = play_instance.read_buffer(channel).unwrap();
            assert_eq!(
                buffer,
                vec![
                    (channel as i16) + 1,
                    (channel as i16) + 10,
                    (channel as i16) + 100
                ]
            );
        }
    }

    #[test]
    fn test_concurrent_access_across_channels() {
        let shared_registry = Arc::new(BufferRegistry::new());
        let mut workers = Vec::with_capacity(CHANNEL_COUNT);

        for channel in 0..CHANNEL_COUNT {
            let registry = Arc::clone(&shared_registry);
            workers.push(thread::spawn(move || {
                for sample in 0..2048 {
                    assert!(registry.write_sample(channel, sample as i16));
                }
            }));
        }

        for worker in workers {
            worker.join().unwrap();
        }

        for channel in 0..CHANNEL_COUNT {
            let buffer = shared_registry.read_buffer(channel).unwrap();
            assert_eq!(buffer.len(), 2048);
            assert_eq!(buffer[0], 0);
            assert_eq!(buffer[2047], 2047);
        }
    }

    #[cfg(unix)]
    mod initialization {
        use super::*;
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        use std::time::{SystemTime, UNIX_EPOCH};

        struct Namespace(String);
        impl Namespace {
            fn new() -> Self {
                let stamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                Self(format!("/uwi{:x}{:x}", std::process::id(), stamp as u64))
            }
        }
        impl Drop for Namespace {
            fn drop(&mut self) {
                let name = CString::new(self.0.as_str()).unwrap();
                // SAFETY: uniquely owned disposable test namespace, no live children.
                unsafe {
                    libc::shm_unlink(name.as_ptr());
                }
                let _ = std::fs::remove_dir_all(format!(
                    "/tmp/ultrawave-{}/{}",
                    unsafe { libc::geteuid() },
                    &self.0[1..]
                ));
            }
        }

        fn create_empty(key: &str) -> OwnedFd {
            let name = CString::new(key).unwrap();
            // SAFETY: valid name, flags and mode. Exclusive creation ensures
            // this test never mutates another process's shared memory.
            let fd = unsafe {
                libc::shm_open(
                    name.as_ptr(),
                    libc::O_CREAT | libc::O_EXCL | libc::O_RDWR,
                    0o600,
                )
            };
            assert!(fd >= 0, "{}", io::Error::last_os_error());
            unsafe { OwnedFd::from_raw_fd(fd) }
        }

        #[test]
        fn crashed_initializer() {
            let Ok(key) = env::var("ULTRAWAVE_INIT_CRASH_KEY") else {
                return;
            };
            let locks = LockFiles::new(&key).unwrap();
            let _gate = locks.lock("gate", true).unwrap();
            let _empty = create_empty(&key);
            if env::var("ULTRAWAVE_INIT_CRASH_PHASE").unwrap() == "metadata" {
                let mapping = platform::SharedMapping::open(&key).unwrap();
                // SAFETY: valid mapping and exclusive lifecycle gate. Inject
                // a crash partway through initialization, before READY exists.
                let layout = unsafe { &*mapping.as_ptr() };
                layout.init_state.store(INIT_IN_PROGRESS, Ordering::Release);
                layout.channels[1].length.store(45, Ordering::Release);
                layout.channels[1].revision.store(3, Ordering::Release);
                layout.channels[1].writer.store(17, Ordering::Release);
                layout.channels[1]
                    .sample_rate_bits
                    .store(f32::NAN.to_bits(), Ordering::Release);
                std::process::exit(0);
            }
            // No destructors: only the kernel can release the lifecycle gate.
            std::process::exit(0);
        }

        #[test]
        fn crash_during_sizing_or_metadata_initialization_can_be_reopened() {
            let _isolated = PROCESS_TEST_LOCK.lock().unwrap();
            for phase in ["sizing", "metadata"] {
                let namespace = Namespace::new();
                let child = std::process::Command::new(env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "machines::shared::tests::initialization::crashed_initializer",
                    ])
                    .env("ULTRAWAVE_INIT_CRASH_KEY", &namespace.0)
                    .env("ULTRAWAVE_INIT_CRASH_PHASE", phase)
                    .output()
                    .unwrap();
                assert!(
                    child.status.success(),
                    "{}",
                    String::from_utf8_lossy(&child.stdout)
                );
                let memory = SharedMemory::open_key(&namespace.0).unwrap();
                assert_eq!(
                    memory.layout().init_state.load(Ordering::Acquire),
                    INIT_READY
                );
                for channel in &memory.layout().channels {
                    assert_eq!(channel.length.load(Ordering::Acquire), 0);
                    assert_eq!(channel.writer.load(Ordering::Acquire), 0);
                    assert_eq!(channel.revision.load(Ordering::Acquire) & 1, 0);
                    assert_eq!(
                        f32::from_bits(channel.sample_rate_bits.load(Ordering::Acquire)),
                        44100.0
                    );
                }
            }
        }

        #[test]
        fn live_lock_directory_is_excluded_from_tmpfiles_aging() {
            let _isolated = PROCESS_TEST_LOCK.lock().unwrap();
            let namespace = Namespace::new();
            let memory = SharedMemory::open_key(&namespace.0).unwrap();
            let directory = std::fs::File::open(format!(
                "/tmp/ultrawave-{}/{}",
                unsafe { libc::geteuid() },
                &namespace.0[1..]
            ))
            .unwrap();
            assert!(matches!(
                directory.try_lock(),
                Err(std::fs::TryLockError::WouldBlock)
            ));
            drop(memory);
            directory.try_lock().unwrap();
        }

        #[test]
        fn incompatible_mapping_is_rejected_without_resizing_or_reinitializing() {
            for size in [1, std::mem::size_of::<SharedLayout>() + 64] {
                let namespace = Namespace::new();
                let fd = create_empty(&namespace.0);
                // SAFETY: disposable mapping, no other users, valid owned FD.
                assert_eq!(
                    unsafe { libc::ftruncate(fd.as_raw_fd(), size as libc::off_t) },
                    0
                );
                assert_eq!(
                    SharedMemory::open_key(&namespace.0).err().unwrap().kind(),
                    io::ErrorKind::InvalidData
                );
                let mut stat: libc::stat = unsafe { std::mem::zeroed() };
                assert_eq!(unsafe { libc::fstat(fd.as_raw_fd(), &mut stat) }, 0);
                assert_eq!(
                    stat.st_size as usize, size,
                    "must not resize an incompatible mapping"
                );
            }
            let namespace = Namespace::new();
            let memory = SharedMemory::open_key(&namespace.0).unwrap();
            memory.layout().version.store(2, Ordering::Release);
            memory.layout().channels[0]
                .length
                .store(7, Ordering::Release);
            assert_eq!(
                SharedMemory::open_key(&namespace.0).err().unwrap().kind(),
                io::ErrorKind::InvalidData
            );
            assert_eq!(memory.layout().version.load(Ordering::Acquire), 2);
            assert_eq!(
                memory.layout().channels[0].length.load(Ordering::Acquire),
                7
            );
        }
    }
}
