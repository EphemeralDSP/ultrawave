use std::sync::atomic::{AtomicBool, AtomicI16, AtomicU32, AtomicU64, AtomicUsize, Ordering};

const MAX_RECORD_SECONDS: f32 = 10.0;
const DEFAULT_SAMPLE_RATE: f32 = 44_100.0;

/// Maximum sample length (10 seconds at 44.1 kHz).
pub const MAX_BUFFER_SAMPLES: usize = (DEFAULT_SAMPLE_RATE * MAX_RECORD_SECONDS) as usize;

#[repr(C, align(64))]
struct SampleBuffer {
    write_position: AtomicUsize,
    length: AtomicUsize,
    sample_rate_bits: AtomicU32,
    samples: [AtomicI16; MAX_BUFFER_SAMPLES],
}

/// Fixed-capacity sample RAM owned by one effect instance.
///
/// Audio and UI readers use atomic metadata snapshots and never lock or copy on
/// trigger. Mutations are serialized by the owning effect's control/audio path.
pub struct SampleMemory {
    // One published take plus staging storage for non-destructive resampling.
    buffers: [Box<SampleBuffer>; 2],
    active: AtomicUsize,
    staging: AtomicBool,
    revision: AtomicU64,
}

impl SampleMemory {
    pub fn new() -> Self {
        let buffers = std::array::from_fn(|_| {
            // SAFETY: All fields are integer atomics, for which the all-zero bit
            // pattern is valid. Allocate directly on the heap so this large value
            // is never materialized on the thread stack.
            let buffer = unsafe { Box::<SampleBuffer>::new_zeroed().assume_init() };
            buffer
                .sample_rate_bits
                .store(DEFAULT_SAMPLE_RATE.to_bits(), Ordering::Relaxed);
            buffer
        });
        Self {
            buffers,
            active: AtomicUsize::new(0),
            staging: AtomicBool::new(false),
            revision: AtomicU64::new(0),
        }
    }

    fn current(&self) -> &SampleBuffer {
        &self.buffers[self.active.load(Ordering::Acquire)]
    }

    fn begin_mutation(&self) {
        self.revision.fetch_add(1, Ordering::AcqRel);
    }

    fn end_mutation(&self) {
        self.revision.fetch_add(1, Ordering::Release);
    }

    /// Get a consistent copy of the sample for non-real-time inspection.
    pub fn read_buffer(&self) -> Option<Vec<i16>> {
        let (len, _, revision) = self.snapshot()?;
        let samples = self.current().samples[..len]
            .iter()
            .map(|sample| sample.load(Ordering::Acquire))
            .collect();
        (self.revision.load(Ordering::Acquire) == revision).then_some(samples)
    }

    pub fn replace_buffer(&self, samples: Vec<i16>) -> bool {
        self.replace_buffer_at_rate(samples, DEFAULT_SAMPLE_RATE)
    }

    pub fn replace_buffer_at_rate(&self, samples: Vec<i16>, sample_rate: f32) -> bool {
        if !sample_rate.is_finite() || sample_rate <= 0.0 {
            return false;
        }
        self.begin_mutation();
        self.staging.store(false, Ordering::Relaxed);
        let buffer = self.current();
        let len = samples.len().min(MAX_BUFFER_SAMPLES);
        for (index, sample) in samples.into_iter().take(len).enumerate() {
            buffer.samples[index].store(sample, Ordering::Release);
        }
        buffer.length.store(len, Ordering::Release);
        buffer.write_position.store(len, Ordering::Release);
        buffer
            .sample_rate_bits
            .store(sample_rate.to_bits(), Ordering::Release);
        self.end_mutation();
        true
    }

    pub fn buffer_len(&self) -> usize {
        self.current().length.load(Ordering::Acquire)
    }

    pub fn clear_buffer(&self) -> bool {
        self.begin_mutation();
        self.staging.store(false, Ordering::Relaxed);
        self.current().length.store(0, Ordering::Release);
        self.current().write_position.store(0, Ordering::Release);
        self.end_mutation();
        true
    }

    pub(crate) fn begin_recording(&self, sample_rate: f32, staging: bool) -> bool {
        if !sample_rate.is_finite() || sample_rate <= 0.0 {
            return false;
        }
        if !staging {
            self.begin_mutation();
        }
        self.staging.store(staging, Ordering::Relaxed);
        let index = self.active.load(Ordering::Relaxed) ^ usize::from(staging);
        let buffer = &self.buffers[index];
        buffer.write_position.store(0, Ordering::Relaxed);
        buffer.length.store(0, Ordering::Release);
        buffer
            .sample_rate_bits
            .store(sample_rate.to_bits(), Ordering::Release);
        if !staging {
            self.end_mutation();
        }
        true
    }

    /// Publish a nonempty staged take in O(1); old voices are invalidated here,
    /// never at the start of resampling. An empty attempt retains the old take.
    pub(crate) fn finish_recording(&self) {
        if self.staging.swap(false, Ordering::Relaxed) {
            let next = self.active.load(Ordering::Relaxed) ^ 1;
            if self.buffers[next].length.load(Ordering::Acquire) != 0 {
                self.begin_mutation();
                self.active.store(next, Ordering::Release);
                self.end_mutation();
            }
        }
    }

    pub(crate) fn write_sample(&self, sample: i16) -> bool {
        let index =
            self.active.load(Ordering::Relaxed) ^ usize::from(self.staging.load(Ordering::Relaxed));
        let buffer = &self.buffers[index];
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
    pub fn snapshot(&self) -> Option<(usize, f32, u64)> {
        let revision = self.revision.load(Ordering::Acquire);
        if revision & 1 != 0 {
            return None;
        }
        let buffer = self.current();
        let len = buffer
            .length
            .load(Ordering::Acquire)
            .min(MAX_BUFFER_SAMPLES);
        let rate = f32::from_bits(buffer.sample_rate_bits.load(Ordering::Acquire));
        (self.revision.load(Ordering::Acquire) == revision).then_some((len, rate, revision))
    }

    #[inline]
    pub fn read_sample_pair(
        &self,
        index: usize,
        revision: u64,
        latched_len: usize,
    ) -> Option<(i16, i16)> {
        if self.revision.load(Ordering::Acquire) != revision {
            return None;
        }
        let buffer = self.current();
        let len = buffer.length.load(Ordering::Acquire).min(latched_len);
        if index >= len {
            return None;
        }
        let a = buffer.samples[index].load(Ordering::Acquire);
        let b = buffer.samples[(index + 1).min(len - 1)].load(Ordering::Acquire);
        (self.revision.load(Ordering::Acquire) == revision).then_some((a, b))
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
    fn same_length_replacement_invalidates_latched_revision() {
        let memory = SampleMemory::new();
        assert!(memory.replace_buffer(vec![1, 2, 3]));
        let (len, _, revision) = memory.snapshot().unwrap();
        assert_eq!(memory.read_sample_pair(1, revision, len), Some((2, 3)));

        assert!(memory.replace_buffer(vec![4, 5, 6]));
        assert_eq!(memory.read_sample_pair(1, revision, len), None);
    }
}
