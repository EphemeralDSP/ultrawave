#!/usr/bin/env bash
# Private macOS REAPER fixture. Never touches the user's REAPER configuration or
# plugin directories, and never runs while another REAPER process is present.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
out="$root/target/mac-host-smoke"
reaper=/Applications/REAPER.app/Contents/MacOS/REAPER
bundle="$root/target/bundled/ultrawave-plugin.vst3"
[[ -x "$reaper" && -d "$bundle" ]] || { echo "Missing REAPER or bundle" >&2; exit 1; }
if pgrep -x REAPER >/dev/null; then
  echo "REAPER is already running; refusing to affect an existing session" >&2
  exit 1
fi
rm -rf "$out"
mkdir -p "$out/config" "$out/home/Library/Audio/Plug-Ins/VST3"
cp -R "$bundle" "$out/home/Library/Audio/Plug-Ins/VST3/"
printf '[REAPER]\n' > "$out/config/reaper.ini"
python3 - "$out/tone.wav" <<'PY'
import math, struct, sys, wave
with wave.open(sys.argv[1], 'wb') as wav:
    wav.setparams((2, 2, 48000, 96000, 'NONE', 'not compressed'))
    for frame in range(96000):
        t = frame / 48000
        wav.writeframesraw(struct.pack('<hh',
            int(12000 * math.sin(2 * math.pi * 440 * t)),
            int(7000 * math.sin(2 * math.pi * 660 * t))))
PY

host_pid=""
stop_host() {
  if [[ -n "$host_pid" ]] && kill -0 "$host_pid" 2>/dev/null; then
    kill -TERM "$host_pid"
    wait "$host_pid" 2>/dev/null || true
  fi
  host_pid=""
}
trap stop_host EXIT
start_host() {
  local phase=$1
  HOME="$out/home" ULTRAWAVE_MAC_OUT="$out" ULTRAWAVE_MAC_PHASE="$phase" \
    "$reaper" -newinst -nosplash -cfgfile "$out/config/reaper.ini" \
    > "$out/$phase-launch.log" 2>&1 &
  host_pid=$!
  # Startup can finish scanning before the first script is dispatched. Forward
  # to this running private instance after its resource directory has appeared.
  for _ in {1..80}; do
    [[ -f "$out/config/reaper-vstplugins_arm64.ini" ]] && break
    kill -0 "$host_pid" 2>/dev/null || { echo "REAPER exited on startup" >&2; exit 1; }
    sleep 0.25
  done
  sleep 2
}
forward() {
  HOME="$out/home" "$reaper" -nonewinst -cfgfile "$out/config/reaper.ini" "$1" \
    >> "$out/forward.log" 2>&1
}
run_phase() {
  local phase=$1
  forward "$root/scripts/mac-host-smoke.lua"
  for _ in {1..160}; do
    if [[ -f "$out/$phase.txt" ]]; then
      if grep -q '^fatal=' "$out/$phase.txt"; then cat "$out/$phase.txt" >&2; exit 1; fi
      if grep -q '^complete=true$' "$out/$phase.txt"; then return; fi
    fi
    kill -0 "$host_pid" 2>/dev/null || { echo "REAPER exited in $phase" >&2; exit 1; }
    sleep 0.25
  done
  echo "REAPER $phase timed out; see $out" >&2
  exit 1
}

start_host capture
run_phase capture
lsof -p "$host_pid" | grep -F "$out/home/Library/Audio/Plug-Ins/VST3/ultrawave-plugin.vst3/Contents/MacOS/ultrawave-plugin" \
  > "$out/loaded-binary.txt" || { echo "Private VST3 binary was not loaded" >&2; exit 1; }
stop_host
start_host reopen
forward "$out/saved.rpp"
# Project opening is queued on the GUI thread. Wait for REAPER to load the
# actual saved FX before forwarding the script to the same instance.
sleep 2
run_phase reopen
stop_host

python3 - "$out" <<'PY'
import math, pathlib, sys, wave
root = pathlib.Path(sys.argv[1])
def rms_at(name, time):
    with wave.open(str(root / name)) as wav:
        assert wav.getframerate() == 48000 and wav.getnchannels() == 2
        wav.setpos(int(time * wav.getframerate()))
        raw = wav.readframes(3840)
        width = wav.getsampwidth()
        values = [int.from_bytes(raw[i:i+width], 'little', signed=True) / 2**(8*width-1)
                  for i in range(0, len(raw), width)]
        assert values, f'{name}: no audio at {time}'
        return math.sqrt(sum(x*x for x in values) / len(values))
before = rms_at('captured-and-played.wav', .3)
played = rms_at('captured-and-played.wav', 1.03)
empty = rms_at('play-after-reopen.wav', 1.03)
assert before < 1e-7, f'unexpected audio before playback: {before}'
assert played > .005, f'MIDI-triggered capture/playback was silent: {played}'
assert empty < 1e-7, f'RAM survived fresh-process project reopen: {empty}'
for phase in ('capture', 'reopen'):
    result = (root / (phase + '.txt')).read_text()
    fields = dict(line.split('=', 1) for line in result.splitlines() if '=' in line)
    suffix = '_after_reopen' if phase == 'reopen' else ''
    assert fields['complete'] == 'true', fields
    assert fields['editor_open' + suffix] == 'true', fields
    assert fields['editor_closed' + suffix] == 'true', fields
print(f'PASS: VST3 MIDI capture/play RMS={played:.6f}; before={before:.6f}; '
      f'after fresh reopen={empty:.6f}; saved controls and editor lifecycle OK')
print(f'Artifacts: {root}')
PY
