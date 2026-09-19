use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use ultrawave::dsp::{effects::Distortion, mix::mix_inputs};
use ultrawave::machines::ram_play::{RamPlay, RamPlayParams};
use ultrawave::machines::ram_record::{RamRecord, RamRecordParams};
use ultrawave::machines::shared::BufferRegistry;

fn main() {
    for drive in [0.0, 0.75] {
        let mut scalar = Vec::new();
        let mut simd = Vec::new();
        for _ in 0..7 {
            scalar.push(capture::<false>(drive));
            simd.push(capture::<true>(drive));
        }
        scalar.sort_by(f64::total_cmp);
        simd.sort_by(f64::total_cmp);
        println!(
            "capture drive={drive}: scalar {:.2}, SIMD {:.2} ns/sample ({:.2}x, median of 7)",
            scalar[3],
            simd[3],
            scalar[3] / simd[3]
        );
    }

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

// Same mix → SRR → distortion → atomic 12-bit capture → monitor in both cases.
// Baseline mixes inline per sample, as the original callback did. LLVM remains
// free to optimize it. This measures the DSP path, not host/plugin overhead.
fn capture<const BATCHED: bool>(drive: f32) -> f64 {
    const BLOCKS: usize = 32768;
    let inputs: [[f32; 64]; 4] = std::array::from_fn(|bus| {
        std::array::from_fn(|i| ((i * 13 + bus * 31) as f32 * 0.1).sin() * 0.4)
    });
    let params = RamRecordParams {
        rec_len: 127,
        ..Default::default()
    };
    let registry = Arc::new(BufferRegistry::new());
    let mut recorder = RamRecord::with_registry(44100.0, registry);
    let mut decimator = ultrawave::Decimator::new();
    let mut distortion = Distortion::new();
    distortion.set_drive(drive);
    let mut mixed = [0.0; 64];
    let mut output = [0.0; 64];
    let start = Instant::now();
    for _ in 0..BLOCKS {
        if !recorder.is_recording(0) {
            recorder.start_recording(&params, true, 0);
        }
        let inputs = black_box(inputs.each_ref().map(|s| s.as_slice()));
        let gains = black_box([0.17, 0.83, 0.37, 0.63]);
        if BATCHED {
            mix_inputs(inputs, gains, &mut mixed);
        }
        for i in 0..64 {
            let dry = if BATCHED {
                mixed[i]
            } else {
                inputs[0][i] * gains[0]
                    + inputs[1][i] * gains[1]
                    + inputs[2][i] * gains[2]
                    + inputs[3][i] * gains[3]
            };
            let wet = distortion.process(decimator.process(dry, drive));
            recorder.record_mono(wet, 0);
            output[i] = dry * 0.25 + recorder.monitor_sample(0) * 0.75;
        }
        black_box(&output);
    }
    start.elapsed().as_nanos() as f64 / (BLOCKS * 64) as f64
}
