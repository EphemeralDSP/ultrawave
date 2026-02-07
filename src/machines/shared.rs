use std::sync::atomic::{AtomicI16, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

const CHANNEL_COUNT: usize = 4;

/// Maximum recording length in seconds.
const MAX_RECORD_SECONDS: f32 = 10.0;

/// Sample rate used for buffer allocation (44.1kHz standard).
const DEFAULT_SAMPLE_RATE: f32 = 44100.0;

/// Lock-free buffer for a single channel (R1-R4).
pub struct SharedBuffer {
    /// Fixed storage to avoid allocation/locking on the audio thread.
    samples: Box<[AtomicI16]>,
    /// Current write position for append/overwrite operations.
    write_position: AtomicUsize,
    /// Number of valid samples currently visible to readers.
    length: AtomicUsize,
    /// Maximum addressable length in samples.
    max_length: usize,
}

impl SharedBuffer {
    fn new() -> Self {
        let max_length = (DEFAULT_SAMPLE_RATE * MAX_RECORD_SECONDS) as usize;
        let samples = std::iter::repeat_with(|| AtomicI16::new(0))
            .take(max_length)
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            samples,
            write_position: AtomicUsize::new(0),
            length: AtomicUsize::new(0),
            max_length,
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

/// Global registry for shared buffers R1-R4.
pub struct BufferRegistry {
    /// The four shared buffers (R1, R2, R3, R4).
    buffers: [SharedBuffer; CHANNEL_COUNT],
}

#[allow(dead_code)]
impl BufferRegistry {
    fn new() -> Self {
        Self {
            buffers: [
                SharedBuffer::new(),
                SharedBuffer::new(),
                SharedBuffer::new(),
                SharedBuffer::new(),
            ],
        }
    }

    /// Get a clone of the buffer data for reading.
    pub fn read_buffer(&self, channel: usize) -> Option<Vec<i16>> {
        self.buffers.get(channel).map(SharedBuffer::read_buffer)
    }

    /// Write a sample to the buffer.
    pub fn write_sample(&self, channel: usize, sample: i16) -> bool {
        self.buffers
            .get(channel)
            .is_some_and(|buffer| buffer.write_sample(sample))
    }

    /// Replace entire buffer contents.
    pub fn replace_buffer(&self, channel: usize, samples: Vec<i16>) -> bool {
        if let Some(buffer) = self.buffers.get(channel) {
            buffer.replace_buffer(&samples);
            true
        } else {
            false
        }
    }

    /// Get current buffer length.
    pub fn buffer_len(&self, channel: usize) -> usize {
        self.buffers.get(channel).map_or(0, SharedBuffer::len)
    }

    /// Clear a specific buffer.
    pub fn clear_buffer(&self, channel: usize) -> bool {
        if let Some(buffer) = self.buffers.get(channel) {
            buffer.clear();
            true
        } else {
            false
        }
    }

    /// Clear all buffers.
    pub fn clear_all(&self) {
        for buffer in &self.buffers {
            buffer.clear();
        }
    }

    /// Reset write position for recording (without clearing data).
    pub fn reset_write_position(&self, channel: usize) -> bool {
        if let Some(buffer) = self.buffers.get(channel) {
            buffer.reset_write_position();
            true
        } else {
            false
        }
    }
}

/// Global singleton instance.
#[allow(dead_code)]
static GLOBAL_REGISTRY: OnceLock<Arc<BufferRegistry>> = OnceLock::new();

/// Get reference to the global buffer registry.
#[allow(dead_code)]
pub fn get_global_registry() -> Arc<BufferRegistry> {
    GLOBAL_REGISTRY
        .get_or_init(|| Arc::new(BufferRegistry::new()))
        .clone()
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
