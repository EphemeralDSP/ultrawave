# Machinedrum timing and sound measurements

Status, 2026-09-26: the owner's X.08 unit was measured using the explicitly
approved disposable RAM1. All 128 RTRG and RTIM settings were captured at 150 BPM.
Ultrawave now uses the firmware count/timing law and measured steady rate limit.
Both analog sends and the C return are verified. Six analog sampling sweeps and
five tone levels establish a repeatable combined-path magnitude response, **not
an isolated DAC transfer function**. Internal generations and read-only RAM1
exports now separate capture from playback. A conservative 17 Hz wet-path low
cut is implemented; treble/interpolation matching remains unfinished.

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
- Scarlett outputs 3/4 → MD L/R are verified by separate −36 dBFS captures and
  an opposite-balance rejection test. Returned right-channel RMS was 0.10 dB
  above left; the rejected source was at the recorded-silence noise floor.
- `macmini.local` is reachable over SSH: Apple Silicon, Python and Rust, Renoise
  and REAPER installed. It currently exposes only built-in audio. Keep hardware
  acquisition on the ThinkPad. Native ARM64 workspace and exported CLAP audio
  tests pass with Rust 1.95 in debug/release, along with 17 VST3 state tests,
  auditions and the capture benchmark. The combined validator command passes
  pluginval strictness 5 and CLAP's 18 tests (3 unsupported skips). Isolated
  REAPER verifies real MIDI capture/play, saved controls, silent RAM after fresh
  process reopen, and editor open/close. macOS screenshot capture was unavailable;
  editor lifecycle was checked through REAPER and pluginval, not visual inspection.
  Final VST3/CLAP bundles are installed in the user's `~/Library/Audio/Plug-Ins/`
  directories; legacy split plugins were preserved.

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
This is original-code evidence, replacing the former tips-page approximation in
`src/machines/ram_play.rs`. The live unit answered the universal identity request
with software revision `X08 ` before measurement.

For direct, settled Q7 owner inputs `G = RTRG << 7`, `H = RTIM << 7`:

- `MAIN:0x204abc..0x204af8` writes the count operand at destination word 8:
  raw 0–63 → raw; raw 64–126 → `floor(raw² / 128) + 32`;
  raw 127 → `0x7f0000`. For example, raw 96 maps to 104; raw 126 maps to 156.
  Physical captures confirm this means **total hits**, with 0 and 1 both giving
  one hit. Every raw finite setting 0–126 matched. Raw 127 continued for 200
  detected hits in the bounded capture, beyond the finite range; the firmware
  sentinel is modeled as continuous playback until stopped.
- `MAIN:0x204af8..0x204b96` writes the interval at word 9. Below owner `0x2000`
  it scales linearly with H; above that boundary it uses the logarithmic table
  at `0x24bd94`. Both branches use tempo at `0x0100150c`.
- The formatter clamps the interval to at least **68 internal units**. Raw RTIM
  0 is not off. The DSP can restart at most once per 32-frame render, so its
  measured steady minimum period is 32/44100 seconds, equivalent to 128 units.
- DSP section 1 `P:0x18f..0x1a9` latches words 9/8 into timing/count state.
  `P:0x1e5..0x221` increments timing by 128 per render, preserves overshoot,
  compares the incremented hit counter against the count operand, and updates
  restart position. The physical 0/1/2 tests resolved the off-by-one question:
  the UI now displays total **Hits**, not additional repeats.

Some direct-input formatter results at 120 BPM:

| Raw RTIM | Formatter operand | Nominal time* |
| ---: | ---: | ---: |
| 0 | 68 | 0.38549 ms |
| 1 | 86 | 0.48753 ms |
| 32 | 2756 | 15.62358 ms |
| 63 | 5425 | 30.75397 ms |
| 64 | 5555 | 31.49093 ms |
| 96 | 11197 | 63.47506 ms |
| 127 | 22050 | 125 ms |

\* These are firmware units divided by 176400, not measured onsets. The plugin
clamps to **128 units**, giving 0.72562 ms for both 0 and 1 at 120/150 BPM.
The 150 BPM hardware 0/1 trains have a spectral peak at 1378.05 Hz (FFT bin
spacing 0.183 Hz), consistent with 44100/32 = 1378.125 Hz.

The implemented mapping preserves the integer operations. With `T = BPM × 24`:

```text
raw < 64:
    scale = floor(floor(270950400 / T) × 60 / 4096)
    units = floor(scale × raw / 16)
raw >= 64:
    step = floor(raw × 512 / 127)
    table = round(2048 × 2^((step & 127) / 127.5))
    scaled = table >> (5 - (step >> 7))
    units = floor(floor(scaled × 1058400 / T) × 60 / 1024)
seconds = max(units, 128) / 176400
```

The analytic exponential matches all 128 original table entries. No extracted
coefficient table is shipped. At 150 BPM the measured steady intervals versus
nominal values include:

| Raw | Measured ms | Nominal ms |
|---:|---:|---:|
| 32 | 12.501 | 12.494 |
| 63 | 24.650 | 24.598 |
| 64 | 25.255 | 25.193 |
| 96 | 50.783 | 50.777 |
| 127 | 100.013 | 100.000 |

Analysis uses 48-frame RMS >0.0008, grouping gaps >96 frames, then an onset
linear fit excluding the first hit. All 103 settings from 25–127 resolve into
eight markers; maximum fitted interval discrepancy is 0.144 ms. Faster settings
overlap and are **not** validated by that onset detector. The raw captures are
retained for finer DSP-block analysis. The startup interval and block jitter
are not sample-exact matches to Ultrawave's uniform fractional timer.

### Model boundary and restoration

The upstream integer smoother can settle 1–3 Q7 units below a requested value.
Changing an existing machine sometimes gave 103 instead of 104 hits at raw 96,
or 155 instead of 156 at 126. Fresh RAM-P assignment before each sweep case
removed this history dependence. Ultrawave deliberately models canonical fresh
values and steady rate, **not** that smoother or sub-block startup jitter. Tempo
law tests at other tempos are firmware-derived, not additional hardware sweeps.

Private evidence is under `~/.local/share/ultrawave/hardware/ram1-measurement/`:
`smoke-01`, `anchors-01`, `boundaries-01`, `sweep-01`, and `sound-01`.
The sweep's `timing-analysis.json` contains every counted case and the 103
resolved interval fits. MIDI journals, raw ten-channel captures, mono WAVs,
parameter readbacks and original SysEx remain there, outside Git.

`measure.py` is an owner-authorized experiment, **not a read-only probe**. It
temporarily assigns RAM-R1/RAM-P1 to track 1 and overwrites RAM1. RAM-R1 raw model
160 and RAM-P1 raw 162 use assignment init mode 0, retaining the other controls.
All 24 controls and track level are read back. Restoration must explicitly
reassign the original model and restore those controls: sending a Kit dump with
slot 127 does **not** install the live edit state. Every completed experiment
verified exact Global, current Kit, stored Kit, active slots and both Scarlett
output routes. The disposable RAM1 contents are intentionally not restored.

## 2. Measure the sampling path before adding coloration

Generated inputs are 48 kHz/32-bit/six-channel WAVs: separate L/R 997 Hz route
probes at −36 dBFS, 20 Hz–20 kHz log sweeps at −36/−24/−12 dBFS, and 997 Hz
tones at −48/−36/−24/−12/−6 dBFS. Each has 250 ms guards and 10 ms fades.
They were acquired in `sound-01` using RAM-R1 LEN/RATE 127, RAM-P1 unity pitch,
full region, HOLD/DEC 127, neutral FX, velocity 64 and track level 100. No clipping
was observed. All restoration checks passed. `measure-script.py` preserves the
acquisition script, and `analyze_sound.py` writes `sound-analysis.json` offline.

Six independent captures (two each at −36/−24/−12 dBFS) gave the following
combined-path response relative to 1 kHz. Analysis takes the complete guarded
sweep's FFT magnitude ratio, averaged in 1/12-octave bands; trigger delay is not
a phase measurement.

| Frequency | Relative gain, six-run range |
|---:|---:|
| 30 Hz | −1.24 to −1.20 dB |
| 100 Hz | −0.20 to −0.19 dB |
| 4 kHz | −0.617 to −0.614 dB |
| 8 kHz | −1.600 to −1.594 dB |
| 12 kHz | −2.793 to −2.780 dB |
| 16 kHz | −5.108 to −5.103 dB |
| 19 kHz | −8.279 to −8.257 dB |

The 997 Hz tone gain was −23.82 to −23.83 dB for sent levels −48 through
−6 dBFS. That absolute gain includes track level, MIDI velocity and interface
settings; do not use it as plugin output attenuation. The returned idle noise
floor was about −100.7 dBFS RMS. High-level tone captures contained isolated
phase steps of about one 48 kHz frame, which inflate whole-window THD+N estimates.
Their origin is not established; do not turn those residuals into a distortion
model. Short-window residuals and repeated calibrated tones are needed first.

### Pitch test constrains the coloration model

The follow-up `pitch-01` records one −24 dBFS sweep into the same approved RAM1,
then plays that unchanged take at raw PITCH 64, 52, 76, and 64 again. A 997 Hz
tone established the actual speed ratios first: 0.793701 at raw 52 and 1.256351
at raw 76. Do not assume those raw values mean ±12 semitones. The spectrum
comparison scales the source frequency by those measured ratios and normalizes
at output 1 kHz. It does not interpret FFT phase or fit a delay.

| Output frequency | Pitch 52 | Pitch 64 | Pitch 76 |
|---:|---:|---:|---:|
| 4 kHz | −0.653 dB | −0.616 dB | −0.630 dB |
| 8 kHz | −1.471 dB | −1.598 dB | −1.650 dB |
| 12 kHz | −3.434 dB | −2.790 dB | −2.708 dB |
| 14 kHz | −5.744 dB | −3.732 dB | −3.329 dB |
| 16 kHz | outside down-pitched sweep | −5.106 dB | −4.060 dB |

The final unity replay agrees within 0.003 dB over 2–19 kHz; tails return to
about −101 dBFS RMS. Higher pitched frequencies near 19 kHz can include folded
out-of-band energy, so they are excluded from the model-placement comparison.
All six restoration checks passed; only disposable RAM1 contents changed.

This rejects a fixed output EQ as the **whole** explanation. It does not uniquely
separate captured filtering from pitch-dependent interpolation. A steep section
before pitch plus a gentle section afterward predicts these new captures better
than putting everything after pitch, without refitting the unity-derived corners.
See [filter-model evidence](machinedrum-whitepaper.md) for the candidates, their
errors and the unresolved source attribution. Silver's generic one-pole/DC-blocker
code is not evidence of a whitepaper-derived UW reconstruction filter.

Treat the Scarlett as provisionally flat for this comparison. Interface loopback
is a useful calibration check, not the leading explanation or a prerequisite
for all further algorithmic investigation. Keep any eventual model small and
physically interpretable; do not fit a many-band EQ to the normal-pitch capture.

### Internal generations and a read-ahead control

`generations-01` temporarily assigns RAM-P1 to track 2/main stereo and RAM-R1
to track 1, capturing MLEV. Track 2 starts 50 ms before recording; the initial
250 ms source silence accommodates three passes in the same disposable RAM1.
Each generation is then played through track 1/C. Master EQ is neutral,
compressor MIX is zero, pan centered and delay/reverb sends zero.

The first additional generation loses 1.04/3.25/6.71/12.84/21.01 dB at
4/8/12/16/19 kHz relative to 1 kHz. Generations 2 and 3 repeat the low/mid loss,
but their upper bands reach the noise floor and are not useful filter targets.
The roughly 9.64 dB level loss per pass includes the chosen levels; it is not
baked into Ultrawave.

`read-ahead-01` verifies that in-place capture is not causing the spectral loss:
track 2 plays through C while track 1 records silence 50 ms behind it. Its
response differs from ordinary playback by at most 0.0015 dB at the eight
checked frequencies from 30 Hz to 19 kHz. Subsequent playback has RMS
0.00000928, the recorded-silence floor. Original Kit/Global, both touched tracks,
master settings, active slots and Scarlett routes were restored exactly.

### RAM1 MIDI export disproves the steep external-capture hypothesis

`sds-01` captures a new sweep, exports the actual RAM1 PCM, plays it at unity and
the two measured pitch ratios, then makes and exports one internal generation.
MCL's `SampleBrowserPage.cpp` establishes a read-only slot-count query
(`70 34`); this unit reports 48 ROM positions, so RAM1 is SDS sample number 48.
The request is `F0 7E 00 03 30 00 F7`. No sample header/upload, ROM write or
other-RAM write is sent. Every packet's sequence and XOR checksum is verified
before ACK. Both exports contain 139,936 words, 16-bit format, period 22,672 ns
(header rate rounded to 44,107 Hz for analysis, not proof of the exact clock).

Compare spectra to the **exported PCM**, not just the originally sent sweep.
These are magnitude-only 1/12-octave bands, normalized at 1 kHz:

| Frequency | External input → stored PCM | Stored PCM → playback/C | Added internal generation |
|---:|---:|---:|---:|
| 30 Hz | −0.103 dB | −1.134 dB | −1.139 dB |
| 100 Hz | −0.035 dB | −0.151 dB | −0.167 dB |
| 4 kHz | +0.029 dB | −0.643 dB | −1.044 dB |
| 8 kHz | +0.102 dB | −1.699 dB | −3.251 dB |
| 12 kHz | +0.134 dB | −2.923 dB | −6.708 dB |
| 16 kHz | +0.031 dB | −5.136 dB | −12.864 dB |
| 19 kHz | −0.293 dB | −7.983 dB | −21.224 dB |

Playback/C measured against generation 1's own PCM agrees with generation 0
within 0.016 dB through 16 kHz (19 kHz is less reliable at the lower level).
Most treble loss is therefore **not stored during external capture**. Its
pitch dependence remains after referencing stored PCM: at output 14 kHz,
pitch 52/64/76 gives −5.66/−3.83/−3.46 dB. Down-pitched 19 kHz lies beyond
the input sweep's support and must be discarded, not interpreted as a boost.
The former “steep capture filter plus gentle playback filter” fit is rejected.

SDS words occupy all sixteen low-nibble codes. This is not a dump of plain
left-shifted 12-bit integers, and does not prove how the playback precision or
sample encoding works. The plugin's existing linear 12-bit quantizer remains
an approximation, not something this experiment has verified.

Evidence, journals, exact-state restoration receipts and script snapshots remain
in the private `ram1-measurement/{generations-01,read-ahead-01,sds-01}` folders.
`analyze_sds.py` there reproduces the exported-PCM comparisons; no private audio
or firmware is needed by the plugin. All completed acquisitions restored state.

### Production choice and the remaining sound-matching work

The repeated ~1.1–1.2 dB loss at 30 Hz supports a broad low-cut approximation.
Ultrawave now applies a normalized first-order 17 Hz DC blocker after wet
playback level/velocity and before the output/self-resampling split. Dry-through
and external-input capture/quantization are unchanged; master resampling captures
the filtered wet contribution. The filter runs across notes and idle tails;
host reset clears its state, not RAM. Offline playback uses the same stage.
This is an engineering model, not a recovered circuit. Host-rate coefficients
preserve its low-frequency corner at 44.1/48/96 kHz.

Treble EQ is deliberately unshipped. Separate pitch-dependent interpolation,
always-running track processing and the internal main tap before selecting a
small causal model. The internal-minus-C loss resembles a two-point averaging
response, but its placement and fixed-clock behavior are not established.
Do not turn that resemblance into a host-rate FIR whose cutoff moves with the
DAW rate, or multiply the entire measured analog response into every generation.

The prepared held-out 17/60/200/1000 Hz and drum run (`validation-01`) stopped
at the exclusive-device check because PipeWire had opened Scarlett playback.
No hardware changes were made by that attempt. New tone/drum hardware A/B,
main L/R analog comparison and a full treble/interpolation model remain open.
An unchanged-gain Scarlett loopback is useful but optional; select PCM3/4
**before** any physical loopback repatch to avoid the restored mixer feedback.

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
not included in this repository. Identity reply on the live unit was
`F0 7E 00 06 02 00 20 3C 02 00 08 00 58 30 38 20 F7` (X.08).
