#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
OUT="$ROOT/target/renoise-test"
HOME_DIR="$OUT/home"
CONFIG_DIR="$HOME_DIR/.config"
DATA_DIR="$HOME_DIR/.local/share"
CACHE_DIR="$HOME_DIR/.cache"
TOOL_DIR="$CONFIG_DIR/Renoise/V3.5.4/Scripts/Tools/com.ephemeraldsp.UltrawaveHostTest.xrnx"
PLUGIN_DIR="$OUT/plugins"
BUNDLE="$ROOT/target/bundled/ultrawave-plugin.vst3"
DISPLAY_NUMBER="${ULTRAWAVE_RENOISE_DISPLAY:-:95}"

for tool in Xvfb xdotool import; do
  command -v "$tool" >/dev/null || { echo "Missing host-test tool: $tool" >&2; exit 1; }
done
[[ -d "$BUNDLE" ]] || { echo "Missing unified VST3 bundle: $BUNDLE" >&2; exit 1; }
rm -rf "$OUT"
mkdir -p "$TOOL_DIR" "$DATA_DIR" "$CACHE_DIR" "$PLUGIN_DIR"
cp -a "$ROOT/tests/renoise/com.ephemeraldsp.UltrawaveHostTest.xrnx/." "$TOOL_DIR/"
# Scan only a private copy of the just-built unified bundle. In particular, do
# not expose Renoise to the user's ~/.vst3, ~/.clap, or stale split bundles.
cp -a "$BUNDLE" "$PLUGIN_DIR/"
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
}
trap cleanup EXIT INT TERM

Xvfb "$DISPLAY_NUMBER" -screen 0 1280x800x24 -nolisten tcp >"$OUT/xvfb.log" 2>&1 &
XVFB_PID=$!
sleep 0.5
if ! kill -0 "$XVFB_PID" 2>/dev/null; then
  cat "$OUT/xvfb.log" >&2
  echo "Isolated Xvfb failed to start on $DISPLAY_NUMBER" >&2
  exit 1
fi

run_host() {
  local phase="$1" marker="$2"
  local alias_created=false
  shift 2
  HOME="$HOME_DIR" XDG_CONFIG_HOME="$CONFIG_DIR" XDG_DATA_HOME="$DATA_DIR" \
  XDG_CACHE_HOME="$CACHE_DIR" DISPLAY="$DISPLAY_NUMBER" \
  VST3_PATH="$PLUGIN_DIR" CLAP_PATH="$OUT/no-clap-plugins" \
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
    # Renoise 3.5 exposes FX aliases to Lua only after they exist; both target
    # indexes are read-only and load_plugin() explicitly rejects aliases. Drive
    # the real instrument-plugin chooser on this private 1280x800 display once,
    # then let Lua verify that it resolved to track 1/device 2 before rendering.
    if [[ "$phase" == capture && "$alias_created" == false ]] &&
       grep -q '^fx_alias_needed=true$' "$OUT/host-result.txt" 2>/dev/null; then
      sleep 0.25
      DISPLAY="$DISPLAY_NUMBER" xdotool key Escape >>"$OUT/xdotool.log" 2>&1
      DISPLAY="$DISPLAY_NUMBER" xdotool mousemove 75 115 click 1 >>"$OUT/xdotool.log" 2>&1
      DISPLAY="$DISPLAY_NUMBER" xdotool mousemove 20 172 click 1 >>"$OUT/xdotool.log" 2>&1
      DISPLAY="$DISPLAY_NUMBER" xdotool mousemove 80 188 click --repeat 2 --delay 100 1 \
        >>"$OUT/xdotool.log" 2>&1
      alias_created=true
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
import -display "$DISPLAY_NUMBER" -window root "$OUT/sample-protected.png"
stop_renoise
# Reopen the saved project in an entirely fresh process. The parameter state and
# FX alias must return, while instance-private sample and Record Once latch must not.
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
