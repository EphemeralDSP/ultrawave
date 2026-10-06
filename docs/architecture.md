# Ultrawave audio architecture

Ultrawave is one MIDI-controlled audio effect, exported through NICE-plug as
VST3/CLAP and as a standalone. Capture and Play are always-visible actions and
DSP machines inside the same instance. The sound is UW-inspired;
interpolation, drive and timing curves are not claimed to be hardware-exact.

## Master mode records the input plus processed playback

```diagram
┌───────────────────┐
│ Stereo main input │
└────────┬──────────┘
         ▼
┌───────────────────┐    ┌────────────────┐    ┌──────────────┐
│ Main/side levels  │───▶│ Capture DSP    │───▶│ SampleMemory │
│ and balances      │    │ 12-bit storage │    │ One take     │
└────────▲──────────┘    └────────────────┘    └──────┬───────┘
         │                                          ▼
┌────────┴──────────┐                         ┌────────────────┐
│ Stereo sidechain  │                         │ Playback DSP   │──▶ Stereo output
└───────────────────┘                         │ Mono to L + R  │
                                             └────────────────┘
```

`src/plugin/sampler.rs` owns host processing, MIDI commands, control edges,
Record once protection and output routing. The diagram shows the sample-only
path. Each block's host input is mixed to mono in bounded 64-frame SIMD chunks.
Master mode additionally feeds processed playback into this capture mix, scaled
by the main capture input level. Capture then applies capture level → SRR → drive
→ recording RATE selection/12-bit quantization. Main balance affects stereo host
input; the mono playback contribution is balance-independent. Sidechain is capture-only.

Master output is untouched stereo main input plus mono playback on both channels.
Sample-only output excludes dry input and capture excludes playback. Mode changes
ramp dry-through and feedback gains over 5 ms; capture storage strategy latches
at trigger. There is no same-frame recursion: playback reads the published take
before recording writes the staging take.

`ram_play.rs` and `sample_engine.rs` handle source-rate-aware varispeed,
linear interpolation, SRR, hold/decay, BRR, drive and retrigger timing. The
callback applies note velocity and playback level. There is one active playback
voice/train and one capture at a time. Master capture leaves the old voice/train
running until the new take is published, then stops it without automatically
playing the replacement. Sample-only capture stops the old voice/train at start
and allows playback to latch the growing prefix. A protected trigger changes neither.

## RAM belongs to the effect instance

`src/machines/memory.rs` owns two preallocated heap buffers of 441,000 `AtomicI16`
samples, storing signed 12-bit codes (-2048…2047): one published take and private
staging for Master resampling. These are not independently selectable samples.
Capacity is fixed, not ten seconds at every sample rate. Length, source rate,
active-buffer index and revision metadata are atomic.

The owning audio/control path serializes mutation. Triggering playback latches
length/rate/revision in O(1), without copying samples, allocating or locking.
Sample-only captures, replacement and clearing change revision immediately so
readers cannot keep playing the old take. Master capture bumps revision at
completion while swapping the active buffer in O(1), without copying. An
interrupted capture publishes its nonempty prefix; zero stored samples retain
the old take. Growing sample-only captures publish their written prefix.

The editor holds an Arc to this same instance's RAM, reads validated snapshots
off the audio thread and caches waveform peaks. It exchanges triggers/meters
through per-instance atomics. There are no OS mappings, global banks, pairing
workers, leases or cross-process writer claims. The public DSP API is also used
by offline renders/tests; public visibility does not mean globally shared RAM.

## Controls persist; audio and runtime latches do not

`src/params.rs` defines 27 saved host controls, including MIDI assignments and
Master Mode (default On), but no buffer selector. Neither RAM nor Record once's spent
latch is serialized. Host reset stops voices/captures while preserving those
runtime data. Recreating the instance produces empty RAM and an armed recorder.
Reset snapshots current command levels: a held/restored high Capture, Playback
or Re-arm is not a new rising edge. MIDI and UI triggers remain independent.

The standalone alone may preload a fixture WAV for screenshots. Hosted effects
ignore that environment variable. Renoise's FX alias and Live MIDI-to-effect
routing address the same effect instance, not a second memory owner; there is
no implicit host-master tap. See the [user guide](manual.md) for routing and semantics.

## Executable checks

Use `scripts/dev-loop.sh fast` for DSP/unit tests, the exported CLAP audio/state
harness, VST3 wrapper regressions and deterministic WAVs. `full --editor` adds
release builds, validators, editor lifecycle checks and benchmarks. The real
Renoise fixture checks routing, FX-alias notes and process save/reopen. See
[plugin validation](testing/plugin-validation.md) for commands and platform limits.
