use std::sync::{Arc, Mutex, OnceLock};

/// Maximum recording length in seconds
const MAX_RECORD_SECONDS: f32 = 10.0;

/// Sample rate used for buffer allocation (44.1kHz standard)
const DEFAULT_SAMPLE_RATE: f32 = 44100.0;

/// Shared buffer for a single channel (R1-R4)
#[derive(Clone)]
pub struct SharedBuffer {
    /// 12-bit audio samples stored as i16
    pub samples: Vec<i16>,
    /// Current write position (for recording)
    pub write_position: usize,
    /// Maximum length in samples
    pub max_length: usize,
}

impl SharedBuffer {
    fn new() -> Self {
        let max_length = (DEFAULT_SAMPLE_RATE * MAX_RECORD_SECONDS) as usize;
        Self {
            samples: Vec::with_capacity(max_length),
            write_position: 0,
            max_length,
        }
    }

    /// Clear buffer and reset write position
    pub fn clear(&mut self) {
        self.samples.clear();
        self.write_position = 0;
    }

    /// Get current buffer length
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// Global registry for shared buffers R1-R4
pub struct BufferRegistry {
    /// The four shared buffers (R1, R2, R3, R4)
    buffers: [Mutex<SharedBuffer>; 4],
}

impl BufferRegistry {
    fn new() -> Self {
        Self {
            buffers: [
                Mutex::new(SharedBuffer::new()),
                Mutex::new(SharedBuffer::new()),
                Mutex::new(SharedBuffer::new()),
                Mutex::new(SharedBuffer::new()),
            ],
        }
    }

    /// Get a clone of the buffer data for reading
    /// Returns None if the channel is invalid or if lock fails
    pub fn read_buffer(&self, channel: usize) -> Option<Vec<i16>> {
        if channel >= 4 {
            return None;
        }
        self.buffers[channel]
            .lock()
            .ok()
            .map(|buf| buf.samples.clone())
    }

    /// Write a sample to the buffer
    /// Returns true if successful, false if channel is invalid or lock fails
    pub fn write_sample(&self, channel: usize, sample: i16) -> bool {
        if channel >= 4 {
            return false;
        }
        if let Ok(mut buf) = self.buffers[channel].lock() {
            let write_pos = buf.write_position;
            if write_pos < buf.samples.len() {
                buf.samples[write_pos] = sample;
            } else if buf.samples.len() < buf.max_length {
                buf.samples.push(sample);
            } else {
                // Buffer is full
                return false;
            }
            buf.write_position += 1;
            true
        } else {
            false
        }
    }

    /// Replace entire buffer contents
    /// Returns true if successful
    pub fn replace_buffer(&self, channel: usize, samples: Vec<i16>) -> bool {
        if channel >= 4 {
            return false;
        }
        if let Ok(mut buf) = self.buffers[channel].lock() {
            buf.samples = samples;
            buf.write_position = buf.samples.len();
            true
        } else {
            false
        }
    }

    /// Get current buffer length
    pub fn buffer_len(&self, channel: usize) -> usize {
        if channel >= 4 {
            return 0;
        }
        self.buffers[channel]
            .lock()
            .ok()
            .map(|buf| buf.len())
            .unwrap_or(0)
    }

    /// Clear a specific buffer
    pub fn clear_buffer(&self, channel: usize) -> bool {
        if channel >= 4 {
            return false;
        }
        if let Ok(mut buf) = self.buffers[channel].lock() {
            buf.clear();
            true
        } else {
            false
        }
    }

    /// Clear all buffers
    pub fn clear_all(&self) {
        for i in 0..4 {
            let _ = self.clear_buffer(i);
        }
    }

    /// Reset write position for recording (without clearing data)
    pub fn reset_write_position(&self, channel: usize) -> bool {
        if channel >= 4 {
            return false;
        }
        if let Ok(mut buf) = self.buffers[channel].lock() {
            buf.write_position = 0;
            true
        } else {
            false
        }
    }
}

/// Global singleton instance
static GLOBAL_REGISTRY: OnceLock<Arc<BufferRegistry>> = OnceLock::new();

/// Get reference to the global buffer registry
pub fn get_global_registry() -> Arc<BufferRegistry> {
    GLOBAL_REGISTRY
        .get_or_init(|| Arc::new(BufferRegistry::new()))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_creation() {
        let registry = BufferRegistry::new();
        for i in 0..4 {
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
        for i in 0..4 {
            registry.write_sample(i, 100 * i as i16);
        }
        registry.clear_all();
        for i in 0..4 {
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

        // Write with first reference
        reg1.write_sample(0, 999);

        // Read with second reference - should see same data
        let buffer = reg2.read_buffer(0).unwrap();
        assert_eq!(buffer[0], 999);
    }

    #[test]
    fn test_reset_write_position() {
        let registry = BufferRegistry::new();
        registry.write_sample(2, 10);
        registry.write_sample(2, 20);
        assert!(registry.reset_write_position(2));
        // Buffer still has data, but write position is reset
        assert_eq!(registry.buffer_len(2), 2);
    }
}
