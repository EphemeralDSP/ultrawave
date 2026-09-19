# Ultrawave

[Buildkite CI](https://buildkite.com/jmlynn/ultrawave)

Real-time RAM recording and playback plugins inspired by the Elektron Machinedrum UW RAM machines. Built with Rust and [NIH-plug](https://github.com/robbert-vdh/nih-plug).

## Overview

Ultrawave provides **two companion plugins** that share a global audio buffer:

| Plugin | Purpose | Audio I/O |
|--------|---------|-----------|
| **Ultrawave Record** | Captures audio to shared RAM buffer | Stereo main + sidechain → optional mono monitoring |
| **Ultrawave Play** | Plays back from shared RAM buffer | MIDI In → Stereo Out |

Use them together in a DAW to sample and resample audio in real-time, similar to the Machinedrum UW's R1/R2 (record) and P1/P2 (play) machines.

### Workflow

1. Insert **Ultrawave Record** on a track to capture audio
2. Insert **Ultrawave Play** on an instrument track
3. Record captures audio to one of 4 shared buffers (R1-R4)
4. Play back via MIDI, with pitch, start/end points, and bit-crushing

## Features

- Undithered signed 12-bit capture/playback, with correct full-scale gain
- 4 shared mono buffers (441,000 samples each: 10 seconds at 44.1 kHz)
- Real-time recording and playback across plugin instances
- RAM is not saved in presets/projects; recapture after a clean full teardown
- Separate bit-depth reduction (BRR), sample-and-hold rate reduction (SRR), and distortion
- Varispeed pitch with linear interpolation, start/end, hold/decay, and retriggers
- MIDI-triggered playback with velocity sensitivity
- Capture/Audition buttons, automatable Capture trigger, clickable R1–R4 tabs,
  per-instance activity/peak meters, and source-rate-aware waveform duration
- VST3 and CLAP formats

## Screenshots

### Ultrawave Record
![Ultrawave Record GUI](screenshots/ultrawave_record.png)

### Ultrawave Play
![Ultrawave Play GUI](screenshots/ultrawave_play.png)

## Building

Building requires SSH access to `EphemeralDSP/ephemeral`. Its allocation-free
Rust DSP is pinned to a Git revision; no sibling checkout or C++ build is needed.
Linux also needs the audio/X11 development packages listed in
`ci/scripts/deps-linux.sh` (that script installs system packages; inspect it first).

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/EphemeralDSP/ultrawave.git
cd ultrawave
cargo build --release

# Bundle plugins (both Record and Play)
cargo xtask bundle ultrawave-record --release
cargo xtask bundle ultrawave-play --release
```

Built plugins are placed in `target/bundled/`:
- `ultrawave-record.clap` / `ultrawave-record.vst3`
- `ultrawave-play.clap` / `ultrawave-play.vst3`

## Installation

Use a single Cargo command to build, bundle, and install both plugins:

```bash
cargo xtask install
```

This performs a user install by default:
- Linux: `~/.vst3` and `~/.clap`
- macOS: `~/Library/Audio/Plug-Ins/VST3` and `~/Library/Audio/Plug-Ins/CLAP`

For system-wide install paths, use:

```bash
cargo xtask install --system
```

If your system paths need elevated permissions, run the command with `sudo`.

## Plugin Formats

Current milestone scope targets macOS/Linux first. Windows support is out-of-scope for this milestone.

| Format | macOS | Linux |
|--------|-------|-------|
| VST3   | ✓     | ✓     |
| CLAP   | ✓     | ✓     |

## Test the sound without a DAW

```bash
./scripts/audition.sh
# Or supply your own 44.1 kHz WAV and an output directory:
./scripts/audition.sh /path/to/drums.wav target/my-audition
```

This builds the standalone binaries and writes six mono WAVs to `target/audition`:
12-bit capture, neutral playback, octave-down playback, BRR, SRR, and a driven
combination. It requires no audio device, MIDI controller, or display. Compare
against the input at matched listening levels; distortion raises loudness.
Offline rendering uses isolated RAM and cannot erase a live DAW's samples.

For individual renders:

```bash
target/release/ultrawave_record --input tests/vinyl909.wav --output target/capture.wav
target/release/ultrawave_play --input target/capture.wav --output target/crunch.wav \
  --pitch 52 --brr 32 --srr 0.5 --dist 16
cargo test --workspace
```

Offline Play accepts `--pitch`, `--brr`, `--dist`, `--start`, `--end`, `--hold`,
`--decay`, `--retrigger`, and `--retrig-time` as integers 0–127. Pitch 64 is unity;
52/76 are one octave down/up. `--srr` ranges from 0 to 0.9629. `--output-secs`
sets render length; use it for long retrigger tails. `--channel 0..3` selects RAM.
The WAV tools currently require **44.1 kHz** input and capture at most 10 seconds.

The tests check actual sample values, clipping, the last sample, CLI renders,
Play's processing callback with MIDI sample offsets and velocity, block-size
invariance, independent BRR/SRR controls, and cross-process shared-memory access.
They are software regression checks, not comparisons against hardware captures.

## Fast development loop

```bash
./scripts/dev-loop.sh fast           # Format check, tests, six renders + metrics
./scripts/dev-loop.sh fast --watch   # Repeat on repository input changes
./scripts/dev-loop.sh full --editor  # Also Clippy, release builds, validators,
                                    # benchmark and 10 editor lifecycle repetitions
./scripts/screenshot.sh             # Render both editors; inspect the PNGs
```

The warm fast loop took about **one second** on the development ThinkPad; an
incremental rebuild took about five seconds. Listen to `target/audition/*.wav`,
then inspect peak/RMS, neutral delta, and effect deltas printed by the script.
Failures stop the pass; watch mode waits for an edit before retrying. The full
loop took about 2.5 minutes. Validators belong on PATH or in `target/tools/`.

`cargo run --release --example dsp_bench` measures trigger cost for short/full
buffers and neutral/crunchy playback. This machine measured roughly 20 ns per
trigger, 18 ns/sample neutral and 22 ns/sample crunchy. These are local scalar
microbenchmarks, not whole-host CPU guarantees. MIDI triggers no longer copy RAM;
allocation tests cover the first trigger, recording, processing and stopping.
Pitch and distortion normalization are calculated outside the sample loop.
SIMD is deferred: the current mono SRR/envelope path is sequential and reads
atomic shared samples. Block conversion or future parallel voices are better
SIMD candidates; benchmark those before introducing architecture-specific code.

## Test in a DAW

1. Run `cargo xtask install` to build and install both CLAP and VST3 plugins into
   your user plugin directories. Rescan plugins in the DAW.
2. Put a short drum WAV on a source track and insert
   **Ultrawave Record**, or send the source audio to a separate Record track.
3. Select R1. Set MLEV 127, ILEV 0, MBAL centered, RATE HI, LEN 1 BAR.
   Click **Capture**, automate Capture from off to on, or send a MIDI note at
   the start of the source. Falling Capture does not cut a timed recording short.
   **Record is silent by default**. CUE1 adds the dry mono mix; CUE2 adds the
   quantized capture while recording. Both monitor gains range from 0 to 127.
4. Put **Ultrawave Play** on an instrument track, select R1, then trigger it with
   MIDI **after capture completes** (one bar is 2 seconds at 120 BPM).
5. Start with PTCH +0 st (raw 64), STRT 0%, END 100%, HOLD/DEC/RTRG/BRR/SRR/DIST zero,
   LEV 0 dB. Change pitch, then BRR and SRR separately. Check that soft MIDI notes
   are quieter and that changing buffer size does not shift the trigger.
6. Capture in real time before bouncing Play. Hosts may render independent tracks
   out of order during an offline bounce; the shared RAM is not a DAW routing edge.
   Saving a project does **not** save RAM: recapture after all instances reload.

For external capture, route a stereo send to Record's **sidechain**, set ILEV
above zero, and adjust IBAL. MLEV/MBAL control the main input; ILEV/IBAL control
the sidechain. Each bus is mixed to mono before LEV, SRR, DIST, RATE and 12-bit
storage. CUE1 bypasses those effects; CUE2 monitors what reaches RAM. Avoid routing
monitor output back into its own capture input unless you deliberately want feedback.

### Automated Renoise test (Linux, Renoise 3.5.4 installed)

```bash
cargo xtask bundle ultrawave-record --release
cargo xtask bundle ultrawave-play --release
./scripts/renoise-test.sh
```

The fixture uses Xvfb, an isolated HOME, and a unique shared-RAM key. It generates
a left-only 440 Hz source **inside Renoise**, captures one bar through the Record
track effect, renders Play R1 and empty R2, then opens/closes each editor five times.
The numerical checks verify a two-second 440 Hz capture, identical left/right
playback, silent capture monitoring, and zero R2 leakage. Artifacts, a generated
`.xrns`, logs and JSON metrics are in `target/renoise-test/`. This passed at 48 kHz
on 2026-09-19. Sidechain/balance/effects are covered by callback tests, **not yet by
a Renoise sidechain-routing fixture**. The test does not modify an open personal session.

`tests/test.RPP` is an old scratch session, **not** a verified ready-to-play test
project (it lacks source→Record routing). Use the routing steps above.

For live testing outside a DAW, run `target/release/ultrawave_record --help` and
`target/release/ultrawave_play --help` for audio/MIDI device options. Keep both
running on the same audio backend/sample rate. On Linux JACK/PipeWire, connect
source audio and MIDI to Record and MIDI to Play using your patchbay. A separate
`ULTRAWAVE_SHM_KEY=/uw-test` environment variable on **both** processes isolates
them from DAW instances. Without MIDI/audio routing, launching a window alone
does not exercise recording or playback.

## Fidelity and current limits

This is **UW-inspired, not a bit-exact Machinedrum emulation**. The current model
uses saturating 12-bit rounding, mono RAM, linear varispeed interpolation,
additional stepped BRR (12 down to 2 bits), sample-and-hold SRR, and tanh drive.
Quantization is level-dependent: quiet tails lose detail; hot input clips.
BRR changes amplitude resolution; SRR changes time resolution. Their parameter
curves, interpolation, envelope times, and drive are not calibrated to hardware.
`technical_specs.md` contains historical research with conflicting claims; it
must not be treated as proof of hardware behavior.

Play's synthesis controls latch on a trigger; pitch, end, BRR, SRR, distortion,
and level also update during playback. NoteOff does not choke a one-shot.
Record controls now affect the capture/monitor paths described above. The filter
code is not in the audible plugin chain. RAM stores its source rate, so 44.1/48/96
kHz playback preserves duration/pitch, including reduced recording RATE. There is
**no fixed hardware sample-clock model**. At RATE HI, capacity is 9.19 seconds at
48 kHz and 4.59 seconds at 96 kHz; the additional ten-second capture-time cap applies
at reduced RATE.

Writers are exclusive across processes: a competing capture is ignored, never
steals an active take. A new take invalidates an old playing voice rather than
mixing revisions; Play latches the available prefix when triggered. Shared-memory
layout v2 uses a new default key: restart all instances together after upgrading.
All instances using a key share R1–R4, even in different DAWs/projects.

Remaining limitations: Unix dead-writer recovery occurs when a new shared mapping
attaches, not in the audio callback. The shared active-instance counter is not yet
crash-safe, so a host crash can retain stale RAM until explicitly recaptured.
Project isolation, crash-safe lifecycle reset, hardware A/B calibration and a
verified REAPER/sidechain fixture remain open work. No hardware-fidelity claim is
made by the software tests.

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

### Validation

Ultrawave uses industry-standard plugin validators:

```bash
cargo xtask bundle ultrawave-record --release
cargo xtask bundle ultrawave-play --release
# Requires pluginval, clap-validator, and (Linux) xvfb-run on PATH:
PATH="$PWD/target/tools:$PATH" bash ci/scripts/validate.sh
```

Linux validation on 2026-09-19: both CLAP bundles passed (18 tests each, 3
unsupported tests skipped); VST3 pluginval strictness 5 and ten repetitions per
editor passed without the previous `BadDrawable` teardown panic. Scripts check
logs as well as exit status. Renoise capture/playback and five editor open/close
cycles per plugin passed. Both editors and the empty/playing states were rendered
and visually inspected. macOS and hardware A/B were not tested in this session.

See [docs/testing/plugin-validation.md](docs/testing/plugin-validation.md) for details.

## License

See [LICENSE](LICENSE) for details.
