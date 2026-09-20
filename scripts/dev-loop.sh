#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE=fast WATCH=0 EDITOR=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    fast|full) MODE="$1";;
    --watch) WATCH=1;;
    --editor) EDITOR=1;;
    -h|--help) echo "usage: $0 [fast|full] [--watch] [--editor]"; exit 0;;
    *) echo "unknown option: $1" >&2; exit 2;;
  esac
  shift
done

run_once() {
  local start=$SECONDS
  cargo fmt --all --check
  if [[ "$MODE" == fast ]]; then
    cargo test --workspace
    "$ROOT/scripts/test-clap-audio.sh"
    "$ROOT/scripts/test-vst3-state.sh"
    "$ROOT/scripts/audition.sh" --profile debug --no-build
  else
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace --release
    "$ROOT/scripts/test-clap-audio.sh" --release
    "$ROOT/scripts/test-vst3-state.sh" --release
    cargo xtask bundle ultrawave-plugin --release
    "$ROOT/scripts/audition.sh" --profile release --no-build
    export PATH="$ROOT/target/tools:$PATH"
    cargo xtask validate-all --skip-bundle
    if (( EDITOR )); then "$ROOT/scripts/test-editor-lifecycle.sh"; fi
    cargo run --release --example dsp_bench
  fi
  echo "dev-loop mode=$MODE seconds=$((SECONDS-start)) ok"
}
cd "$ROOT"
if (( ! WATCH )); then run_once; exit; fi
echo "watching repository inputs (1s polling; Ctrl-C to stop)"
stamp() {
  git ls-files -z --cached --others --exclude-standard | python3 -c '
import hashlib, os, sys
digest = hashlib.sha256()
for path in sorted(set(sys.stdin.buffer.read().split(b"\0"))):
    if not path or not os.path.isfile(path):
        continue
    stat = os.stat(path)
    digest.update(path + str((stat.st_mtime_ns, stat.st_size)).encode())
print(digest.hexdigest())'
}
args=("$MODE")
if (( EDITOR )); then args+=(--editor); fi
last=""
while true; do
  now="$(stamp)"
  if [[ "$now" != "$last" ]]; then
    last="$now"
    # A fresh shell preserves errexit; `run_once || true` would disable it
    # for every command inside the function and report failed tests as success.
    if ! "$ROOT/scripts/dev-loop.sh" "${args[@]}"; then
      echo "dev-loop failed; waiting for the next edit" >&2
    fi
  fi
  sleep 1
done
