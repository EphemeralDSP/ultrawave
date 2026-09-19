#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
OUT="$ROOT/target/renoise-test"
HOME_DIR="$OUT/home"
CONFIG_DIR="$HOME_DIR/.config"
DATA_DIR="$HOME_DIR/.local/share"
CACHE_DIR="$HOME_DIR/.cache"
TOOL_DIR="$CONFIG_DIR/Renoise/V3.5.4/Scripts/Tools/com.ephemeraldsp.UltrawaveHostTest.xrnx"
DISPLAY_NUMBER="${ULTRAWAVE_RENOISE_DISPLAY:-:95}"
SHM_KEY="/ultrawave_renoise_test_${$}"

for tool in Xvfb xdotool import; do
  command -v "$tool" >/dev/null || { echo "Missing host-test tool: $tool" >&2; exit 1; }
done
rm -rf "$OUT"
mkdir -p "$TOOL_DIR" "$DATA_DIR" "$CACHE_DIR"
cp -a "$ROOT/tests/renoise/com.ephemeraldsp.UltrawaveHostTest.xrnx/." "$TOOL_DIR/"
: > "$OUT/host-result.txt"

# Avoid Renoise's first-run realtime-priority warning.  Work only on an
# isolated copy: the user's profile is never written by this fixture.
if [[ -f "$HOME/.config/Renoise/V3.5.4/Config.xml" ]]; then
  cp "$HOME/.config/Renoise/V3.5.4/Config.xml" "$CONFIG_DIR/Renoise/V3.5.4/Config.xml"
  sed -i 's#<UseRealTimePriority>true</UseRealTimePriority>#<UseRealTimePriority>false</UseRealTimePriority>#' \
    "$CONFIG_DIR/Renoise/V3.5.4/Config.xml"
  sed -i 's#<ShowWelcomeDialog>true</ShowWelcomeDialog>#<ShowWelcomeDialog>false</ShowWelcomeDialog>#' \
    "$CONFIG_DIR/Renoise/V3.5.4/Config.xml"
fi

stop_renoise() {
  if [[ -n "${RENOISE_PID:-}" ]] && kill -0 "$RENOISE_PID" 2>/dev/null; then
    kill -TERM "$RENOISE_PID" 2>/dev/null || true
    for _ in {1..20}; do
      kill -0 "$RENOISE_PID" 2>/dev/null || break
      sleep 0.1
    done
    if kill -0 "$RENOISE_PID" 2>/dev/null; then kill -KILL "$RENOISE_PID" 2>/dev/null || true; fi
    wait "$RENOISE_PID" 2>/dev/null || true
  fi
  RENOISE_PID=""
}

cleanup() {
  stop_renoise
  if [[ -n "${XVFB_PID:-}" ]] && kill -0 "$XVFB_PID" 2>/dev/null; then
    kill -TERM "$XVFB_PID" 2>/dev/null || true
    wait "$XVFB_PID" 2>/dev/null || true
  fi
  rm -f "/dev/shm${SHM_KEY}" 2>/dev/null || true
  rm -f "/dev/shm${SHM_KEY}-bank-1" 2>/dev/null || true
  rm -rf "/tmp/ultrawave-$(id -u)/${SHM_KEY#/}-bank-1"
}
trap cleanup EXIT INT TERM

Xvfb "$DISPLAY_NUMBER" -screen 0 1280x800x24 -nolisten tcp >"$OUT/xvfb.log" 2>&1 &
XVFB_PID=$!
sleep 0.5

run_host() {
  local phase="$1" marker="$2"
  shift 2
  # Prepare the initial bank during activation, before offline rendering races
  # the connector worker. Reopen starts OFF to prove the project restores it.
  local initial_bank=0
  if [[ "$phase" == capture ]]; then initial_bank=1; fi
  HOME="$HOME_DIR" XDG_CONFIG_HOME="$CONFIG_DIR" XDG_DATA_HOME="$DATA_DIR" \
  XDG_CACHE_HOME="$CACHE_DIR" DISPLAY="$DISPLAY_NUMBER" \
  VST3_PATH="$ROOT/target/bundled" ULTRAWAVE_SHM_KEY="$SHM_KEY" ULTRAWAVE_BANK="$initial_bank" \
  ULTRAWAVE_RENOISE_TEST_ROOT="$OUT" ULTRAWAVE_RENOISE_PHASE="$phase" \
    /usr/bin/renoise "$@" >>"$OUT/renoise.stdout.log" 2>>"$OUT/renoise.stderr.log" &
  RENOISE_PID=$!
  local deadline=$((SECONDS + ${ULTRAWAVE_RENOISE_TIMEOUT:-300}))
  while ! grep -q "^${marker}=true$" "$OUT/host-result.txt" 2>/dev/null; do
    # Only dismiss first-run dialogs, on this isolated display.
    if [[ ! -s "$OUT/host-result.txt" ]]; then
      while read -r window; do
        DISPLAY="$DISPLAY_NUMBER" xdotool key --window "$window" Return >>"$OUT/xdotool.log" 2>&1 || true
      done < <(DISPLAY="$DISPLAY_NUMBER" xdotool search --onlyvisible --name 'Renoise' 2>>"$OUT/xdotool.log" || true)
    fi
    if grep -q '^fatal=' "$OUT/host-result.txt" 2>/dev/null; then
      cat "$OUT/host-result.txt" >&2
      exit 1
    fi
    if ! kill -0 "$RENOISE_PID" 2>/dev/null; then
      echo "Renoise exited during $phase; inspect $OUT/renoise.stdout.log" >&2
      exit 1
    fi
    if (( SECONDS >= deadline )); then
      import -display "$DISPLAY_NUMBER" -window root "$OUT/isolated-renoise-blocked.png" 2>/dev/null || true
      echo "fatal=Renoise $phase phase timed out" >> "$OUT/host-result.txt"
      echo "Timed out; inspect $OUT/host-result.txt and Renoise logs" >&2
      exit 1
    fi
    sleep 0.25
  done
}

run_host capture capture_complete
sleep 0.5
import -display "$DISPLAY_NUMBER" -window root "$OUT/one-shot-locked.png"
stop_renoise
# Keep the SAME shared-memory key and mapping. A fresh host must clear RAM by
# lifecycle rules, not because this fixture deleted the mapping between runs.
run_host reopen complete "$OUT/ultrawave-host-test.xrns"
sleep 0.5 # Let the final editor paint before capture.
import -display "$DISPLAY_NUMBER" -window root "$OUT/isolated-renoise.png"
if grep -Eiq 'panicked at|BadDrawable|BadWindow|Error while closing window' "$OUT/renoise.stderr.log" "$OUT/renoise.stdout.log"; then
  echo "Plugin panic/X11 error in Renoise logs" >&2
  exit 1
fi

python "$ROOT/tests/renoise/verify_audio.py" "$OUT"

cat "$OUT/host-result.txt"
echo "PASS: artifacts are in $OUT"
