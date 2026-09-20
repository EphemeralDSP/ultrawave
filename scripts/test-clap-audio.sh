#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
"${CARGO:-cargo}" build -p ultrawave-plugin "$@"
# Verify an inherited screenshot environment cannot populate a hosted effect.
ULTRAWAVE_SCREENSHOT_WAV="$ROOT/tests/vinyl909.wav" \
"${CARGO:-cargo}" run --example clap_audio_harness "$@"
