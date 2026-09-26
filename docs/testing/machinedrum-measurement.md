# Machinedrum timing and sound measurements

Status, 2026-09-26: MIDI control and the output-C return are verified. Test
stimuli are generated. No sampler recording, sample replacement, parameter
sweep, transfer-function fit, or plugin DSP change has been performed yet.

## The bench is on the ThinkPad

- Scarlett 8i6 USB, USB identity `1235:8213`, ALSA `hw:CARD=USB,DEV=0`.
  Playback is six channels; capture is ten channels, S32_LE at 48 kHz.
- U2MIDI Pro, USB identity `7104:1400`, is the connected MIDI path. Scarlett's
  MIDI port is not the established path. Resolve ALSA client/card numbers anew.
- Track 1 has raw model 32, route 2 (C), note 36 on MIDI channel 1. Global and
  Kit slots are 0; the observed tempo field is 3600 (150 BPM).
- **Output C → Scarlett analogue input 3 is measured, not assumed.** One
  velocity-64 hit gave peak 0.0815134, idle RMS 0.00000965004, post-trigger RMS
  0.0314387: 70.26 dB above baseline. The other analogue slots stayed near their
  baselines. No clipping or ALSA overrun occurred.
- Scarlett outputs 3/4 → MD L/R are the owner's reported wiring. Those send
  paths and L/R order still need independent low-level probes.
- `macmini.local` is reachable over SSH: Apple Silicon, Python and Rust, Renoise
  and REAPER installed. It currently exposes only built-in audio. Keep hardware
  acquisition on the ThinkPad; Mac host testing is separate and not yet done.

### Break the accidental feedback loop before sending anything

The Scarlett currently routes analogue outputs 3/4 from **Mix C/D**, not PCM3/4.
Those mixes include analogue input 3, the MD return. This can feed the MD back
into its own analogue inputs independently of the intended resampling test.

For acquisition, snapshot the mixer and temporarily select `PCM 3` and `PCM 4`
on `Analogue Output 03 Playback Enum` and `Analogue Output 04 Playback Enum`.
Verify both choices. Send six-channel audio with slots 1/2/5/6 exactly silent.
Do not alter input gains, monitor mixes, or outputs 1/2. Restore the original
choices afterward. The completed return probe did this and verified restoration.

## Repeat the safe setup check

The bench reuses Rivet's established MIDI identity checking and verified SysEx
decoder from the existing `machinedrum-oracle` environment. No new system Python
packages, MIDI connections, or audio configuration files are required.

From the Ultrawave checkout:

```bash
PY=../machinedrum-oracle/.venv/bin/python
RUN="$HOME/.local/share/ultrawave/hardware/$(date -u +%Y%m%dT%H%M%SZ)"

# Queries and private backups only. Requires both USB devices.
"$PY" -B scripts/md-hardware-bench.py preflight "$RUN-preflight"

# One existing track-1 hit; temporarily isolates outputs 3/4, then restores them.
# Refuses a different machine/note/route or an occupied playback device.
"$PY" -B scripts/md-hardware-bench.py return-probe "$RUN-return"

# Offline only: generate signals; this does NOT play them to the hardware.
"$PY" -B scripts/md-hardware-bench.py stimuli "$RUN-stimuli"

# Offline safety/channel/level and injected-failure cleanup tests.
"$PY" -B -m unittest discover -s tests -p test_md_hardware_bench.py -v
```

Each action requires a new directory. Backups, journal, audio, and mixer dumps
stay outside Git. The probe sends only the existing note 36 on/off plus read-only
requests; it does not write Global/Kit state, change a machine, start transport,
or overwrite RAM. It verifies Global/current Kit/stored Kit bytes and active
slots after the probe. Cleanup runs on ordinary failures, SIGINT, and SIGTERM;
SIGKILL/power loss cannot run cleanup. Original mixer choices are saved before
any change for manual recovery. If restoration fails, stop rather than repeat.

Evidence from this session is under
`~/.local/share/ultrawave/hardware/20260926-return-probe/`:
`preflight.json`, `return-analysis.json`, `restoration.json`, `midi-journal.json`,
`original-*.syx`, `mixer-before.txt`, `return-10ch.wav`, and `arecord.log`.
All eight restoration checks passed. A second run of the final script in
`20260926-return-probe-verified/` also passed (71.10 dB above baseline, peak
0.0823110, all eight restoration checks true). Generated inputs are in the
sibling `20260926-stimuli/` directory with a SHA-256 manifest.

## 1. Replace timing guesses with firmware plus measured hit trains

### What the original firmware establishes

The private X.08 MAIN image has SHA-256
`022fdded563c4688ee5bd58571c2122558906a9cfb9b95af8023dfd36b3430c3`.
Its model table at `0x252092` selects formatter `0x20490e` for RAMP1/2/3/4.
The complete formatter span `0x20490e..0x204ba2` is **byte-identical in official
1.63**; span SHA-256 is
`3e4f853c68bcc584b1c730e849d58a87200954e1713a8f1eba0599746c89a83e`.
This is original-code evidence, not the tips-page approximation currently in
`src/machines/ram_play.rs`.

For direct, settled Q7 owner inputs `G = RTRG << 7`, `H = RTIM << 7`:

- `MAIN:0x204abc..0x204af8` writes the count operand at destination word 8:
  raw 0–63 → raw; raw 64–126 → `floor(raw² / 128) + 32`;
  raw 127 → `0x7f0000`. For example, raw 96 maps to 104; raw 126 maps to 156.
  This is **an internal count operand**, not yet a promise of that many
  *additional* audible hits. Do not equate raw 127 with a proven infinity branch.
- `MAIN:0x204af8..0x204b96` writes the interval at word 9. Below owner `0x2000`
  it scales linearly with H; above that boundary it uses the logarithmic table
  at `0x24bd94`. Both branches use tempo at `0x0100150c`.
- The final interval is clamped to at least **68 internal units**. Raw RTIM 0
  is not an off switch in this formatter. Ultrawave currently treats it as off.
- DSP section 1 `P:0x18f..0x1a9` latches words 9/8 into timing/count state.
  `P:0x1e5..0x221` increments timing by 128 per render, preserves overshoot,
  compares the incremented hit counter against the count operand, and updates
  restart position. This raises an off-by-one question for raw RTRG 0/1/2;
  measuring those cases is mandatory before changing the UI's “Repeats” meaning.

Some direct-input formatter results at 120 BPM:

| Raw RTIM | Operand | Candidate nominal time* |
| ---: | ---: | ---: |
| 0 | 68 | 0.38549 ms |
| 1 | 86 | 0.48753 ms |
| 32 | 2756 | 15.62358 ms |
| 63 | 5425 | 30.75397 ms |
| 64 | 5555 | 31.49093 ms |
| 96 | 11197 | 63.47506 ms |
| 127 | 22050 | 125 ms |

\* Dividing by four units per 44.1 kHz frame is consistent with the 128-unit
advance and 32-output-frame render. These are **not measured onset intervals**.
Sub-block restart logic and minimum-interval behavior still need verification.
The upstream integer smoother can settle 1–3 Q7 units below an upward target;
that matters at the 63/64 boundary and table-index boundaries. Sweep in both
directions, and do not substitute a raw-input table for live behavior blindly.

### Measurement order and acceptance

1. Reserve a disposable RAM buffer and temporary test-track assignments. Back
   up state first, and restore/read back every changed field afterward. A Kit
   dump does **not** back up recorded sample RAM; obtain approval before erasing it.
2. Capture a short recognizable marker with a silent tail; route RAMP playback
   alone to C. Disable LFO, sends, distortion, SRR and filtering as appropriate;
   read back all 24 track controls and level. Preserve the recorded marker for
   every timing case. Keep capture muted while measuring playback.
3. Check RTRG 0/1/2, 63/64/65, 96, 126/127 first. Detect actual restart positions
   by correlation with the recorded marker, not wall-clock MIDI timestamps.
   Count both total hits and repeats after the initial hit. Bound the 127 case.
4. Sweep all 128 RTIM values at 120 BPM; repeat the boundaries/landmarks at
   60 and 150 BPM. Include upward/downward changes, minimum RTIM, retriggers
   after sample end, forward/reverse regions, and tempo changes mid-train.
5. Sweep finite RTRG values at an interval long enough to resolve the marker.
   Compare the count law and off-by-one interpretation independently of timing.
6. Only then change Ultrawave's mapping, display, parser and cancellation rules.
   Add source-pinned mapping tests and hardware-derived timing fixtures, retain
   fractional-interval drift tests, and rerun CLAP plus actual Renoise tests.

Pass means correct total hit counts, confirmed endpoint behavior, and onset
spacing within the measured hardware repeatability/block quantization—not an
arbitrary tolerance around the existing implementation. If the measured law is
simple, implement it directly; do not fit 128 unrelated magic values.

## 2. Measure the sampling path before adding coloration

Generated inputs are 48 kHz/32-bit/six-channel WAVs: separate L/R 997 Hz route
probes at −36 dBFS, 20 Hz–20 kHz log sweeps at −36/−24/−12 dBFS, and 997 Hz
tones at −48/−36/−24/−12/−6 dBFS. Each has 250 ms guards and 10 ms fades.
They are preparation, not hardware measurements. Start quiet, inspect each
return, and stop escalating as soon as a stage approaches clipping.

1. **Confirm sends:** one side at a time, input monitor/capture settings verified
   on the MD. Establish L/R balance and gain without relying on an existing mix.
2. **Calibrate the interface:** a direct Scarlett output3→input3 loopback, and
   output4→input3 if needed, at unchanged gain/rate. This requires a temporary
   physical cable repatch. Retain both calibrated and raw results; do not call
   the uncalibrated combined response “the Machinedrum DAC.”
3. **Analog sampling round trip:** record each low-level sweep into the approved
   RAM slot, then play at unity pitch through C with neutral track processing.
   Record several repeats, silence and level sweeps. Estimate delay, frequency
   response, noise, harmonics, clipping threshold and level-dependent residual.
   A 12-bit quantizer/clipping chain is nonlinear; one impulse response cannot
   describe its whole sound. Align delay/drift and gain before comparing spectra.
4. **Separate playback from capture:** if a disposable digital sample slot or
   supported sample-transfer path is approved, load the exact reference PCM
   digitally and compare it with analog-captured PCM playback. Combine that
   with interface calibration to separate input-side and playback-side effects.
   Do not overwrite a ROM slot merely to obtain this control measurement.
5. **Pitch and resampling:** unity, octave down/up, representative intermediate
   pitches, and several internal resampling generations versus analogue-loop
   generations. The original sampler DSP has interpolation/filter stages; the
   crunchy difference need not be DAC EQ. Keep capture/playback gain fixed.
6. **Output topology:** output C isolates the voice, but it cannot establish the
   main-output path's response. Repeat selected tests on main L/R before claiming
   a master-bus/DAC match. Output C and the master may have different processing.
7. **Fit only what repeats:** add the smallest justified filter/nonlinearity,
   then hold out some levels, pitches and musical loops for blind, level-matched
   A/B. Validate the exported plugin offline, in Renoise, and on the Mac before
   calling the measured match finished. No guessed “vintage” tilt by default.

## Sources and limits

- Original Rivet method and restoration evidence:
  [PI-ML hardware check](../../../Rivet/docs/research/pi-ml-hardware-check-20260908.md)
  and its private `capture.py` under
  `~/.local/share/machinedrum-oracle/private/pi-ml-hardware-20260908/`.
- MIDI implementation: `machinedrum-oracle/src/machinedrum_oracle/sysex.py`
  (`MachinedrumSession`, checksum/framing, Global/Kit decoding) and `midi.py`
  (USB/kernel endpoint identity). Existing FM-only write helpers are deliberately
  restricted; no restriction was removed to make this probe work.
- Original sampler bytes: private `efmxt-first-hit-continuation-v1/unpacked-x08/`
  and `unpacked-163/section-0.bin`; DSP consumer in `section-1.bin`.
  MAIN decoded with Capstone M68K; DSP load records decoded by
  `mdhack_rebuilt.dsp`, then the existing `dsp56300-disasm -f u24be -s ADDRESS -c never`.
- Selector/lifecycle indexes: `mdhack-rebuilt/evidence/analysis/`
  `DSP-SELECTOR-PLUS-ONE-HYPOTHESIS.json` and
  `DSP-SECTION1-DISPATCH-SEMANTICS.json`, checked against the actual byte images.
- Owner Q7 smoothing: original MAIN `0x2625d4..0x262624`, corroborated by
  [Rivet's filter producer investigation](../../../Rivet/docs/research/md-filter-producer.md).

Private firmware, SysEx, captured audio and extracted coefficient tables are
not included in this repository. The live unit's firmware version was not queried
by this setup probe. The identical 1.63/X.08 formatter narrows that uncertainty;
it does not replace the pending measurement on the owner's unit.
