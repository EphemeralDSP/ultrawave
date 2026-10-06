![Ultrawave: one waveform, Record and Play, and a handful of ways to mess with a sample](screenshots/ultrawave.png)

# Ultrawave

**Grab a loop. Make a mess. Sample it again.**

Ultrawave is a free, open-source **12-bit sampling effect** inspired by the
Elektron Machinedrum UW. Catch a phrase from your track, twist it into something
else, then record the result back into itself.

One instance. One mono sample. No sample library to organize—just whatever is
playing right now.

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

## Small sampler, bad habits

- **Catch it live.** Record a drum bus, a synth phrase or your whole mix. Capture
  a beat, a bar, or up to ten seconds, depending on sample rate.
- **Give it some grit.** Start with 12-bit capture, then push rate crush, bit
  crush and drive. Recording and playback have their own processing.
- **Find the good bit.** Trim with the waveform handles, pitch it down, or cross
  Start and End to play the slice backward.
- **Make it stutter.** Turn a one-shot into a roll with Hits and Interval, all
  the way to an infinite retrigger train.
- **Go around again.** Resample the processed take along with incoming audio.
  The old take stays playable until the new one is ready.
- **Play it like an effect.** Use the buttons, MIDI notes or DAW automation.
  No second plugin or sampler instrument needed.

This is **UW-inspired, not a bit-exact Machinedrum emulation**. The aim is that
immediate capture–mangle–resample workflow, with its own rough edges and happy
accidents. [More on the sound](docs/manual.md#capacity-and-sound).

## Your first loop

1. Put Ultrawave on an **audio track or bus**—it's an effect, not an instrument.
2. Play some audio and press **Record**. The default capture length is **1 bar**.
3. When recording finishes, press **Play**. Try lowering Pitch or crossing the
   waveform's Start and End handles.
4. Leave **Resample** selected and hit Record while the sample plays to capture
   another generation. Press Play again when the new take is ready.

Looping a pattern? Enable **Record once** to protect the take. **Re-arm** lets
the next Record trigger replace it. Start quietly: drive and resampling can get
loud.

MIDI note **48** records; **50** plays. In Renoise, use an
[FX alias](docs/manual.md#renoise-use-an-fx-alias) to send notes to the effect.
See the guide for [other hosts and sidechains](docs/manual.md#midi-and-daw-routing).

**Print the happy accidents.** Projects save controls, **not recorded audio**.
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
