# Filter-model sources and limits

No filter citation identifying the user's remembered whitepaper was found in
`silver-release`. The related thesis below was checked, but has **not** been
established as that reference. Do not infer a shipping sampler algorithm from
an unidentified paper or a similarly named synthesizer filter.

## Sources checked

1. Anders Gärder, *Physical Modeling of Percussion Instruments*, Chalmers
   Master's Thesis 2005:23 (80 PDF pages). Original Chalmers full text:
   <https://publications.lib.chalmers.se/records/fulltext/5768.pdf>; repository
   record: <http://studentarbeten.chalmers.se/publication/5768-physical-modeling-of-percussion-instruments>.
   The PDF identifies Gärder as author, Chalmers University of Technology, and
   2005. SHA-256 of the fetched PDF: `1e06b5cc2e8168263c8f3311190decd5d35187f28cec6f150442a59f546db8cc`.
2. Elektron, *Machinedrum User's Manual*, the manufacturer's primary product
   documentation. Official product/download page:
   <https://www.elektron.se/support-downloads/machinedrum-uw>. The currently
   accessible copy read for page-accurate quotations was
   <https://www.strumentimusicali.net/manuali/ELEKTRON_SPS1UWMKII_ENG.pdf>.
   Its introduction identifies **OS 1.53, revision J**, not OS 1.63; the earlier
   edition label in this note was incorrect. Firmware evidence in the timing
   investigation separately compares actual X.08 and 1.63 images.

The Gärder thesis is physical-modeling research connected to an Elektron
drum-machine project, not a specification of the shipping instrument. The
thesis itself limits the inference: “This work was
initiated by me as a project to find new exciting drum models for a drum
machine being developed by the synthesizer company Elektron ESI AB” (PDF p. 11,
printed p. 1), then says that the scope changed to “a more scientific line of
creating a working model of a rock drum using physical modeling.” It also says
commercial implementations are “corporate secrets” (PDF p. 11, printed p. 1).

## Filter evidence

### Shipping resonant track filter

The thesis does not describe the Machinedrum's per-track resonant filter, give
its topology, coefficients, cutoff mapping, or Q mapping. Its filters belong to
waveguide-mesh experiments: dispersion compensation, spatial interpolation,
and damping. Those equations cannot be relabeled as the product's track filter.

Elektron's manual establishes only the user-visible contract (printed p. 23):

> “The TRACK EFFECT system offers a resonant 24dB lo/hi/band-pass filter.”

It further says FLTF controls a base cutoff, FLTW the distance between high-pass
and low-pass cutoffs, and FLTQ boosts around **both** cutoff frequencies. At the
endpoints one Q is disabled. This supports a compact algorithmic model made from
a resonant high-pass and low-pass section with coupled cutoffs; it does not
establish Butterworth, ladder, state-variable, biquad, pole count beyond the
marketing slope, nonlinear saturation, coefficient interpolation, or exact
parameter laws. In particular, it is not evidence for a fixed output EQ.

### Machine-internal high-pass filters

The paper does not document the EFM machine diagrams or the often-repeated
one-pole formula `H(z)=(1-z^-1)/(1-a z^-1)`. No such formula, EFM machine name,
or Machinedrum parameter map occurs in the original PDF. The paper does contain
first-order all-pass and interpolation-filter equations, but they concern a
digital waveguide mesh, not a shipping EFM voice or global output stage.

**A low-frequency candidate, not a complete coloration model:** a first-order
high-pass/DC-removal stage has the recurrence
`y[n] = x[n] - x[n-1] + a y[n-1]`, with `a = exp(-2πfc/fs)`. Fit only its broad
corner. This is an engineering hypothesis, not a coefficient recovered
from the paper. The paper supplies no value for `a` or `fc` and does not justify
adding that stage to the complete output. It cannot explain the measured treble
roll-off.

## UW sampling evidence

The 2005 physical-modeling paper contains no UW, UserWave, ROM/RAM machine,
12-bit playback, capture-filter, sample-rate-conversion, or interpolation claim.
Its uses of “sampling,” “resampling,” “interpolation,” and “oversampling” concern
numerical waveguide meshes and laboratory drum recordings. They are not evidence
for the later UW sampler.

Elektron's manual does establish these bounded facts:

- Printed p. 71: “All sample speeds from 4 kHz to 48 kHz are accepted and
  handled” and higher rates “are downsampled to half the sampling speed.” It
  also says all bit depths are accepted but “will be played back at 12 bit.”
- Printed p. 71: RAM machines can be filled by “sampling the internal engine or
  sample from the external inputs” and are intended for real-time sampling.
- Printed p. 24: the separately exposed SRR effect “reduces the sample rate using
  sample and hold.” This describes SRR, not normal pitched sample interpolation.

Neither primary source names nearest-neighbour, linear, Lagrange, cubic, sinc,
or polyphase interpolation for normal UW playback. Neither gives an
anti-alias/capture-filter response, order, coefficients, transition band, or
latency. “Downsampled to half” does not specify the converter or its filter.
The manual's 12-bit statement describes playback precision, not the analog DAC
word length and not an analog reconstruction-filter transfer function.

## Analog ADC/DAC boundary

Neither source identifies the ADC/DAC part, analog anti-alias filter, analog
reconstruction filter, or component values. The thesis therefore cannot support
a “Machinedrum output filter” model, and the manual cannot separate digital
track filtering, UW resampling, converters, and analog I/O. Any measured
end-to-end roll-off is their combined response until those stages are isolated.

## Existing implementation is not hardware proof

At `silver-release` revision `890a765`, `modules/silver/PluginProcessorBase.cpp`
uses `createDcBlocker` from `vendor/ephemeral/ephemeral/filter/OnePole.hpp`: a
10 Hz one-pole low-pass subtracted from the input. Vintage mode uses JUCE
Lagrange rate conversion; oversampled mode uses JUCE half-band polyphase IIR.
Voice-specific one-pole HPFs also exist. None establishes a sampler output EQ.
The lone “MD whitepaper” comment in `PluginProcessorBase.hpp` concerns sine-table
size, not filters. Release provenance explicitly preserves historical C++ DSP,
not a verified Machinedrum analog path.

Rivet's `dsp/core/src/md_filter.rs` contains a separate recovered **track filter**
with native instruction fixtures. Its production adapter deliberately uses a
generic TPT filter instead: the recovered open endpoint gave roughly +11 dB at
440 Hz without a validated complete signal context. See Rivet's
`docs/research/md-filter-recovery.md` and the adapter's source comment. Do not
copy this into Ultrawave as a verified fixed output filter.

## Initial fits were hypotheses, not a causal model

Treat the Scarlett as provisionally flat for model exploration; a direct
loopback remains a check, not the presumed explanation for several dB of loss.
The combined measurement does not yet assign coloration to capture, sample
interpolation, the always-running track filter, or analog output.

Exploratory comparison uses the −24 dBFS first sweep. Fit a high-pass corner to
30/100 Hz and low-pass parameters to 2/8/16 kHz, all relative to 1 kHz with no
free gain. Evaluate 4/12/19 kHz on the other five recordings without refitting.
Measured repeats at different levels are repeatability evidence, not independent
proof of filter topology. Comparing candidate families after seeing the spectra
is **not** a blind validation. The later RAM exports below supersede the proposed
capture-side placement.

| Candidate | Fitted low-pass settings | Omitted-frequency RMS / worst error |
|---|---|---|
| Analog RC pole | 10.84 kHz | 1.33 / 2.22 dB |
| Analog second-order Butterworth | 12.88 kHz, fixed Q | 0.56 / 0.69 dB |
| RC + fourth-order analog Butterworth | 11.82 / 20.24 kHz | 0.46 / 0.72 dB |

All include a provisional 17.27 Hz high-pass corner. The second-order candidate
still misses the fitted 8 kHz observation by about 1 dB. Extra stages do not
earn their complexity yet. Bilinear variants at 44.1 kHz and ideal hold/interpolation
magnitude envelopes were also compared; none justifies a causal attribution.
Changing the mathematical domain near Nyquist materially changes the prediction:
an analog response cannot simply be replaced by a host-rate biquad at the same
cutoff and called equivalent.

### New pitched captures test placement without refitting

The subsequent `pitch-01` experiment uses one unchanged captured sweep. The
997 Hz tone measured speed ratios independently of any assumed MD pitch law.
Use the RC 11.82 kHz + Butterworth4 20.24 kHz candidate above, preserving both
corner frequencies and filter orders. Compare 4/8/12/14 kHz at raw PITCH 52 and
4/8/12/14/16 kHz at 76; omit frequencies near the up-pitched alias region.

| Placement | Pitch 52 RMS error | Pitch 76 RMS error |
|---|---:|---:|
| Both filters after playback | 0.90 dB | 0.59 dB |
| Steep section before pitch; RC afterward | 0.38 dB | 0.37 dB |
| RC before pitch; steep section afterward | 0.63 dB | 0.40 dB |

This is evidence for investigating separate pre/post-pitch mechanisms, not proof
of RC/Butterworth circuits or a reason to ship those candidate corners unchanged.
The real interpolator can introduce pitch-dependent filtering of its own. An
input-side effect also should not be reapplied to internal master resampling
that bypasses the analog inputs. That distinction matters more than forcing the
last fraction of a dB out of one normal-pitch magnitude curve.

Use a few physically interpretable corner frequencies and a justified order,
not independent EQ gains, high-order fitted FIR coefficients, or an impulse
response recorded from this one setup. Local numeric evidence for these initial
fits is
`.amp/in/artifacts/response-model-comparison.json`, `pitch-analysis.json`, and
`response-placement-comparison.json` in the same directory; primary hardware
evidence and the signal path are documented in [the measurement note](machinedrum-measurement.md).

## RAM export rules out the proposed steep capture filter

Read-only, checksum-verified SDS exports of RAM1 show external capture nearly
flat: +0.10 dB at 8 kHz, +0.03 dB at 16 kHz, −0.29 dB at 19 kHz relative to
1 kHz. Playback referenced to that stored PCM loses 1.70/5.14/7.98 dB at those
frequencies and still changes with pitch. The split-filter candidate's good
numerical fit did **not** identify its mechanism. Do not put that Butterworth
section into the recorder.

An internal generation adds 3.25/12.86/21.22 dB of loss at 8/16/19 kHz. This
cannot all be analog DAC coloration: the stored-PCM comparison contains no
analog output. A silence-overwrite control verifies that reading ahead within
RAM1 does not create the loss. Master EQ and compression were neutral/bypassed.
Interpolation, always-running track processing and the internal main tap still
need separation. There is no production treble EQ, fitted FIR or convolution.

The repeated low-frequency loss is narrower evidence: direct playback and
internal generation both lose about 1.13 dB at 30 Hz. Production now uses a
conventional normalized first-order **17 Hz high-pass on wet playback**:

```text
a = exp(-2*pi*17/sample_rate)
y[n] = ((1+a)/2) * (x[n]-x[n-1]) + a*y[n-1]
```

It runs after pitch, character, envelope, velocity and output gain, before the
wet-output/internal-feedback split. Dry master audio and external-input capture
are unchanged; internal resampling captures the filtered wet contribution.
The same stage runs in the offline player. State survives voice
boundaries and retriggers, clears on host reset, and has a reported 250 ms tail;
active voices/captures keep the host awake longer when necessary. Float output
can briefly exceed unity after DC removal; this is not integer clipping.

This is a conservative algorithmic approximation, not a whitepaper attribution,
circuit identification or completed sound match. No high-order fitting is
needed for this low corner. The new held-out tone/drum acquisition was blocked
by the Scarlett being in use; remaining treble/interpolation and hardware A/B
work stays explicitly open.
