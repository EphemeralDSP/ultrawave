#!/usr/bin/env bash
# Validate one CLAP bundle; retain output and reject caught plugin panics.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLUGIN="" FILTER="" VALIDATOR="" VERBOSE=0 LIST=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --plugin|--tests|--validator-path)
      [[ $# -ge 2 ]] || { echo "missing value for $1" >&2; exit 2; }
      case "$1" in
        --plugin) PLUGIN="$2";;
        --tests) FILTER="$2";;
        --validator-path) VALIDATOR="$2";;
      esac
      shift;;
    --verbose) VERBOSE=1;;
    --list-tests) LIST=1;;
    -h|--help)
      echo "usage: $0 --plugin PATH [--tests REGEX] [--verbose] [--validator-path PATH]"
      echo "       $0 --list-tests [--validator-path PATH]"
      exit 0;;
    *) echo "unknown option: $1" >&2; exit 2;;
  esac
  shift
done
if [[ -z "$VALIDATOR" ]]; then
  VALIDATOR="$(command -v clap-validator || true)"
  [[ -n "$VALIDATOR" ]] || VALIDATOR="$ROOT/target/tools/clap-validator"
fi
[[ -x "$VALIDATOR" ]] || { echo "clap-validator not executable: $VALIDATOR" >&2; exit 2; }
if (( LIST )); then exec "$VALIDATOR" list tests; fi
[[ -f "$PLUGIN" ]] || { echo "CLAP plugin not found: $PLUGIN" >&2; exit 2; }
args=()
if (( VERBOSE )); then args+=(--verbosity trace); fi
args+=(validate)
if [[ -n "$FILTER" ]]; then args+=(--test-filter "$FILTER"); fi
args+=("$PLUGIN")
export ULTRAWAVE_SHM_KEY="${ULTRAWAVE_SHM_KEY:-/uw-clap-$$}"
mkdir -p "$ROOT/target/validation"
LOG="$ROOT/target/validation/clap-validator-$(basename "$PLUGIN").log"
set +e
"$VALIDATOR" "${args[@]}" 2>&1 | tee "$LOG"
status=${PIPESTATUS[0]}
set -e
if grep -Eiq '(panic(ked)?|BadDrawable)' "$LOG"; then
  echo "fatal signature in $LOG" >&2
  exit 1
fi
[[ $status -eq 0 ]] || { echo "clap-validator failed ($status); log: $LOG" >&2; exit "$status"; }
echo "clap-validator passed; log: $LOG"
