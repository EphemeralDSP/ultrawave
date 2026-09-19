#!/usr/bin/env bash
set -euo pipefail

CARGO_BIN="${CARGO:-cargo}"
"${CARGO_BIN}" test --workspace --release
./scripts/test-vst3-state.sh --release
