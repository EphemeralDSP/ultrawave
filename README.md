# Ultrawave

[Buildkite CI](https://buildkite.com/jmlynn/ultrawave)

A MIDI-controlled **audio effect** for real-time 12-bit RAM sampling, inspired by
the Elektron Machinedrum UW. One instance records and plays its own four mono
buffers. Built in Rust with [NICE-plug](https://codeberg.org/RustAudio/nice-plug)
and egui; available as VST3, CLAP, and a standalone application.

## Record and play in one instance

1. Insert **Ultrawave** on the audio track, group, send bus, or master you want to sample.
2. Select R1. Leave MAIN at 127, SIDE at 0, balances centered, RATE at HI, LEN at 1 BAR.
3. Send MIDI note **48** to record, then note **50** to play after capture finishes.
   These are MIDI numbers, because DAWs use different octave names. Both are
   assignable on the Record page. The Capture/Audition buttons and automatable
   Capture/Playback rising edges provide alternatives to MIDI.
4. The default **DRY + PLAYBACK** output preserves incoming stereo and adds the
   mono sample to both channels. Select **PLAYBACK ONLY** on a capture/send bus
   when the source is already audible elsewhere. Switching output modes ramps
   dry gain over 5 ms; neither mode changes what is recorded.

The effect reads main/sidechain input **before adding playback**, so it does not
implicitly record itself. There is no automatic access to the DAW master:
insert on the master or route that mix to the effect. MAIN/BAL control the main
input; SIDE/BAL control the optional stereo sidechain. These mix to mono before
Capture LEV, SRR, DIST, RATE and signed 12-bit quantization. Capture controls do
not color dry-through. Playback has its own level, SRR and distortion controls.

One shared selector chooses the buffer for the **next** record/play trigger.
An existing take or voice keeps its latched buffer, so R2 can be recorded while
R1 plays. A new take in the same buffer invalidates its old playing voice.
Playback triggered during recording uses the available prefix at that instant.

### Renoise: use an FX alias, not a second plugin

1. Add **Ultrawave** to the source track's **Track DSP** chain.
2. In an empty instrument slot's **Plugin** tab, choose the **FX Alias** entry
   for that exact track's Ultrawave effect. The slot only sends notes to the
   existing effect; it does not create another sampler or another RAM bank.
3. Enter **C-4 (48)** to record and **D-4 (50)** to play using that alias
   instrument. Place the playback note after LEN. Notes may be on a separate
   pattern track; the sample audio comes out of the effect's track.
4. Enable ONE SHOT to keep the first take when the pattern loops. RE-ARM allows
   another capture without clearing the current take.

For a mix of tracks, use a send/group receiver with Ultrawave. A normal `#Send`
feeds its **main input**. Renoise's `#Sidechain` device feeds its **sidechain**;
choose the receiver track and Ultrawave device, then raise SIDE. Put a sidechain
receiver **after the sending tracks**: Renoise refuses sends to previous tracks.
Multiple sidechain senders sum. Use Playback Only if monitoring the sources
elsewhere; leave dry-through enabled for an ordinary insert or master effect.

### Ableton Live and other hosts

Insert the VST3 effect on an audio/group track. On a MIDI track, choose that
audio track in **MIDI To**, then **Ultrawave** as the destination device. Send
48/50 notes or use host automation. Select a source in Live's plugin sidechain
panel when using SIDE. This is standard MIDI-to-effect routing; Live execution
has not yet been verified here. Hosts without MIDI-to-effect routing can use
Capture and Playback automation instead. Live does not host CLAP natively.

## Character and playing controls

- Undithered, saturating signed **12-bit** capture: quiet tails lose precision,
  hot inputs clip. BRR reduces playback resolution further, down to 2 bits.
- SRR is sample-and-hold time reduction; it is independent of BRR. Distortion
  uses tanh drive. Pitch is varispeed with linear interpolation.
- **Cross STRT and END for reverse playback.** Inverted interior markers reverse
  a slice; equal markers are silent. Direction/start latch on a trigger; end
  remains live within that direction. Pitch is set by PTCH, not the trigger note.
- Playback velocity controls loudness. NoteOff and zero-velocity notes do not
  choke a one-shot. Other note numbers are ignored. If both note assignments
  are equal, Record takes priority; use different notes for separate commands.
- RTRG 1–126 adds that many repeats; 127 is **INF**. RTRG 0 or RTIM 0 cancels
  repeats without cutting the current hit. A new note replaces the old train.
  Host stop/reset stops recording and playback, preserving RAM.
- RTIM follows host tempo (120 BPM fallback). Values 127/109/96/77/64/44/32 are
  1/16, 1/16T, 1/32, 1/32T, 1/64, 1/64T, 1/128. Fractional intervals retain their
  remainder; tempo changes preserve interval progress. Reported landmarks come
  from [Tarekith's tips](https://tarekith.com/assets/machinedrum_tipsandtricks.htm#Reference),
  not a measured firmware timing law.

### One Shot and project saving

ONE SHOT records on the first trigger for LEN (or capacity), then protects that
take from subsequent triggers. RE-ARM, or a rising Re-arm automation edge,
allows another take. Re-arming waits for the next trigger and is ignored during
capture. Disabling One Shot restores repeated recording; enabling it during a
take protects that take. Transport stop/start and buffer selection do not re-arm.

**Projects save controls, not audio or the One Shot runtime latch.** A newly
loaded instance starts with empty RAM and an armed recorder. An existing
instance keeps RAM across transport resets. Restoring or resetting a held-high
Capture, Playback or Re-arm control does not count as a new trigger: automate
Off → On for a new command. MIDI triggers and editor buttons remain available.
Every instance owns separate RAM:
no banks, pairing, shared-memory files, cross-project leaks, or recovery workers.
This replaces the old Record/Play pair; old plugin IDs/presets are not supported.

## Editor

Record and Play are pages of the same 760×460 monochrome editor. Click numeric
values to type; drag waveform markers for a region or reverse playback.

![Record page](screenshots/ultrawave_record.png)
![Play page](screenshots/ultrawave_play.png)

## Build and install

Requires Rust 1.95+ and SSH access to the pinned `EphemeralDSP/ephemeral` DSP
dependency. Linux development packages are listed in `ci/scripts/deps-linux.sh`.
NICE-plug/core 0.4.2 and egui adapter 0.5.1 are pinned. The framework is ISC;
VST3 bindings are MIT/Apache-2.0. See [vendor patches](vendor/README.md).

```bash
cargo build --release
cargo xtask bundle ultrawave-plugin --release
# Produces target/bundled/ultrawave-plugin.{vst3,clap}
cargo xtask install
```

Install destinations: `~/.vst3` / `~/.clap` on Linux, and
`~/Library/Audio/Plug-Ins/{VST3,CLAP}` on macOS. Close hosts before replacing a
loaded binary, then rescan. The new bundle does not overwrite the old pair.

## Fast offline iteration

```bash
./scripts/dev-loop.sh fast          # Tests, exported CLAP audio/state, VST3 state, WAVs
./scripts/dev-loop.sh fast --watch  # Repeat after source changes
./scripts/dev-loop.sh full --editor # Clippy, release tests, validators, benchmark, UI lifecycle
./scripts/screenshot.sh            # Both pages; inspect screenshots/*.png
./scripts/audition.sh              # Six deterministic WAVs + audio metrics, no DAW/device
```

The CLAP harness loads the actual cdylib and checks routing, timing, MIDI note
assignment, One Shot, reverse/retrigger, saved controls, private RAM and instance
recreation at 44.1/48/96 kHz. These are software regressions, not hardware A/Bs.
Failures stop the pass. Validators belong on PATH or in `target/tools/`.

Individual offline renders use the same DSP and explicit private RAM:

```bash
target/release/ultrawave record --input tests/vinyl909.wav --output target/capture.wav
target/release/ultrawave play --input target/capture.wav --output target/crunch.wav \
  --pitch 52 --brr 32 --srr 0.5 --dist 16
target/release/ultrawave play --input target/capture.wav --output target/reverse.wav \
  --start 127 --end 0
target/release/ultrawave play --input target/capture.wav --output target/retrig.wav \
  --retrigger 127 --retrig-time 127 --tempo 120 --output-secs 4
```

WAV tools require 44.1 kHz input. Integer synthesis controls range 0–127; pitch
64 is unity, 52/76 one octave down/up. SRR ranges 0–0.9629. `--output-secs` bounds
even infinite retrigger renders. Start listening quietly: drive raises loudness.
For live standalone audio/MIDI device options use `target/release/ultrawave --help`.
On Linux JACK/PipeWire connect source audio and MIDI through your patchbay.

### Actual Renoise regression

```bash
cargo xtask bundle ultrawave-plugin --release
./scripts/renoise-test.sh
```

Requires Renoise 3.5.4, Xvfb, xdotool, ImageMagick and Python. Uses a disposable
HOME/display and only the newly built bundle, leaving personal sessions and
installed plugins untouched. Tests FX-alias notes, main/sidechain routing,
dry-through/playback-only, One Shot, reverse/retrigger, project save and process
reopen. Artifacts and logs: `target/renoise-test/`. This exercises VST3; the CLAP
harness and clap-validator exercise CLAP. `tests/test.RPP` remains unverified
historical scratch, not a supported setup.

## Fidelity and platform limits

**UW-inspired, not bit-exact Machinedrum emulation.** Quantization, interpolation,
envelopes, drive and intermediate retrigger curves still need hardware captures.
No measured DAC equalization or fixed hardware sample-clock model is claimed.
RAM stores its source rate; 44.1/48/96 kHz playback preserves duration and pitch.
Each buffer has 441,000 samples: 10 s at 44.1 kHz, 9.19 s at 48 kHz, 4.59 s at
96 kHz at RATE HI. Reduced RATE also observes the ten-second capture-time cap.

Capture mixing uses SSE2 on x86-64 and NEON on supported AArch64 targets with
scalar tails/fallback. Stateful SRR/envelopes remain sequential. Allocation
checks and scalar equivalence tests protect the real-time path; run
`cargo run --release --example dsp_bench` for local measurements. Linux x86-64
is the exercised platform. macOS/ARM, Live, and hardware A/B remain to be verified.

See [CONTRIBUTING.md](CONTRIBUTING.md) and [LICENSE](LICENSE).
