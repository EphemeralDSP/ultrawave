#!/usr/bin/env bash
# Fail-closed pluginval runner. Output is always retained in target/validation.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLUGIN_PATH="" STRICTNESS=5 PLUGINVAL_PATH="${PLUGINVAL:-}" VERBOSE=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --plugin|--strictness|--pluginval-path) [[ $# -ge 2 ]] || { echo "missing value for $1" >&2; exit 2; }; key="$1"; val="$2"; shift 2; case "$key" in --plugin) PLUGIN_PATH="$val";; --strictness) STRICTNESS="$val";; *) PLUGINVAL_PATH="$val";; esac;;
    --verbose) VERBOSE=true; shift;;
    -h|--help) echo "usage: $0 --plugin PATH [--strictness 1..10] [--pluginval-path PATH] [--verbose]"; exit 0;;
    *) echo "unknown option: $1" >&2; exit 2;;
  esac
done
[[ "$STRICTNESS" =~ ^([1-9]|10)$ ]] || { echo "strictness must be 1..10" >&2; exit 2; }
[[ -n "$PLUGIN_PATH" && -e "$PLUGIN_PATH" ]] || { echo "plugin not found: ${PLUGIN_PATH:-<missing --plugin>}" >&2; exit 2; }
[[ -n "$PLUGINVAL_PATH" ]] || PLUGINVAL_PATH="$(command -v pluginval || true)"
[[ -x "$PLUGINVAL_PATH" ]] || { echo "pluginval not executable; set PLUGINVAL or --pluginval-path" >&2; exit 2; }
mkdir -p "$ROOT/target/validation"
LOG="$ROOT/target/validation/pluginval-$(basename "$PLUGIN_PATH").log"
args=(--validate-in-process --strictness-level "$STRICTNESS" --validate "$PLUGIN_PATH")
$VERBOSE && args+=(--verbose)
runner=()
if [[ "$(uname -s)" == Linux ]]; then runner=(xvfb-run -a); fi
set +e
"${runner[@]}" "$PLUGINVAL_PATH" "${args[@]}" 2>&1 | tee "$LOG"
status=${PIPESTATUS[0]}
set -e
if grep -Eiq '(panic(ked)?|BadDrawable|BadWindow|Error while closing window)' "$LOG"; then echo "fatal signature in $LOG" >&2; exit 1; fi
[[ $status -eq 0 ]] || { echo "pluginval failed ($status); log: $LOG" >&2; exit "$status"; }
echo "pluginval passed; log: $LOG"
