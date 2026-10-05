# Elektron Machinedrum RAM Machines Reference

> **Note:** Ultrawave adds SRR (sample rate reduction) and DIST (distortion) to capture and playback.
> Both effects are implemented locally; the SRR decimator was copied from Ephemeral under MIT.

---

## RAM RECORD (RAM-R1 - RAM-R4)

These machines capture sound (internal and external). 4 RAM RECORD machines available. DSP memory is shared with ROM machines - more memory = higher recording quality.

### Parameters (8 knobs)

| Param | Name | Description |
|-------|------|-------------|
| **MLEV** | Main Level | Level of internal sound rerouted from Main outputs. 0 = as-is, positive = boost, negative = cut |
| **MBAL** | Main Balance | Mono recording balance from Main outs |
| **ILEV** | Input Level | Level of external input signal. 0 = as-is, positive = boost, negative = cut |
| **IBAL** | Input Balance | Mono recording balance from external inputs |
| **CUE1** | Cue 1 | Pre-listen level for Main outputs or external inputs. ⚠️ Can cause feedback if too high |
| **CUE2** | Cue 2 | Pre-listen level of what will be recorded when triggered |
| **LEN** | Length | Recording duration. 127 = 2 bars. Each value = ¼ step. Lower if low on DSP memory |
| **RATE** | Rate | Sample rate/quality. Lower = reduced quality |

---

## RAM PLAY (RAM-P1 - RAM-P4)

Plays back sound captured by corresponding RAM RECORD machine. Functions like ROM machines 25-48.

### Parameters (8 knobs)

| Param | Name | Description |
|-------|------|-------------|
| **PTCH** | Pitch | Playback pitch/tuning |
| **DEC** | Decay | Amplitude decay time |
| **HOLD** | Hold | Sustain/hold time |
| **BRR** | Bit Rate Reduction | Lo-fi bitcrusher effect |
| **STRT** | Start | Sample start point |
| **END** | End | Sample end point |
| **RTRG** | Retrigger | Retrigger rate |
| **RTIM** | Retrigger Time | Retrigger timing/interval |
