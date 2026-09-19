use crate::machines::shared::{BufferRegistry, InstanceLease};
use crossbeam_queue::ArrayQueue;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
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
    fn disconnected(_status_bank: u8) -> Self {
        Self {
            registry: Arc::new(BufferRegistry::new()),
            _lease: None,
        }
    }

    fn open(bank: u8) -> std::io::Result<Self> {
        if bank == OFF {
            return Ok(Self::disconnected(OFF));
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
    requested: Arc<AtomicU8>,
    generation: Arc<AtomicU64>,
    status: Arc<AtomicU8>,
    stop: Arc<AtomicBool>,
    prepared: Arc<ArrayQueue<Prepared>>,
    retired: Arc<ArrayQueue<Retired>>,
    ui_registry: Arc<Mutex<Arc<BufferRegistry>>>,
    current: Option<BankResource>,
    adopted_generation: u64,
    initial_prepared: bool,
    worker: Option<JoinHandle<()>>,
}

impl Pairing {
    pub fn new() -> Self {
        let initial = BankResource::disconnected(OFF);
        let requested = Arc::new(AtomicU8::new(OFF));
        let generation = Arc::new(AtomicU64::new(0));
        let status = Arc::new(AtomicU8::new(OFF));
        let stop = Arc::new(AtomicBool::new(false));
        let prepared = Arc::new(ArrayQueue::new(2));
        let retired = Arc::new(ArrayQueue::new(4));
        let ui_registry = Arc::new(Mutex::new(initial.registry.clone()));
        let worker = {
            let requested = requested.clone();
            let generation = generation.clone();
            let status = status.clone();
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
                        let wanted_generation = generation.load(Ordering::Acquire);
                        if wanted_generation != handled {
                            let bank = requested.load(Ordering::Acquire);
                            let (resource, next_status) = match BankResource::open(bank) {
                                Ok(resource) => {
                                    (resource, if bank == OFF { OFF } else { CONNECTED })
                                }
                                Err(error) => {
                                    eprintln!("Ultrawave bank {bank} attachment failed: {error}");
                                    (BankResource::disconnected(bank), FAILED)
                                }
                            };
                            // Only the worker discards superseded prepared ownership.
                            while let Some(stale) = prepared.pop() {
                                drop(stale);
                            }
                            if generation.load(Ordering::Acquire) == wanted_generation {
                                *ui_registry.lock().expect("bank UI registry poisoned") =
                                    resource.registry.clone();
                                status.store(next_status, Ordering::Release);
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
            generation,
            status,
            stop,
            prepared,
            retired,
            ui_registry,
            current: Some(initial),
            adopted_generation: 0,
            initial_prepared: false,
            worker: Some(worker),
        }
    }

    /// Lock-free and allocation-free; safe to call once per process block.
    pub fn request(&self, bank: u8) -> bool {
        if self.requested.swap(bank, Ordering::AcqRel) != bank {
            self.status.store(CONNECTING, Ordering::Release);
            self.generation.fetch_add(1, Ordering::AcqRel);
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
        if self.initial_prepared {
            return;
        }
        self.initial_prepared = true;
        let (resource, next_status) = match BankResource::open(bank) {
            Ok(resource) => (resource, if bank == OFF { OFF } else { CONNECTED }),
            Err(error) => {
                eprintln!("Ultrawave bank {bank} attachment failed: {error}");
                (BankResource::disconnected(bank), FAILED)
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
        self.requested.store(bank, Ordering::Release);
        self.status.store(next_status, Ordering::Release);
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
        if update.generation < self.generation.load(Ordering::Acquire) {
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
        self.adopted_generation = update.generation;
        self.status.store(update.status, Ordering::Release);
        self.retired
            .push(Retired {
                _resource: old,
                _machine_registry: old_machine_registry,
            })
            .is_ok()
    }

    pub fn status(&self) -> u8 {
        self.status.load(Ordering::Acquire)
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
