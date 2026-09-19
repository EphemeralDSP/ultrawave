use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use ultrawave::machines::ram_play::{RamPlay, RamPlayParams};
use ultrawave::machines::shared::BufferRegistry;

fn main() {
    for (label, len) in [("short", 4_410), ("10s", 441_000)] {
        let registry = Arc::new(BufferRegistry::new());
        registry.replace_buffer(0, vec![512; len]);
        let mut player = RamPlay::with_registry(44_100.0, registry);
        let params = RamPlayParams {
            vol: 127,
            ..Default::default()
        };
        let start = Instant::now();
        for _ in 0..100_000 {
            player.trigger(black_box(&params), 0);
        }
        println!(
            "trigger {label}: {:.1} ns",
            start.elapsed().as_nanos() as f64 / 100_000.0
        );
    }

    for (label, params) in [
        (
            "neutral",
            RamPlayParams {
                vol: 127,
                ..Default::default()
            },
        ),
        (
            "crunchy",
            RamPlayParams {
                vol: 127,
                srr: 0.8,
                brr: 96,
                dist: 110,
                ..Default::default()
            },
        ),
    ] {
        let registry = Arc::new(BufferRegistry::new());
        registry.replace_buffer(0, vec![512; 441_000]);
        let mut player = RamPlay::with_registry(44_100.0, registry);
        player.trigger(&params, 0);
        let start = Instant::now();
        for _ in 0..1_000_000 {
            black_box(player.process(0));
            if !player.is_playing(0) {
                player.trigger(&params, 0);
            }
        }
        println!(
            "process {label}: {:.1} ns/sample",
            start.elapsed().as_nanos() as f64 / 1_000_000.0
        );
    }
}
