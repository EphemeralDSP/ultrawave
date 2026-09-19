# Ultrawave

[Buildkite CI](https://buildkite.com/jmlynn/ultrawave)

Real-time RAM recording and playback plugins inspired by the Elektron Machinedrum UW RAM machines. Built with Rust and [NIH-plug](https://github.com/robbert-vdh/nih-plug).

## Overview

Ultrawave provides **two companion plugins** that share a global audio buffer:

| Plugin | Purpose | Audio I/O |
|--------|---------|-----------|
| **Ultrawave Record** | Captures audio to shared RAM buffer | Stereo In → (silent) |
| **Ultrawave Play** | Plays back from shared RAM buffer | MIDI In → Stereo Out |

Use them together in a DAW to sample and resample audio in real-time, similar to the Machinedrum UW's R1/R2 (record) and P1/P2 (play) machines.

### Workflow

1. Insert **Ultrawave Record** on a track to capture audio
2. Insert **Ultrawave Play** on an instrument track
3. Record captures audio to one of 4 shared buffers (R1-R4)
4. Play back via MIDI, with pitch, start/end points, and bit-crushing

## Features

- Undithered signed 12-bit capture/playback, with correct full-scale gain
- 4 shared buffers (up to 10 seconds each)
- Real-time recording and playback across plugin instances
- Volatile RAM lifecycle: shared buffers are intentionally not persisted in
  plugin state/presets and are cleared after full plugin teardown/reload
- Separate bit-depth reduction (BRR), sample-and-hold rate reduction (SRR), and distortion
- Varispeed pitch with linear interpolation, start/end, hold/decay, and retriggers
- MIDI-triggered playback with velocity sensitivity
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

## Test in a DAW

1. Run `cargo xtask install` to build and install both CLAP and VST3 plugins into
   your user plugin directories. Rescan plugins in the DAW.
2. Use a **44.1 kHz** session. Put a short drum WAV on a source track and insert
   **Ultrawave Record**, or send the source audio to a separate Record track.
3. Select R1. Set MLEV fully up (display `+63`, raw 127), MBAL centered, RATE HI,
   and LEN 1 BAR. Route a MIDI note to Record at the start of the source.
   **Record's output is silent**; monitor the source on a separate dry route.
4. Put **Ultrawave Play** on an instrument track, select R1, then trigger it with
   MIDI **after capture completes** (one bar is 2 seconds at 120 BPM).
5. Start with PTCH 0 (raw 64), STRT 0%, END 100%, HOLD/DEC/RTRG/BRR/SRR/DIST zero,
   LEV 0 dB. Change pitch, then BRR and SRR separately. Check that soft MIDI notes
   are quieter and that changing buffer size does not shift the trigger.
6. Capture in real time before bouncing Play. Hosts may render independent tracks
   out of order during an offline bounce; the shared RAM is not a DAW routing edge.
   Saving a project does **not** save RAM: recapture after all instances reload.

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
Record's ILEV/IBAL, CUE controls, and sidebar effects remain unimplemented; use
MLEV/MBAL and the **Play** effects for now. The filter code is not in the audible
plugin chain. Shared RAM has no source-rate metadata or fixed 44.1 kHz internal
clock; use matching rates. Playback still copies RAM on MIDI triggers, and
concurrent writers across separate host processes are not arbitrated. Use one
Record per buffer. Hardware A/B captures and real-time ownership/copy improvements
are the next fidelity and reliability work.

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

### Validation

Ultrawave uses industry-standard plugin validators:

```bash
cargo xtask bundle ultrawave-record --release
cargo xtask bundle ultrawave-play --release
# Requires pluginval, clap-validator, and (Linux) xvfb-run on PATH:
ULTRAWAVE_SHM_KEY=/uw-validation bash ci/scripts/validate.sh
```

Linux validation on 2026-09-19: both CLAP bundles passed (18 tests each, 3
unsupported tests skipped). VST3 pluginval at strictness 5 returned exit 0 but
logged a baseview `BadDrawable` panic during editor teardown. Treat this as an
open GUI compatibility failure, not a clean VST3 certification. Standalone
screenshots render correctly; no actual DAW session or hardware A/B is verified.

See [docs/testing/plugin-validation.md](docs/testing/plugin-validation.md) for details.

## License

See [LICENSE](LICENSE) for details.
