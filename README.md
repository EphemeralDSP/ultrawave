# Ultrawave

[Buildkite CI](https://buildkite.com/jmlynn/ultrawave)

Real-time RAM recording and playback plugins inspired by the Elektron Machinedrum UW RAM machines. Built with Rust and [NICE-plug](https://codeberg.org/RustAudio/nice-plug), with the official egui editor adapter.

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
- RAM is not saved in presets/projects; recapture after all instances close or crash
- Record One Shot captures once until re-armed; its mode is saved, its latch is volatile
- Separate bit-depth reduction (BRR), sample-and-hold rate reduction (SRR), and distortion
- Varispeed pitch with linear interpolation, start/end, hold/decay, and retriggers
- Reverse playback when STRT is greater than END
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

Building requires Rust 1.95 or newer and SSH access to `EphemeralDSP/ephemeral`.
Its allocation-free Rust DSP is pinned to a Git revision; no sibling checkout or C++ build is needed.
Linux also needs the audio/X11 development packages listed in
`ci/scripts/deps-linux.sh` (that script installs system packages; inspect it first).

NICE-plug/core 0.4.2, nice-plug-egui 0.5.1 and nice-plug-xtask 0.1.1 are pinned.
The framework is ISC-licensed; the VST3 bindings use MIT/Apache-2.0. The build no
longer depends on NIH-plug, Vizia or GPL-licensed `vst3-sys`. See
[`vendor/README.md`](vendor/README.md) for the VST3 immediate-save and X11 teardown patches.
Plugin IDs, parameter IDs/types and audio layouts are unchanged; existing project
controls remain compatible. RAM is still deliberately volatile.

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
target/release/ultrawave_play --input target/capture.wav --output target/reverse.wav \
  --start 127 --end 0
target/release/ultrawave_play --input target/capture.wav --output target/retrig.wav \
  --retrigger 127 --retrig-time 127 --tempo 120 --output-secs 4
cargo test --workspace
```

Offline Play accepts `--pitch`, `--brr`, `--dist`, `--start`, `--end`, `--hold`,
`--decay`, `--retrigger`, and `--retrig-time` as integers 0–127. Pitch 64 is unity;
52/76 are one octave down/up. `--srr` ranges from 0 to 0.9629. `--output-secs`
sets render length; use it for long retrigger tails. `--channel 0..3` selects RAM.
`--tempo` sets a positive BPM (default 120) for offline retriggers. Infinite
retriggering still renders only the requested duration; it cannot hang the CLI.
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

Both loops include VST3 immediate-save/automation regressions. Run those alone
with `./scripts/test-vst3-state.sh` (or append `--release`).
They also build and load the actual CLAP cdylibs and run the bounded,
isolated audio/state harness in `./scripts/test-clap-audio.sh`. The harness uses
fresh shared-memory names and removes its mappings and lease directories. It
checks four-lane main/sidechain routing, exact MIDI/automation offsets, reverse,
finite/infinite/OFF retriggers at two tempos, saved controls and bank isolation
at 44.1/48/96 kHz. The full loop uses release libraries (`--release`). It
documents current transport behavior: a playing-to-stopped edge and reset stop
voices, while host seek position and loop metadata do not restart or stop them.
The warm fast loop takes about **eight seconds** on the development ThinkPad with
debug symbols and incremental compilation disabled; first builds take longer.
Listen to `target/audition/*.wav`,
then inspect peak/RMS, neutral delta, and effect deltas printed by the script.
Failures stop the pass; watch mode waits for an edit before retrying. The full
loop also runs the plugin validators, which belong on PATH or in `target/tools/`.

`cargo run --release --example dsp_bench` compares the original per-sample capture
mix with the SIMD capture path, including SRR, distortion, atomic 12-bit storage
and monitoring. On the development ThinkPad, seven-run medians measured about
**11.3 → 8.2 ns/sample neutral** and **21.5 → 17.3 ns/sample driven** (roughly
28% and 20% less CPU time). Playback remains about 18/22 ns/sample neutral/crunchy;
trigger cost is about 20 ns regardless of buffer length. These are local DSP-path
measurements, not whole-host CPU guarantees. Rerun on the target machine.

Record mixes four time samples at once using SSE2 on x86-64 and NEON on enabled
AArch64 targets, with a scalar fallback/tail. It uses bounded stack chunks, keeps
the original floating-point sum order, and does not approximate tanh or change
12-bit rounding. Stateful SRR, MIDI timing and envelopes remain sequential.
Debug/release tests verify bit-exact scalar equivalence, unaligned/tail buffers,
and MIDI around chunk/host-block boundaries. SSE2 was executed here; NEON awaits
an ARM run. Allocation tests cover mixing, first trigger, recording and stopping.

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

**Flip STRT and END to play backwards.** STRT 100%, END 0% reverses the whole
sample; inverted interior markers reverse a slice. Equal markers are silent.
Direction is chosen on each note or retrigger. STRT is latched on the note;
END remains live within the chosen direction. Pitch, retriggers, envelopes and
12-bit effects work in either direction.

For external capture, route a stereo send to Record's **sidechain**, set ILEV
above zero, and adjust IBAL. MLEV/MBAL control the main input; ILEV/IBAL control
the sidechain. Each bus is mixed to mono before LEV, SRR, DIST, RATE and 12-bit
storage. CUE1 bypasses those effects; CUE2 monitors what reaches RAM. Avoid routing
monitor output back into its own capture input unless you deliberately want feedback.

### Tempo-synced retriggers

RTRG 1–126 adds that many repeats after the initial note; 127 displays **INF**.
RTRG 0 or RTIM 0 disables repeats. A repeat restarts the selected region and its
envelope, forward or backward, including after a short sample has ended.
RTIM follows host tempo; missing/invalid tempo falls back to 120 BPM. Fractional
frame intervals retain their remainder rather than drifting on each repeat.
Tempo changes preserve progress through the current interval.

| RTIM raw value | Note interval |
|---|---|
| 127 | 1/16 |
| 109 | 1/16 triplet |
| 96 | 1/32 |
| 77 | 1/32 triplet |
| 64 | 1/64 |
| 44 | 1/64 triplet |
| 32 | 1/128 |

These and the other reported landmarks follow Legofriendly's table in
[Tarekith's Machinedrum tips](https://tarekith.com/assets/machinedrum_tipsandtricks.htm#Reference),
not a firmware-derived timing law. Intermediate values use logarithmic
interpolation; below 32 the doubling-per-32 trend is extrapolated. Hardware
calibration is still needed. The editor shows musical lengths at the listed
values and quarter-note beats (`bt`) elsewhere.

Count/time controls latch on a new note, except setting either to OFF cancels
pending repeats immediately without cutting the current hit. A new note replaces
the old train. Host transport stop, plugin reset, or replacement of the sampled
RAM cancels it; auditioning while transport is stopped is still available.
NoteOff does not choke playback. **Compatibility:** saved parameter IDs and raw
values are unchanged, but RTIM no longer uses the old 0–500 ms approximation,
and RTRG 127 now means infinite. Existing retrigger patches will sound different.

### Record once, then keep the take

Enable **ONE SHOT**, then use Capture, MIDI or Capture automation as usual. The
first successful trigger records for LEN (or until capacity); further triggers
cannot restart or replace that take. The status moves from **ARMED** through
**RECORDING** to **LOCKED**. This automates muting the record trigger after one
pass without muting Play or changing CUE monitoring.

Click **RE-ARM** after recording to allow one more capture. Arming does not erase
RAM or start recording; it waits for the next trigger. Re-arm is unavailable
during capture. Switching One Shot off restores repeated-trigger behavior;
switching it back on arms it again. Enabling it during a take protects that take.
A rejected trigger on a busy buffer does not consume the arm.

The lock belongs to this Record instance: transport stop/start, editor reopening
and selecting another R1–R4 buffer do not re-arm it. Stopping transport mid-take
can leave a partial take locked; use Re-arm to replace it. Other Record instances
can still write the same buffer when no writer owns it.

**Projects/presets save controls, not samples or the One Shot latch.** After a
full shutdown RAM is empty, and a fresh Record instance with One Shot enabled
starts armed. No sample files or RAM snapshots are embedded in the project.

### Pairing Record and Play

Set **BANK** to the same number (1–16) in Record and Play to share R1–R4. BANK
is saved with plugin state but is intentionally not automatable. The default is
**OFF**: every new instance has private RAM and cannot accidentally hear another
instance. Different banks are isolated. Choosing the same bank in another
project deliberately joins that RAM; bank numbers are not project identities.
Samples remain volatile, and fully closing a bank clears it when it is next
opened even while other banks remain active. The footer shows `B…` while
connecting and `B!` if attachment failed; failure uses disconnected private RAM
rather than pretending pairing succeeded. Standalone/screenshot runs may set
`ULTRAWAVE_BANK=1`; `ULTRAWAVE_SHM_KEY` remains the namespace prefix for all
banks.

To sample a single track, several tracks, or a master mix, route that source to
Record's main or sidechain input in the DAW. Ultrawave does not implicitly tap the
DAW's master; the DAW send/bus routing supplies the equivalent of the hardware's
master or analog input. Use a separate capture bus to avoid silencing your master
with Record's default-silent output.

### Automated Renoise test (Linux, Renoise 3.5.4 installed)

```bash
cargo xtask bundle ultrawave-record --release
cargo xtask bundle ultrawave-play --release
./scripts/renoise-test.sh
```

The fixture uses Xvfb, an isolated HOME, and a unique shared-RAM key. It generates
a left-only 440 Hz source **inside Renoise**, captures one bar through the Record
track effect, and verifies that repeated One Shot triggers with an 880 Hz source
leave that take bit-for-bit unchanged. Reversed STRT/END must render every frame
of that take in reverse order. R1/R2 routing, dual-mono output, monitoring, and
five editor open/close cycles are also checked. Retrigger renders compare every
sample at 60/120 BPM, with RTIM off, and through 256 infinite-mode hits.

It then saves non-default controls, terminates that host, and opens the project
in a fresh process with the **same RAM key**. Controls must survive, RAM must be
silent, and One Shot must allow a fresh capture. It saves immediately after edits,
without an intervening render to flush VST3 parameter changes. Requires `Xvfb`,
`xdotool`, ImageMagick and Python on PATH. Artifacts, `.xrns`, logs and metrics are in `target/renoise-test/`.
This passed at 48 kHz on 2026-09-19. Sidechain/balance/effects also have callback
tests, **not yet a Renoise sidechain-routing fixture**. No open personal session
is modified. This fixture exercises VST3; CLAP state/processing use clap-validator.

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
Record controls affect the capture/monitor paths described above.
RAM stores its source rate, so 44.1/48/96
kHz playback preserves duration/pitch, including reduced recording RATE. There is
**no fixed hardware sample-clock model**. At RATE HI, capacity is 9.19 seconds at
48 kHz and 4.59 seconds at 96 kHz; the additional ten-second capture-time cap applies
at reduced RATE.

Writers are exclusive across processes: a competing capture is ignored, never
steals an active take. A new take invalidates an old playing voice rather than
mixing revisions; Play latches the available prefix when triggered. Shared-memory
**layout v3 uses a new default key**: update Record and Play together, restart all
instances and recapture. If setting `ULTRAWAVE_SHM_KEY`, choose a fresh key rather
than reusing a v2 mapping. Incompatible layouts are never resized or overwritten.
All instances using a key still share R1–R4, even in different DAWs/projects.

RAM lifecycle uses OS-held leases instead of persistent counters or PID checks:

- The first plugin after every participant closes/crashes starts with empty RAM.
  Simultaneous first registrations serialize initialization and clearing.
- If another host remains open, a background worker checks abandoned sessions
  every 100 ms (subject to OS scheduling). It discards unfinished takes owned by
  the dead session, preserving other live writers and completed takes. Trigger
  capture again after recovery; an earlier rejected trigger is not replayed.
- A paused/live owner is never stolen because of a timeout. Crashes during file
  sizing or initialization can also be recovered by the next loader.
- RAM access on the audio thread remains atomic-only: no filesystem calls,
  mutexes, allocation or worker startup. Final lease drop joins the worker before
  library unload; final registry drop releases the mapping and descriptors.

Unix lease files live in the private `/tmp/ultrawave-<uid>/<key>/` directory.
They intentionally persist; **do not remove them while any instance is open**.
A shared directory lock protects active namespaces from systemd temporary-file
aging. There are 64 concurrent registry sessions per key; instances in one loaded
plugin module share a session. Core-library callers using shared RAM must retain
the guard returned by `register_instance()` through processing; isolated offline
registries do not need a lease.

Linux tests exercise real process termination, a stopped live owner, recovery in
an already-attached survivor, concurrent first loads, odd revisions, interrupted
initialization, incompatible layouts and orderly reload. This is crash-safe
coordination, **not sample persistence or hardware calibration**. macOS/ARM
verification, project isolation, hardware A/B and a verified REAPER/sidechain
fixture remain open work.

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

NICE-plug migration validated on Linux x86-64, 2026-09-19: 83 workspace tests and
9 wrapper tests pass in debug/release, along with fmt, Clippy and six offline
renders. Both CLAP bundles pass (18 tests each, 3 unsupported tests skipped);
VST3 pluginval strictness 5 and ten repetitions per editor pass without teardown
warnings or panics. Scripts check logs as well as exit status. Renoise validates
capture/routing, immediate-save/reopen and pre-migration project compatibility.
Both editors, trimmed waveform markers and Record armed/recording/locked/rearmed
states were rendered and inspected. macOS and hardware A/B remain unverified.

See [docs/testing/plugin-validation.md](docs/testing/plugin-validation.md) for details.

## License

See [LICENSE](LICENSE) for details.
