use std::env;
use std::ffi::CString;
use std::io;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicI16, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

pub const CHANNEL_COUNT: usize = 4;

/// Maximum recording length in seconds.
const MAX_RECORD_SECONDS: f32 = 10.0;

/// Sample rate used for buffer allocation (44.1kHz standard).
const DEFAULT_SAMPLE_RATE: f32 = 44100.0;

/// Number of samples allocated per buffer (10s @ 44.1kHz).
pub const MAX_BUFFER_SAMPLES: usize = (DEFAULT_SAMPLE_RATE * MAX_RECORD_SECONDS) as usize;

const SHARED_MAGIC: u32 = 0x5557_524D; // "UWRM"
const SHARED_VERSION: u32 = 1;
const INIT_UNINITIALIZED: u32 = 0;
const INIT_IN_PROGRESS: u32 = 1;
const INIT_READY: u32 = 2;

#[cfg(unix)]
const DEFAULT_SHM_KEY: &str = "/ultrawave_global_buffers_v1";
#[cfg(windows)]
const DEFAULT_SHM_KEY: &str = "Local\\UltrawaveGlobalBuffersV1";

#[repr(C, align(64))]
struct SharedChannelLayout {
    write_position: AtomicUsize,
    length: AtomicUsize,
    samples: [AtomicI16; MAX_BUFFER_SAMPLES],
}

#[repr(C)]
struct SharedLayout {
    magic: AtomicU32,
    version: AtomicU32,
    init_state: AtomicU32,
    _reserved: AtomicU32,
    active_instances: AtomicUsize,
    channels: [SharedChannelLayout; CHANNEL_COUNT],
}

struct SharedMemory {
    mapping: platform::SharedMapping,
}

impl SharedMemory {
    fn open() -> io::Result<Self> {
        let key = shared_memory_key();
        let mapping = platform::SharedMapping::open(&key)?;
        let memory = Self { mapping };
        memory.ensure_initialized();
        Ok(memory)
    }

    fn layout(&self) -> &SharedLayout {
        // SAFETY: The mapping is created for exactly `SharedLayout` bytes and
        // lives as long as `self`.
        unsafe { &*self.mapping.as_ptr() }
    }

    fn ensure_initialized(&self) {
        let layout = self.layout();

        loop {
            let state = layout.init_state.load(Ordering::Acquire);
            if state == INIT_READY {
                let magic = layout.magic.load(Ordering::Acquire);
                let version = layout.version.load(Ordering::Acquire);
                if magic == SHARED_MAGIC && version == SHARED_VERSION {
                    return;
                }
                if layout
                    .init_state
                    .compare_exchange(
                        INIT_READY,
                        INIT_IN_PROGRESS,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    self.initialize_layout();
                    return;
                }
                continue;
            }

            if state == INIT_UNINITIALIZED {
                if layout
                    .init_state
                    .compare_exchange(
                        INIT_UNINITIALIZED,
                        INIT_IN_PROGRESS,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    self.initialize_layout();
                    return;
                }
                continue;
            }

            std::thread::yield_now();
        }
    }

    fn initialize_layout(&self) {
        let layout = self.layout();
        for channel in &layout.channels {
            channel.length.store(0, Ordering::Release);
            channel.write_position.store(0, Ordering::Release);
            for sample in &channel.samples {
                sample.store(0, Ordering::Release);
            }
        }
        layout.active_instances.store(0, Ordering::Release);
        layout.magic.store(SHARED_MAGIC, Ordering::Release);
        layout.version.store(SHARED_VERSION, Ordering::Release);
        layout.init_state.store(INIT_READY, Ordering::Release);
    }
}

/// Local lock-free buffer implementation used for explicit `BufferRegistry::new()`.
/// The plugin/global path uses `SharedMemory`.
struct LocalBuffer {
    samples: Box<[AtomicI16]>,
    write_position: AtomicUsize,
    length: AtomicUsize,
    max_length: usize,
}

impl LocalBuffer {
    fn new() -> Self {
        let samples = std::iter::repeat_with(|| AtomicI16::new(0))
            .take(MAX_BUFFER_SAMPLES)
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            samples,
            write_position: AtomicUsize::new(0),
            length: AtomicUsize::new(0),
            max_length: MAX_BUFFER_SAMPLES,
        }
    }

    fn clear(&self) {
        self.length.store(0, Ordering::Release);
        self.write_position.store(0, Ordering::Release);
    }

    fn len(&self) -> usize {
        self.length.load(Ordering::Acquire)
    }

    fn write_sample(&self, sample: i16) -> bool {
        let position = loop {
            let current = self.write_position.load(Ordering::Acquire);
            if current >= self.max_length {
                return false;
            }

            if self
                .write_position
                .compare_exchange(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break current;
            }
        };

        self.samples[position].store(sample, Ordering::Release);
        self.length.fetch_max(position + 1, Ordering::AcqRel);
        true
    }

    fn read_buffer(&self) -> Vec<i16> {
        let len = self.len();
        let mut out = Vec::with_capacity(len);
        for sample in self.samples.iter().take(len) {
            out.push(sample.load(Ordering::Acquire));
        }
        out
    }

    fn replace_buffer(&self, samples: &[i16]) {
        let len = samples.len().min(self.max_length);
        for (idx, sample) in samples.iter().take(len).enumerate() {
            self.samples[idx].store(*sample, Ordering::Release);
        }
        self.length.store(len, Ordering::Release);
        self.write_position.store(len, Ordering::Release);
    }

    fn reset_write_position(&self) {
        self.write_position.store(0, Ordering::Release);
    }
}

struct LocalRegistry {
    buffers: [LocalBuffer; CHANNEL_COUNT],
    active_instances: AtomicUsize,
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
            active_instances: AtomicUsize::new(0),
        }
    }
}

enum RegistryBackend {
    Local(LocalRegistry),
    Shared(SharedMemory),
}

/// Global registry for shared buffers R1-R4.
pub struct BufferRegistry {
    backend: RegistryBackend,
}

#[allow(dead_code)]
impl BufferRegistry {
    /// Creates an isolated local registry (primarily for tests and offline flows).
    pub fn new() -> Self {
        Self {
            backend: RegistryBackend::Local(LocalRegistry::new()),
        }
    }

    fn new_shared() -> io::Result<Self> {
        Ok(Self {
            backend: RegistryBackend::Shared(SharedMemory::open()?),
        })
    }

    fn new_global() -> Self {
        #[cfg(test)]
        {
            return Self::new();
        }
        #[cfg(not(test))]
        {
            match Self::new_shared() {
                Ok(registry) => registry,
                Err(err) => {
                    eprintln!("Ultrawave shared memory unavailable, falling back to local RAM: {err}");
                    Self::new()
                }
            }
        }
    }

    fn shared_channel(layout: &SharedLayout, channel: usize) -> Option<&SharedChannelLayout> {
        layout.channels.get(channel)
    }

    fn shared_len(channel: &SharedChannelLayout) -> usize {
        channel.length.load(Ordering::Acquire)
    }

    fn shared_read_buffer(channel: &SharedChannelLayout) -> Vec<i16> {
        let len = Self::shared_len(channel);
        let mut out = Vec::with_capacity(len);
        for sample in channel.samples.iter().take(len) {
            out.push(sample.load(Ordering::Acquire));
        }
        out
    }

    fn shared_write_sample(channel: &SharedChannelLayout, sample: i16) -> bool {
        let position = loop {
            let current = channel.write_position.load(Ordering::Acquire);
            if current >= MAX_BUFFER_SAMPLES {
                return false;
            }
            if channel
                .write_position
                .compare_exchange(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break current;
            }
        };

        channel.samples[position].store(sample, Ordering::Release);
        channel.length.fetch_max(position + 1, Ordering::AcqRel);
        true
    }

    fn shared_replace_buffer(channel: &SharedChannelLayout, samples: &[i16]) {
        let len = samples.len().min(MAX_BUFFER_SAMPLES);
        for (idx, sample) in samples.iter().take(len).enumerate() {
            channel.samples[idx].store(*sample, Ordering::Release);
        }
        channel.length.store(len, Ordering::Release);
        channel.write_position.store(len, Ordering::Release);
    }

    fn shared_clear_buffer(channel: &SharedChannelLayout) {
        channel.length.store(0, Ordering::Release);
        channel.write_position.store(0, Ordering::Release);
    }

    fn register_instance(&self) {
        match &self.backend {
            RegistryBackend::Local(local) => {
                let previous = local.active_instances.fetch_add(1, Ordering::AcqRel);
                if previous == 0 {
                    self.clear_all();
                }
            }
            RegistryBackend::Shared(shared) => {
                let previous = shared
                    .layout()
                    .active_instances
                    .fetch_add(1, Ordering::AcqRel);
                if previous == 0 {
                    self.clear_all();
                }
            }
        }
    }

    fn unregister_instance(&self) {
        match &self.backend {
            RegistryBackend::Local(local) => {
                let _ = local
                    .active_instances
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                        count.checked_sub(1)
                    });
            }
            RegistryBackend::Shared(shared) => {
                let _ = shared
                    .layout()
                    .active_instances
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                        count.checked_sub(1)
                    });
            }
        }
    }

    /// Get a clone of the buffer data for reading.
    pub fn read_buffer(&self, channel: usize) -> Option<Vec<i16>> {
        match &self.backend {
            RegistryBackend::Local(local) => local.buffers.get(channel).map(LocalBuffer::read_buffer),
            RegistryBackend::Shared(shared) => {
                let layout = shared.layout();
                Self::shared_channel(layout, channel).map(Self::shared_read_buffer)
            }
        }
    }

    /// Write a sample to the buffer.
    pub fn write_sample(&self, channel: usize, sample: i16) -> bool {
        match &self.backend {
            RegistryBackend::Local(local) => local
                .buffers
                .get(channel)
                .is_some_and(|buffer| buffer.write_sample(sample)),
            RegistryBackend::Shared(shared) => {
                let layout = shared.layout();
                Self::shared_channel(layout, channel)
                    .is_some_and(|ch| Self::shared_write_sample(ch, sample))
            }
        }
    }

    /// Replace entire buffer contents.
    pub fn replace_buffer(&self, channel: usize, samples: Vec<i16>) -> bool {
        match &self.backend {
            RegistryBackend::Local(local) => {
                if let Some(buffer) = local.buffers.get(channel) {
                    buffer.replace_buffer(&samples);
                    true
                } else {
                    false
                }
            }
            RegistryBackend::Shared(shared) => {
                let layout = shared.layout();
                if let Some(ch) = Self::shared_channel(layout, channel) {
                    Self::shared_replace_buffer(ch, &samples);
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Get current buffer length.
    pub fn buffer_len(&self, channel: usize) -> usize {
        match &self.backend {
            RegistryBackend::Local(local) => local.buffers.get(channel).map_or(0, LocalBuffer::len),
            RegistryBackend::Shared(shared) => {
                let layout = shared.layout();
                Self::shared_channel(layout, channel).map_or(0, Self::shared_len)
            }
        }
    }

    /// Clear a specific buffer.
    pub fn clear_buffer(&self, channel: usize) -> bool {
        match &self.backend {
            RegistryBackend::Local(local) => {
                if let Some(buffer) = local.buffers.get(channel) {
                    buffer.clear();
                    true
                } else {
                    false
                }
            }
            RegistryBackend::Shared(shared) => {
                let layout = shared.layout();
                if let Some(ch) = Self::shared_channel(layout, channel) {
                    Self::shared_clear_buffer(ch);
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Clear all buffers.
    pub fn clear_all(&self) {
        match &self.backend {
            RegistryBackend::Local(local) => {
                for buffer in &local.buffers {
                    buffer.clear();
                }
            }
            RegistryBackend::Shared(shared) => {
                let layout = shared.layout();
                for channel in &layout.channels {
                    Self::shared_clear_buffer(channel);
                }
            }
        }
    }

    /// Reset write position for recording (without clearing data).
    pub fn reset_write_position(&self, channel: usize) -> bool {
        match &self.backend {
            RegistryBackend::Local(local) => {
                if let Some(buffer) = local.buffers.get(channel) {
                    buffer.reset_write_position();
                    true
                } else {
                    false
                }
            }
            RegistryBackend::Shared(shared) => {
                let layout = shared.layout();
                if let Some(ch) = Self::shared_channel(layout, channel) {
                    ch.write_position.store(0, Ordering::Release);
                    true
                } else {
                    false
                }
            }
        }
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

/// Global singleton instance.
#[allow(dead_code)]
static GLOBAL_REGISTRY: OnceLock<Arc<BufferRegistry>> = OnceLock::new();
static RECORD_MIDI_NOTE_ACTIVE: AtomicBool = AtomicBool::new(false);
static PLAY_MIDI_NOTE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Get reference to the global buffer registry.
#[allow(dead_code)]
pub fn get_global_registry() -> Arc<BufferRegistry> {
    GLOBAL_REGISTRY
        .get_or_init(|| Arc::new(BufferRegistry::new_global()))
        .clone()
}

/// Register a plugin instance lifecycle.
///
/// When the first instance is created after a full teardown, RAM buffers are
/// cleared to guarantee volatile/non-persistent behavior across host reloads.
pub fn register_plugin_instance() {
    get_global_registry().register_instance();
}

/// Unregister a plugin instance lifecycle.
pub fn unregister_plugin_instance() {
    get_global_registry().unregister_instance();
}

pub fn set_record_midi_note_active(active: bool) {
    RECORD_MIDI_NOTE_ACTIVE.store(active, Ordering::Release);
}

pub fn record_midi_note_active() -> bool {
    RECORD_MIDI_NOTE_ACTIVE.load(Ordering::Acquire)
}

pub fn set_play_midi_note_active(active: bool) {
    PLAY_MIDI_NOTE_ACTIVE.store(active, Ordering::Release);
}

pub fn play_midi_note_active() -> bool {
    PLAY_MIDI_NOTE_ACTIVE.load(Ordering::Acquire)
}

#[cfg(unix)]
mod platform {
    use super::*;
    use libc::{
        close, fstat, ftruncate, mmap, munmap, off_t, shm_open, stat, MAP_FAILED,
        MAP_SHARED, O_CREAT, O_EXCL, O_RDWR, PROT_READ, PROT_WRITE,
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
            let c_name =
                CString::new(name).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid shm name"))?;

            // SAFETY: `c_name` is a valid C string and flags/mode are valid.
            let mut fd = unsafe { shm_open(c_name.as_ptr(), O_CREAT | O_EXCL | O_RDWR, 0o600 as libc::mode_t) };
            let created = if fd >= 0 {
                true
            } else {
                let err = io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::EEXIST) {
                    // SAFETY: `c_name` is a valid C string and flags are valid.
                    fd = unsafe { shm_open(c_name.as_ptr(), O_RDWR, 0o600 as libc::mode_t) };
                    if fd < 0 {
                        let open_err = io::Error::last_os_error();
                        return Err(io::Error::new(
                            open_err.kind(),
                            format!("shm_open(existing {name}) failed: {open_err}"),
                        ));
                    }
                    false
                } else {
                    return Err(io::Error::new(
                        err.kind(),
                        format!("shm_open(create {name}) failed: {err}"),
                    ));
                }
            };

            let len = std::mem::size_of::<SharedLayout>();

            if created {
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
            } else {
                // SAFETY: zeroed struct is valid as out-param for fstat.
                let mut info: stat = unsafe { std::mem::zeroed() };
                // SAFETY: `fd` is valid and `info` points to writable memory.
                if unsafe { fstat(fd, &mut info as *mut stat) } != 0 {
                    let err = io::Error::last_os_error();
                    // SAFETY: `fd` came from `shm_open`.
                    let _ = unsafe { close(fd) };
                    return Err(io::Error::new(
                        err.kind(),
                        format!("fstat({name}) failed: {err}"),
                    ));
                }
                if info.st_size < len as off_t {
                    // SAFETY: `fd` came from `shm_open`.
                    let _ = unsafe { close(fd) };
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "shared mapping too small for {name}: {} < {}",
                            info.st_size, len
                        ),
                    ));
                }
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
                .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "mmap returned null"))?;

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
                return Err(io::Error::from_raw_os_error(unsafe { GetLastError() as i32 }));
            }

            // SAFETY: `handle` is valid from CreateFileMappingW.
            let raw_ptr = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, len as usize) };
            if raw_ptr.is_null() {
                // SAFETY: `handle` is valid.
                unsafe { CloseHandle(handle) };
                return Err(io::Error::from_raw_os_error(unsafe { GetLastError() as i32 }));
            }

            let ptr = NonNull::new(raw_ptr as *mut SharedLayout)
                .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "MapViewOfFile returned null"))?;

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
        assert_eq!(registry.buffer_len(2), 2);
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
}
