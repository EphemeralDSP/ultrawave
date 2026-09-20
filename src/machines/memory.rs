use std::sync::atomic::{AtomicI16, AtomicU32, AtomicU64, AtomicUsize, Ordering};

pub const CHANNEL_COUNT: usize = 4;

const MAX_RECORD_SECONDS: f32 = 10.0;
const DEFAULT_SAMPLE_RATE: f32 = 44_100.0;

/// Number of samples allocated per buffer (10 seconds at 44.1 kHz).
pub const MAX_BUFFER_SAMPLES: usize = (DEFAULT_SAMPLE_RATE * MAX_RECORD_SECONDS) as usize;

#[repr(C, align(64))]
struct SampleBuffer {
    write_position: AtomicUsize,
    length: AtomicUsize,
    sample_rate_bits: AtomicU32,
    revision: AtomicU64,
    samples: [AtomicI16; MAX_BUFFER_SAMPLES],
}

struct HeapBuffer(Box<SampleBuffer>);

impl HeapBuffer {
    fn new() -> Self {
        // SAFETY: All fields are integer atomics, for which the all-zero bit
        // pattern is valid. Allocate directly on the heap so this large value
        // is never materialized on the thread stack.
        let buffer = unsafe { Box::<SampleBuffer>::new_zeroed().assume_init() };
        buffer
            .sample_rate_bits
            .store(DEFAULT_SAMPLE_RATE.to_bits(), Ordering::Relaxed);
        Self(buffer)
    }
}

/// Fixed-capacity sample RAM owned by one effect instance.
///
/// Audio and UI readers use atomic metadata snapshots and never lock or copy on
/// trigger. Mutations are serialized by the owning effect's control/audio path.
pub struct SampleMemory {
    buffers: [HeapBuffer; CHANNEL_COUNT],
}

impl SampleMemory {
    pub fn new() -> Self {
        Self {
            buffers: std::array::from_fn(|_| HeapBuffer::new()),
        }
    }

    fn buffer(&self, channel: usize) -> Option<&SampleBuffer> {
        self.buffers.get(channel).map(|buffer| &*buffer.0)
    }

    fn begin_mutation(buffer: &SampleBuffer) {
        buffer.revision.fetch_add(1, Ordering::AcqRel);
    }

    fn end_mutation(buffer: &SampleBuffer) {
        buffer.revision.fetch_add(1, Ordering::Release);
    }

    /// Get a consistent copy of a buffer for non-real-time inspection.
    pub fn read_buffer(&self, channel: usize) -> Option<Vec<i16>> {
        let (len, _, revision) = self.snapshot(channel)?;
        let buffer = self.buffer(channel)?;
        let samples = buffer.samples[..len]
            .iter()
            .map(|sample| sample.load(Ordering::Acquire))
            .collect();
        (buffer.revision.load(Ordering::Acquire) == revision).then_some(samples)
    }

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
        let Some(buffer) = self.buffer(channel) else {
            return false;
        };
        Self::begin_mutation(buffer);
        let len = samples.len().min(MAX_BUFFER_SAMPLES);
        for (index, sample) in samples.into_iter().take(len).enumerate() {
            buffer.samples[index].store(sample, Ordering::Release);
        }
        buffer.length.store(len, Ordering::Release);
        buffer.write_position.store(len, Ordering::Release);
        buffer
            .sample_rate_bits
            .store(sample_rate.to_bits(), Ordering::Release);
        Self::end_mutation(buffer);
        true
    }

    pub fn buffer_len(&self, channel: usize) -> usize {
        self.buffer(channel)
            .map_or(0, |buffer| buffer.length.load(Ordering::Acquire))
    }

    pub fn clear_buffer(&self, channel: usize) -> bool {
        let Some(buffer) = self.buffer(channel) else {
            return false;
        };
        Self::begin_mutation(buffer);
        buffer.length.store(0, Ordering::Release);
        buffer.write_position.store(0, Ordering::Release);
        Self::end_mutation(buffer);
        true
    }

    pub fn clear_all(&self) {
        for channel in 0..CHANNEL_COUNT {
            self.clear_buffer(channel);
        }
    }

    pub(crate) fn begin_recording(&self, channel: usize, sample_rate: f32) -> bool {
        if !sample_rate.is_finite() || sample_rate <= 0.0 {
            return false;
        }
        let Some(buffer) = self.buffer(channel) else {
            return false;
        };
        Self::begin_mutation(buffer);
        buffer.write_position.store(0, Ordering::Relaxed);
        buffer.length.store(0, Ordering::Release);
        buffer
            .sample_rate_bits
            .store(sample_rate.to_bits(), Ordering::Release);
        Self::end_mutation(buffer);
        true
    }

    pub(crate) fn write_sample(&self, channel: usize, sample: i16) -> bool {
        let Some(buffer) = self.buffer(channel) else {
            return false;
        };
        let position = buffer.write_position.load(Ordering::Relaxed);
        if position >= MAX_BUFFER_SAMPLES {
            return false;
        }
        buffer.samples[position].store(sample, Ordering::Release);
        buffer.write_position.store(position + 1, Ordering::Relaxed);
        buffer.length.store(position + 1, Ordering::Release);
        true
    }

    /// O(1) consistent metadata snapshot for real-time readers.
    pub fn snapshot(&self, channel: usize) -> Option<(usize, f32, u64)> {
        let buffer = self.buffer(channel)?;
        let revision = buffer.revision.load(Ordering::Acquire);
        if revision & 1 != 0 {
            return None;
        }
        let len = buffer
            .length
            .load(Ordering::Acquire)
            .min(MAX_BUFFER_SAMPLES);
        let rate = f32::from_bits(buffer.sample_rate_bits.load(Ordering::Acquire));
        (buffer.revision.load(Ordering::Acquire) == revision).then_some((len, rate, revision))
    }

    #[inline]
    pub fn read_sample_pair(
        &self,
        channel: usize,
        index: usize,
        revision: u64,
        latched_len: usize,
    ) -> Option<(i16, i16)> {
        let buffer = self.buffer(channel)?;
        if buffer.revision.load(Ordering::Acquire) != revision {
            return None;
        }
        let len = buffer.length.load(Ordering::Acquire).min(latched_len);
        if index >= len {
            return None;
        }
        let a = buffer.samples[index].load(Ordering::Acquire);
        let b = buffer.samples[(index + 1).min(len - 1)].load(Ordering::Acquire);
        (buffer.revision.load(Ordering::Acquire) == revision).then_some((a, b))
    }
}

impl Default for SampleMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_invalidates_latched_revision() {
        let memory = SampleMemory::new();
        assert!(memory.replace_buffer(0, vec![1, 2, 3]));
        let (len, _, revision) = memory.snapshot(0).unwrap();
        assert_eq!(memory.read_sample_pair(0, 1, revision, len), Some((2, 3)));

        assert!(memory.replace_buffer(0, vec![4, 5, 6]));
        assert_eq!(memory.read_sample_pair(0, 1, revision, len), None);
    }
}
