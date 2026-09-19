#!/usr/bin/env bash
# Render deterministic UW-inspired audition WAVs without audio/MIDI hardware.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INPUT="${1:-$ROOT/tests/vinyl909.wav}"
OUT="${2:-$ROOT/target/audition}"

cargo build --manifest-path "$ROOT/Cargo.toml" --release --bins
mkdir -p "$OUT"
"$ROOT/target/release/ultrawave_record" --input "$INPUT" --output "$OUT/recorded-12bit.wav"
PLAY="$ROOT/target/release/ultrawave_play"
"$PLAY" --input "$OUT/recorded-12bit.wav" --output "$OUT/play-12bit.wav"
"$PLAY" --input "$OUT/recorded-12bit.wav" --output "$OUT/pitch-down.wav" --pitch 52
"$PLAY" --input "$OUT/recorded-12bit.wav" --output "$OUT/brr.wav" --brr 64
"$PLAY" --input "$OUT/recorded-12bit.wav" --output "$OUT/srr.wav" --srr 0.75
"$PLAY" --input "$OUT/recorded-12bit.wav" --output "$OUT/driven.wav" --dist 32 --pitch 52 --srr 0.5
printf '\nAuditions: %s\nCompare recorded-12bit/play-12bit, then pitch-down, BRR, SRR, driven.\nStart monitoring quietly; distortion raises loudness. These are approximations, not hardware reference captures.\n' "$OUT"
