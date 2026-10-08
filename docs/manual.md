# Playing Ultrawave

[Back to Ultrawave](../README.md) · [Download releases](https://github.com/EphemeralDSP/ultrawave/releases)

Ultrawave is an audio effect, not a separate sampler instrument. Each instance
captures and plays one mono sample. Put it on the track you want to sample, or
route audio to it through a bus or sidechain.

## Installation

Release packages contain VST3 and CLAP plugins, not an installer or standalone
application. Choose the ZIP for your operating system and CPU from the
[releases page](https://github.com/EphemeralDSP/ultrawave/releases), when available.

Close your DAW before installing or replacing a plugin. Extract the ZIP and copy
the whole `ultrawave-plugin.vst3` bundle or `ultrawave-plugin.clap` into the
appropriate folder, then reopen your DAW and rescan plugins.

| Platform | VST3 folder | CLAP folder |
| --- | --- | --- |
| Windows x86-64 | `C:\Program Files\Common Files\VST3` | `C:\Program Files\Common Files\CLAP` |
| macOS Apple Silicon | `~/Library/Audio/Plug-Ins/VST3` | `~/Library/Audio/Plug-Ins/CLAP` |
| Linux x86-64 | `~/.vst3` | `~/.clap` |

Windows system folders may require administrator permission. Mac builds are
ad-hoc signed, not notarized; Windows builds are unsigned. OS trust prompts may
occur. There is no Intel Mac/universal or AU build.

Linux release-worker builds use Ubuntu 24.04 with glibc 2.39; compatibility with
older distributions is unverified. The current plugin does not overwrite or
load presets from the old Record/Play pair.

## Record and play

1. Insert **Ultrawave** on an audio track, group, send bus or master.
2. Play audio through it. Leave **Main + sample** at 100% and **Sidechain** at 0%.
3. Press **Record**. The duration dropdown defaults to **1 bar**.
4. When recording finishes, press **Play**. MIDI note **48** records; **50** plays.
   Change these in the **MIDI** dropdown. Note numbers are definitive because
   DAWs use different octave names.

Capture and Playback automation also work: use an Off → On edge for each command.
There is no automatic access to the DAW master. Insert Ultrawave on the master,
or route that mix to its track.

### Resample or Sample only

**Resample** is the default. It passes dry stereo input through and adds sample
playback. Recording captures the configured input mix and the processed sample
playback, so each take can become the source for the next.

The old take stays playable while recording. When capture finishes, the new
take replaces it and stops old playback and repeats. Press Play or send another
Play Note to hear the replacement; it does not start automatically.

An interrupted recording keeps its nonempty prefix. An empty attempt retains
the old take. **Record once** prevents replacement until re-armed.

**Sample only** records input only and outputs only the sample. Use it on a
dedicated send receiver to avoid doubling the original source. It replaces the
old take and stops playback as soon as capture starts.

Playback triggered during a Sample only capture uses the written prefix at that
instant. Changing modes during a take ramps the routing, but the take keeps the
storage strategy selected at its start.

### Inputs and Record FX

**Inputs** contains **Main + sample** in Resample mode (or **Main input** in
Sample only), **Sidechain**, and their stereo balances. These mix to mono before
Gain and signed 12-bit quantization.

Input controls affect recording only, never audible dry output or playback level.
**Record FX** contains stored sample rate, rate crush (SRR), and drive. Rate crush
and drive are printed into each new recording.

Playback has independent level, rate crush, bit crush (BRR) and drive controls.
Start quietly and leave headroom: drive raises loudness, and the processed sample
can briefly exceed unity.

## MIDI and DAW routing

### Renoise: use an FX alias

1. Add Ultrawave to the source track's **Track DSP** chain.
2. In an empty instrument slot's **Plugin** tab, choose the **FX Alias** entry for
   that exact Ultrawave effect. It sends notes to the existing effect; it does
   not create another sampler or RAM bank.
3. Enter **C-4 (48)** to record and **D-4 (50)** to play. Put the playback note
   after the capture duration. Notes can be on another pattern track; audio
   comes out of the effect's track.
4. Enable **Record once** to keep the first take when the pattern loops.
   **Re-arm** allows another capture without clearing the current take.

For a mix of tracks, use a send/group receiver. A normal `#Send` feeds its main
input. Renoise's `#Sidechain` feeds its sidechain: choose the receiver track and
Ultrawave device, then raise Sidechain in Ultrawave.

Put a sidechain receiver **after the sending tracks**: Renoise refuses sends to
previous tracks. Multiple sidechain senders sum. Choose Sample only on a
dedicated receiver to avoid doubling the source.

For master resampling, put Ultrawave on the **Master** track with Resample
selected. Capture/Playback automation works there without an FX alias.

### Ableton Live and other hosts

Insert the VST3 effect on an audio/group track. On a MIDI track, choose that audio
track in **MIDI To**, then **Ultrawave** as the destination device. Send notes
48/50 or use host automation.

Choose a source in Live's plugin sidechain panel when using Sidechain. This is
standard MIDI-to-effect routing; Live execution is not yet verified here.
Live does not host CLAP natively.

Hosts without MIDI-to-effect routing can use Capture and Playback automation.
Real-host checks cover Renoise VST3 on Linux and REAPER VST3 on Apple Silicon
macOS. Windows validation is not a Windows DAW test.

## Shape the take

- **12-bit capture:** undithered, saturating signed samples. Quiet tails lose
  precision; hot inputs clip. Bit crush reduces playback resolution further,
  down to 2 bits.
- **Rate crush:** sample-and-hold time reduction, independent of bit crush.
  Drive uses tanh distortion. Pitch is varispeed with linear interpolation.
- **Start and End:** drag the waveform handles or their knobs to select a slice.
  Cross them for reverse playback. Equal markers are silent. Direction and
  start latch on a trigger; end stays live within that direction.
- **MIDI:** velocity controls playback loudness, not pitch. Pitch comes from the
  Pitch knob. NoteOff and zero-velocity notes do not choke a one-shot. Other note
  numbers are ignored; if Record and Play share a note, Record wins.
- **Hits:** total hits, including the first. Choose **1 hit** to cancel pending
  hits without cutting the current one. A new note replaces the train. Host
  stop/reset stops recording and playback, preserving RAM.

### Retrigger timing

Hits follows the hardware's nonlinear range. Raw MD values 0/1 play once,
2–63 give that many hits, 96 gives 104, and 126 gives 156. Raw 127 is **INF**.

Interval follows host tempo, with a 120 BPM fallback. The minimum is not off:
it reaches about 1378 hits/second. The maximum is a sixteenth note; intermediate
settings are not exact note divisions.

The editor displays and accepts milliseconds at the current tempo. Generic host
parameter text remains the raw MD 0–127 code because it has no tempo context.
Fractional intervals retain their remainder; tempo changes preserve progress.

See the [hardware measurements](testing/machinedrum-measurement.md) for the
firmware mapping, measured rate limit and deliberate omission of hardware block
jitter and smoothing.

## Keep a take—or start again

**Record once** captures on the first trigger for the chosen duration or capacity,
then shows **Protected** and ignores further capture triggers. **Re-arm**, or a
rising Re-arm automation edge, allows another take.

Re-arming waits for the next trigger and is ignored during capture. Disabling
Record once restores repeated recording; enabling it during a take protects
that take. Transport stop/start does not re-arm.

**Projects save controls, not audio or the Record once runtime latch.** A newly
loaded instance starts with empty RAM and an armed recorder. Record or render
the output in your DAW before closing the project if you want to keep it.

An existing instance keeps RAM across transport resets. Restoring or resetting
a held-high Capture, Playback or Re-arm control is not a new trigger. Automate
Off → On; MIDI triggers and editor buttons remain available.

Each instance owns separate RAM. There are no banks, pairing, shared-memory
files, cross-project samples or recovery workers.

## Editor shortcuts and readouts

Record and Play share one 920×650 pane. The waveform dims excluded audio and
shows playback direction. Start/End handles and knobs edit the same parameters;
both support crossing for reverse playback.

Drag knobs vertically, Shift-drag for fine changes, and double-click to restore
defaults. Click numeric values to type. MIDI assignments live in the header's
MIDI panel, not among the sound controls.

The Record panel shows Ready/Recording/Protected status, recording progress,
elapsed/total time and Gain. Re-arm appears only after a protected take finishes.
Inputs and Record FX open in popovers.

Beat-based durations show the effective cap when needed: ten seconds or the
available RAM at the selected sample rate, whichever is shorter. Effective
seconds also appear beside the status. Actual timing is latched when recording
starts, so a later tempo, duration or capture-rate change does not retime that take.

Record FX sample rate is shown in kHz and accepts kHz or Hz. Generic host-facing
Capture Rate text remains the divisor **÷1…÷16**. Source, drive and rate-crush
readouts are percentages; Bit crush reports the effective bit depth.

## Capacity and sound

Each instance holds 441,000 samples: 10 seconds at 44.1 kHz, 9.19 at 48 kHz,
or 4.59 at 96 kHz with Capture rate HI. Reduced-rate recording also observes the
ten-second capture-time cap.

Timed capture lengths are 1/2, 1, 2, 4, 8, 16 or 32 quarter-note beats; the last
four appear as 1/2/4/8 bars. Maximum fills available capacity. RAM stores its
source rate so 44.1/48/96 kHz playback preserves duration and pitch.

**UW-inspired, not bit-exact Machinedrum emulation.** Quantization, interpolation,
envelopes and drive remain approximations. Retrigger counts and intervals follow
original firmware and hardware measurements.

A fixed 17 Hz low cut models measured low-frequency playback loss. It affects
only the wet sample, including each resampling generation; dry-through is
unchanged. State continues across notes and the short tail, clearing on host reset.

Offline playback uses the same path. No complete DAC EQ, treble/interpolation
match or fixed hardware sample clock is claimed. Exported hardware RAM disproved
the initial capture-side treble-filter hypothesis; that filter is not included.
