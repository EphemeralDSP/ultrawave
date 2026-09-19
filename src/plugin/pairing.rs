use crate::machines::shared::{BufferRegistry, InstanceLease};
use crossbeam_queue::ArrayQueue;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub const OFF: u8 = 0;
pub const CONNECTING: u8 = 1;
pub const CONNECTED: u8 = 2;
pub const FAILED: u8 = 3;

struct BankResource {
    registry: Arc<BufferRegistry>,
    _lease: Option<Arc<InstanceLease>>,
}

impl BankResource {
    fn disconnected() -> Self {
        Self {
            registry: Arc::new(BufferRegistry::new()),
            _lease: None,
        }
    }

    fn open(bank: u8) -> std::io::Result<Self> {
        if bank == OFF {
            return Ok(Self::disconnected());
        }
        let registry = BufferRegistry::open_bank(bank)?;
        let lease = registry.register_instance()?;
        Ok(Self {
            registry,
            _lease: Some(lease),
        })
    }
}

struct Prepared {
    generation: u64,
    status: u8,
    resource: BankResource,
}

struct Retired {
    _resource: BankResource,
    _machine_registry: Arc<BufferRegistry>,
}

/// Per-plugin bank connector. Mapping, lifecycle registration and destruction
/// happen only on this worker; the process callback only swaps prepared Arcs.
pub struct Pairing {
    // One atomic publishes the bank (low byte) and generation together.
    requested: Arc<AtomicU64>,
    status: u8,
    stop: Arc<AtomicBool>,
    prepared: Arc<ArrayQueue<Prepared>>,
    retired: Arc<ArrayQueue<Retired>>,
    ui_registry: Arc<Mutex<Arc<BufferRegistry>>>,
    current: Option<BankResource>,
    initial_prepared: bool,
    worker: Option<JoinHandle<()>>,
}

impl Pairing {
    pub fn new() -> Self {
        let initial = BankResource::disconnected();
        let requested = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let prepared = Arc::new(ArrayQueue::new(2));
        let retired = Arc::new(ArrayQueue::new(4));
        let ui_registry = Arc::new(Mutex::new(initial.registry.clone()));
        let worker = {
            let requested = requested.clone();
            let stop = stop.clone();
            let prepared = prepared.clone();
            let retired = retired.clone();
            let ui_registry = ui_registry.clone();
            thread::Builder::new()
                .name("ultrawave-bank".into())
                .spawn(move || {
                    let mut handled = 0;
                    while !stop.load(Ordering::Acquire) {
                        while retired.pop().is_some() {}
                        let wanted_generation = requested.load(Ordering::Acquire);
                        // Generation zero was prepared synchronously on activation.
                        if wanted_generation >> 8 != 0 && wanted_generation != handled {
                            let bank = wanted_generation as u8;
                            let (resource, next_status) = match BankResource::open(bank) {
                                Ok(resource) => {
                                    (resource, if bank == OFF { OFF } else { CONNECTED })
                                }
                                Err(error) => {
                                    eprintln!("Ultrawave bank {bank} attachment failed: {error}");
                                    (BankResource::disconnected(), FAILED)
                                }
                            };
                            // Only the worker discards superseded prepared ownership.
                            while let Some(stale) = prepared.pop() {
                                drop(stale);
                            }
                            if requested.load(Ordering::Acquire) == wanted_generation {
                                *ui_registry.lock().expect("bank UI registry poisoned") =
                                    resource.registry.clone();
                                let _ = prepared.push(Prepared {
                                    generation: wanted_generation,
                                    status: next_status,
                                    resource,
                                });
                                handled = wanted_generation;
                            }
                        }
                        thread::sleep(Duration::from_millis(2));
                    }
                    while retired.pop().is_some() {}
                    while prepared.pop().is_some() {}
                })
                .expect("failed to start Ultrawave bank worker")
        };
        Self {
            requested,
            status: OFF,
            stop,
            prepared,
            retired,
            ui_registry,
            current: Some(initial),
            initial_prepared: false,
            worker: Some(worker),
        }
    }

    /// Lock-free and allocation-free; safe to call once per process block.
    pub fn request(&mut self, bank: u8) -> bool {
        let previous = self.requested.load(Ordering::Relaxed);
        if previous as u8 != bank {
            self.status = CONNECTING;
            self.requested
                .store(((previous & !255) + 256) | bank as u64, Ordering::Release);
            true
        } else {
            false
        }
    }

    /// Activation is explicitly non-RT, so the restored initial bank can be
    /// ready before the first block (and before standalone waveform preload).
    pub fn prepare_initial(
        &mut self,
        bank: u8,
        rebind: impl FnOnce(Arc<BufferRegistry>) -> Arc<BufferRegistry>,
    ) {
        if self.initial_prepared
            && self.requested.load(Ordering::Acquire) as u8 == bank
            && matches!(self.status, OFF | CONNECTED)
        {
            return;
        }
        self.initial_prepared = true;
        let (resource, next_status) = match BankResource::open(bank) {
            Ok(resource) => (resource, if bank == OFF { OFF } else { CONNECTED }),
            Err(error) => {
                eprintln!("Ultrawave bank {bank} attachment failed: {error}");
                (BankResource::disconnected(), FAILED)
            }
        };
        let old_machine = rebind(resource.registry.clone());
        let old = self
            .current
            .replace(resource)
            .expect("pairing resource missing");
        // Activation is not an audio callback, so these old local resources may
        // be destroyed synchronously.
        drop(old_machine);
        drop(old);
        self.requested.store(bank as u64, Ordering::Release);
        self.status = next_status;
        *self.ui_registry.lock().expect("bank UI registry poisoned") = self.current_registry();
    }

    pub fn adopt(
        &mut self,
        rebind: impl FnOnce(Arc<BufferRegistry>) -> Arc<BufferRegistry>,
    ) -> bool {
        if self.retired.is_full() {
            return false;
        }
        let Some(update) = self.prepared.pop() else {
            return false;
        };
        if update.generation != self.requested.load(Ordering::Acquire) {
            // Return ownership for off-thread destruction. Capacity was checked.
            let old_arc = update.resource.registry.clone();
            let _ = self.retired.push(Retired {
                _resource: update.resource,
                _machine_registry: old_arc,
            });
            return false;
        }
        let old_machine_registry = rebind(update.resource.registry.clone());
        let old = self
            .current
            .replace(update.resource)
            .expect("pairing resource missing");
        self.status = update.status;
        self.retired
            .push(Retired {
                _resource: old,
                _machine_registry: old_machine_registry,
            })
            .is_ok()
    }

    pub fn status(&self) -> u8 {
        self.status
    }
    pub fn ui_registry(&self) -> Arc<Mutex<Arc<BufferRegistry>>> {
        self.ui_registry.clone()
    }
    pub fn current_registry(&self) -> Arc<BufferRegistry> {
        self.current.as_ref().unwrap().registry.clone()
    }
}

impl Drop for Pairing {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        // Plugin teardown is outside process; current may safely release here.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reactivation_prepares_restored_bank_before_the_first_audio_block() {
        let mut pairing = Pairing::new();
        let mut machine = pairing.current_registry();
        pairing.prepare_initial(17, |next| std::mem::replace(&mut machine, next));
        assert_eq!(pairing.status(), FAILED);
        pairing.prepare_initial(0, |next| std::mem::replace(&mut machine, next));
        assert_eq!(pairing.status(), OFF);
        assert!(!pairing.request(0));
        assert!(Arc::ptr_eq(&machine, &pairing.current_registry()));
    }

    #[test]
    fn prepared_bank_is_not_connected_until_audio_adopts_it() {
        let mut pairing = Pairing::new();
        let mut machine = pairing.current_registry();
        // Invalid bank exercises the failure path without creating shared files.
        assert!(pairing.request(17));
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while pairing.prepared.is_empty() {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(pairing.status(), CONNECTING);
        assert!(pairing.adopt(|next| std::mem::replace(&mut machine, next)));
        assert_eq!(pairing.status(), FAILED);
        assert!(Arc::ptr_eq(&machine, &pairing.current_registry()));
    }

    #[test]
    fn full_retirement_queue_and_superseded_update_keep_ownership_until_drained() {
        let mut pairing = Pairing::new();
        // Pause only the non-RT consumer to exercise deterministic backpressure.
        pairing.stop.store(true, Ordering::Release);
        pairing.worker.take().unwrap().join().unwrap();
        let mut machine = pairing.current_registry();
        for _ in 0..pairing.retired.capacity() {
            let resource = BankResource::disconnected();
            assert!(pairing
                .retired
                .push(Retired {
                    _machine_registry: resource.registry.clone(),
                    _resource: resource,
                })
                .is_ok());
        }
        pairing.request(1);
        let resource = BankResource::disconnected();
        let weak = Arc::downgrade(&resource.registry);
        assert!(pairing
            .prepared
            .push(Prepared {
                generation: pairing.requested.load(Ordering::Acquire),
                status: CONNECTED,
                resource,
            })
            .is_ok());
        assert!(!pairing.adopt(|_| panic!("must not adopt when retirement is full")));
        pairing.request(2);
        drop(pairing.retired.pop());
        assert!(!pairing.adopt(|_| panic!("must not adopt a superseded bank")));
        assert_eq!(pairing.status(), CONNECTING);
        assert!(
            weak.upgrade().is_some(),
            "stale resource must be retired, not destroyed"
        );
        while pairing.retired.pop().is_some() {}
        assert!(weak.upgrade().is_none());
        let resource = BankResource::disconnected();
        assert!(pairing
            .prepared
            .push(Prepared {
                generation: pairing.requested.load(Ordering::Acquire),
                status: CONNECTED,
                resource,
            })
            .is_ok());
        assert!(pairing.adopt(|next| std::mem::replace(&mut machine, next)));
        assert_eq!(pairing.status(), CONNECTED);
        assert!(Arc::ptr_eq(&machine, &pairing.current_registry()));
    }
}
