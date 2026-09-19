use std::sync::Arc;
use ultrawave::machines::shared::BufferRegistry;

#[test]
fn explicit_banks_pair_isolate_disconnect_and_clear_independently() {
    let prefix = format!("ultrawave-bank-test-{}", std::process::id());
    std::env::set_var("ULTRAWAVE_SHM_KEY", prefix);

    let bank1a = BufferRegistry::open_bank(1).unwrap();
    let lease1a = bank1a.register_instance().unwrap();
    let bank1b = BufferRegistry::open_bank(1).unwrap();
    let lease1b = bank1b.register_instance().unwrap();
    let bank2 = BufferRegistry::open_bank(2).unwrap();
    let _lease2 = bank2.register_instance().unwrap();
    assert!(bank1a.replace_buffer(0, vec![11, 22]));
    assert_eq!(bank1b.read_buffer(0).unwrap(), vec![11, 22]);
    assert_eq!(bank2.buffer_len(0), 0);

    let off_a = Arc::new(BufferRegistry::new());
    let off_b = Arc::new(BufferRegistry::new());
    assert!(off_a.replace_buffer(0, vec![7]));
    assert_eq!(off_b.buffer_len(0), 0);

    drop(lease1a);
    drop(lease1b);
    drop(bank1a);
    drop(bank1b);
    // Keeping another bank alive must not keep bank 1's volatile session alive.
    let reopened = BufferRegistry::open_bank(1).unwrap();
    let _reopened_lease = reopened.register_instance().unwrap();
    assert_eq!(reopened.buffer_len(0), 0);
    assert!(bank2.replace_buffer(0, vec![33]));
    assert_eq!(bank2.read_buffer(0).unwrap(), vec![33]);
}
