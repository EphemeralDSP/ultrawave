# Ultrawave audio architecture

Ultrawave is one MIDI-controlled audio effect, exported through NICE-plug as
VST3/CLAP and as a standalone. Capture and Play are always-visible actions and
DSP machines inside the same instance. The sound is UW-inspired;
interpolation, drive and timing curves are not claimed to be hardware-exact.

## Input is captured before sample-only output is written

```diagram
┌───────────────────┐
│ Stereo main input │
└────────┬──────────┘
         ▼
┌───────────────────┐    ┌────────────────┐    ┌──────────────┐
│ Main/side levels  │───▶│ Capture DSP    │───▶│ One sample   │
│ and balances      │    │ 12-bit storage │    │ SampleMemory │
└────────▲──────────┘    └────────────────┘    └──────┬───────┘
         │                                          ▼
┌────────┴──────────┐                         ┌────────────────┐
│ Stereo sidechain  │                         │ Playback DSP   │──▶ Stereo output
└───────────────────┘                         │ Mono to L + R  │
                                             └────────────────┘
```

`src/plugin/sampler.rs` owns host processing, MIDI commands, control edges,
Record once protection and sample-only output. Each block is mixed to a mono capture
source in bounded 64-frame SIMD chunks before any playback reaches the output.
The capture chain is input mix → capture level → SRR → drive → recording RATE
selection/12-bit quantization. Incoming audio never passes through to the output.

`ram_play.rs` and `sample_engine.rs` handle source-rate-aware varispeed,
linear interpolation, SRR, hold/decay, BRR, drive and retrigger timing. The
callback applies note velocity and playback level. There is one active playback
voice/train and one capture at a time. An accepted capture stops the old voice
and repeat train, then replaces the sample. A protected trigger changes neither.
Playback can latch the written prefix while a capture continues to grow.

## RAM belongs to the effect instance

`src/machines/memory.rs` owns one heap buffer of 441,000 `AtomicI16` samples,
storing signed 12-bit codes (-2048…2047). Capacity is fixed, not ten seconds
at every sample rate. Length, source rate and revision metadata are atomic.

The owning audio/control path serializes mutation. Triggering playback latches
length/rate/revision in O(1), without copying samples, allocating or locking.
New captures, replacement and clearing change revision so readers cannot keep
playing the old take. Growing captures publish only their written prefix.

The editor holds an Arc to this same instance's RAM, reads validated snapshots
off the audio thread and caches waveform peaks. It exchanges triggers/meters
through per-instance atomics. There are no OS mappings, global banks, pairing
workers, leases or cross-process writer claims. The public DSP API is also used
by offline renders/tests; public visibility does not mean globally shared RAM.

## Controls persist; audio and runtime latches do not

`src/params.rs` defines 26 saved host controls, including MIDI assignments but no
buffer selector or output mode. Neither RAM nor Record once's spent
latch is serialized. Host reset stops voices/captures while preserving those
runtime data. Recreating the instance produces empty RAM and an armed recorder.
Reset snapshots current command levels: a held/restored high Capture, Playback
or Re-arm is not a new rising edge. MIDI and UI triggers remain independent.

The standalone alone may preload a fixture WAV for screenshots. Hosted effects
ignore that environment variable. Renoise's FX alias and Live MIDI-to-effect
routing address the same effect instance, not a second memory owner; there is
no implicit host-master tap. See [README](../README.md) for routing and semantics.

## Executable checks

Use `scripts/dev-loop.sh fast` for DSP/unit tests, the exported CLAP audio/state
harness, VST3 wrapper regressions and deterministic WAVs. `full --editor` adds
release builds, validators, editor lifecycle checks and benchmarks. The real
Renoise fixture checks routing, FX-alias notes and process save/reopen. See
[plugin validation](testing/plugin-validation.md) for commands and platform limits.
