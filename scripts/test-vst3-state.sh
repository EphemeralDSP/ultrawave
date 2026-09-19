#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# The vendored core is deliberately outside our workspace so ordinary lint/format
# runs do not rewrite upstream code. Seed its test-only lockfile from our pins.
cp "$ROOT/Cargo.lock" "$ROOT/vendor/nice-plug/Cargo.lock"
"${CARGO:-cargo}" test --manifest-path "$ROOT/vendor/nice-plug/Cargo.toml" \
  --target-dir "$ROOT/target" --no-default-features --features vst3 --lib "$@" wrapper::vst3
