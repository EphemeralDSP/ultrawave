//! Save controller edits without changing the DSP parameters between sample offsets.

use std::sync::atomic::{AtomicU64, Ordering};

const EMPTY: u64 = u32::MAX as u64; // A NaN, never an accepted normalized value.

/// One coherent revision/value token. A processing block can retire only the revision it
/// observed on entry, even if the controller publishes A -> B -> A during that block.
/// Revisions wrap after 2^32 publications; a block must not span that many edits.
pub(super) struct PendingValue(AtomicU64);

impl Default for PendingValue {
    fn default() -> Self {
        Self(AtomicU64::new(EMPTY))
    }
}

impl PendingValue {
    pub fn snapshot(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }

    pub fn value(token: u64) -> Option<f32> {
        (token as u32 != EMPTY as u32).then(|| f32::from_bits(token as u32))
    }

    pub fn publish(&self, value: f32) -> u64 {
        debug_assert!(value.is_finite() && (0.0..=1.0).contains(&value));
        self.replace(value.to_bits() as u64)
    }

    pub fn invalidate(&self) {
        self.replace(EMPTY);
    }

    fn replace(&self, bits: u64) -> u64 {
        let next = |old: u64| (old.wrapping_add(1 << 32) & !EMPTY) | bits;
        // Publishers are on the controller thread, or at the existing state-restore boundary.
        // The audio processing path below does only a single bounded compare/exchange.
        let old = self
            .0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| Some(next(old)))
            .unwrap();
        next(old)
    }

    pub fn retire(&self, observed: u64) {
        if Self::value(observed).is_some() {
            let _ = self.0.compare_exchange(
                observed,
                (observed & !EMPTY) | EMPTY,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_block_cannot_retire_a_newer_edit_even_with_equal_value() {
        let pending = PendingValue::default();
        let old = pending.publish(0.25);
        pending.publish(0.75);
        let newest = pending.publish(0.25);
        pending.retire(old);
        assert_eq!(pending.snapshot(), newest);
        pending.retire(newest);
        assert_eq!(PendingValue::value(pending.snapshot()), None);
    }

    #[test]
    fn state_load_invalidates_old_tokens_without_reusing_revisions() {
        let pending = PendingValue::default();
        let old = pending.publish(0.0);
        pending.invalidate();
        assert_eq!(PendingValue::value(pending.snapshot()), None);
        let new = pending.publish(0.0);
        pending.retire(old);
        assert_eq!(pending.snapshot(), new);
    }
}
