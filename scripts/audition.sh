#!/usr/bin/env bash
# Render deterministic UW-inspired audition WAVs without audio/MIDI hardware.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${ULTRAWAVE_PROFILE:-debug}"; BUILD=1
while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile) [[ $# -ge 2 ]] || { echo "missing value for --profile" >&2; exit 2; }; PROFILE="$2"; shift 2;;
    --no-build) BUILD=0; shift;;
    -h|--help) echo "usage: $0 [--profile debug|release] [--no-build] [INPUT] [OUT]"; exit 0;;
    -*) echo "unknown option: $1" >&2; exit 2;;
    *) break;;
  esac
done
[[ "$PROFILE" == debug || "$PROFILE" == release ]] || { echo "profile must be debug or release" >&2; exit 2; }
INPUT="${1:-$ROOT/tests/vinyl909.wav}"; OUT="${2:-$ROOT/target/audition}"
[[ -f "$INPUT" ]] || { echo "input not found: $INPUT" >&2; exit 2; }

if (( BUILD )); then
  build_args=(--manifest-path "$ROOT/Cargo.toml" --bins)
  if [[ "$PROFILE" == release ]]; then build_args+=(--release); fi
  cargo build "${build_args[@]}"
fi
BIN="$ROOT/target/$PROFILE/ultrawave"
[[ -x "$BIN" ]] || { echo "$PROFILE binary missing; omit --no-build" >&2; exit 1; }
mkdir -p "$OUT"
"$BIN" record --input "$INPUT" --output "$OUT/recorded-12bit.wav"
"$BIN" play --input "$OUT/recorded-12bit.wav" --output "$OUT/play-12bit.wav"
"$BIN" play --input "$OUT/recorded-12bit.wav" --output "$OUT/pitch-down.wav" --pitch 52
"$BIN" play --input "$OUT/recorded-12bit.wav" --output "$OUT/brr.wav" --brr 64
"$BIN" play --input "$OUT/recorded-12bit.wav" --output "$OUT/srr.wav" --srr 0.75
"$BIN" play --input "$OUT/recorded-12bit.wav" --output "$OUT/driven.wav" --dist 32 --pitch 52 --srr 0.5
python3 "$ROOT/scripts/audio-metrics.py" "$OUT"
printf '\nAuditions: %s\nCompare recorded-12bit/play-12bit, then pitch-down, BRR, SRR, driven.\nStart monitoring quietly; distortion raises loudness. These are approximations, not hardware reference captures.\n' "$OUT"
