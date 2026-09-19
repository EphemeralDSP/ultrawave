#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export ULTRAWAVE_SHM_KEY="clap-harness-$$-${RANDOM}"
cleanup() {
  rm -f "/dev/shm/${ULTRAWAVE_SHM_KEY}-"* 2>/dev/null || true
  rm -rf "/tmp/ultrawave-$(id -u)/${ULTRAWAVE_SHM_KEY}-"* 2>/dev/null || true
}
trap cleanup EXIT
cd "$ROOT"
"${CARGO:-cargo}" build -p ultrawave-record -p ultrawave-play "$@"
"${CARGO:-cargo}" run --example clap_audio_harness "$@"
