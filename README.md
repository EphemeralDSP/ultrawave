![Ultrawave's recording and playback controls](screenshots/ultrawave.png)

# Ultrawave

Ultrawave is a free, open-source audio effect for **real-time sampling**, inspired
by the RAM machines of the Elektron Machinedrum UW.

**Sequence both recording and playback** from your DAW using MIDI notes or
automation. Capture audio while the music is running, then trigger and rearrange
the captured sound as part of the same sequence.

Each instance records one mono sample in 12-bit resolution. Sample a track, a
bus or the master output, or resample the processed playback into a new take
without stopping the music.

**[Releases & downloads](https://github.com/EphemeralDSP/ultrawave/releases)** ·
[User guide](docs/manual.md) ·
[Report a bug](https://github.com/EphemeralDSP/ultrawave/issues)

## Get Ultrawave

**No packaged release is published yet.** Binaries will live on the
[releases page](https://github.com/EphemeralDSP/ultrawave/releases).
For now, you can [build from source](CONTRIBUTING.md#building).

Native release targets are **Windows x86-64**, **Linux x86-64**, and
**macOS Apple Silicon**, in **VST3 and CLAP** format. No AU or Intel Mac builds.

When a release is available, choose your platform's ZIP under **Assets**, not
GitHub's “Source code” archives. Unzip it, copy the plugin to your DAW's plugin
folder, and rescan. The [installation guide](docs/manual.md#installation) has paths.

Mac builds are ad-hoc signed, not notarized; Windows builds are unsigned.
Your OS may show a trust prompt. Linux compatibility depends on the build's
glibc version; see the installation guide.

## Sampling and playback

- **Separate record and play triggers.** Place recording and playback notes in
  your DAW's sequence, or use automation. Recording becomes part of the pattern,
  rather than a separate preparation step.
- **Timed capture.** Record from the main input or sidechain for a beat-based
  duration, or fill the available RAM—up to ten seconds, depending on sample rate.
- **Resampling.** Record the processed sample playback along with incoming audio.
  The old take stays playable until the new one is ready.
- **Sample manipulation.** Change pitch, select a region with Start and End, or
  cross the markers for reverse playback. Hits and Interval control repeated
  playback, including infinite retriggering.
- **12-bit sound.** Recording and playback have independent rate crush and drive.
  Playback bit crush reduces the resolution further, down to 2 bits.

Ultrawave follows the UW's approach to live sampling, but is **not a bit-exact
Machinedrum emulation**. [More on the sound](docs/manual.md#capacity-and-sound).

## Sequence a recording

1. Put Ultrawave on an **audio track or bus**—it's an effect, not an instrument.
   Route MIDI to that same effect instance.
2. Leave the capture length at **1 bar**. Place MIDI note **48** at the start of
   a bar to record the audio passing through the effect.
3. Place MIDI note **50** at the start of the following bar to play the recording.
   Add more Play notes to retrigger it at different points in the pattern.
4. Automate Pitch, Start and End to vary the playback. With **Resample** selected,
   another Record note can capture the processed playback along with the input.

Enable **Record once** to keep the first take when the pattern loops. Leave it
off to capture new audio on each Record trigger. **Re-arm** allows another take
when protected. Start quietly: drive and resampling can get loud.

In Renoise, use an [FX alias](docs/manual.md#renoise-use-an-fx-alias) to send
notes to the effect. You can also use the Record/Play buttons or host automation.
See the guide for [other hosts and sidechains](docs/manual.md#midi-and-daw-routing).

**Recorded audio is temporary.** Projects save controls, **not recorded audio**.
RAM starts empty when you reopen a project. Record or render Ultrawave's output
in your DAW before closing if you want to keep the take.

## Building and contributing

Requires **Rust 1.95+** and your platform's development libraries. No private
repositories or dependency credentials are needed.

```bash
cargo build --release
cargo xtask bundle ultrawave-plugin --release
```

The bundles land in `target/bundled/`. A standalone application is also available
when building from source. See [Contributing](CONTRIBUTING.md) for setup, tests
and offline audio tools, or the [release workflow](docs/releasing.md) for packaging.

Built with Rust, [NICE-plug](https://codeberg.org/RustAudio/nice-plug) and egui.
[MIT licensed](LICENSE) · [Framework notices](vendor/README.md) ·
[Buildkite](https://buildkite.com/jmlynn/ultrawave)
