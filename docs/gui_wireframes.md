# Ultrawave GUI Wireframes

ASCII wireframes based on Elektron Machinedrum UW aesthetic (1-bit monochrome, hardware-inspired).

Reference image: [reference/gui_reference.png](../reference/gui_reference.png)

---

## Ultrawave Record

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           ULTRAWAVE RECORD                                  │
├───────────────┬─────────────────────────────────────────────────────────────┤
│  LEV  SRR DST │  ┌──────────┬──────────┬──────────┬──────────┐              │
│ ┃███┃░░░┃███┃ │  │   MLEV   │   MBAL   │   ILEV   │   IBAL   │              │
│ ┃███┃░░░┃███┃ │  │    ◯     │    ◯     │    ◯     │    ◯     │              │
│ ┃█▓▓┃▓▓▓┃▓▓▓┃ │  │    0     │   L-R    │    0     │   L-R    │              │
│ ┃▓▓▓┃▓▓▓┃▓▓▓┃ │  ├──────────┼──────────┼──────────┼──────────┤              │
│  0.0  OFF OFF │  │   CUE1   │   CUE2   │   LEN    │   RATE   │              │
│               │  │    ◯     │    ◯     │    ◯     │    ◯     │              │
│ ▌▌ R1 ▐▐      │  │   -inf   │   -inf   │  2 BAR   │   HI     │              │
│  RECORDING    │  └──────────┴──────────┴──────────┴──────────┘              │
│               │                                                             │
│               │  ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅▄▃▂▁  TIME: 2.4s / 10.0s       │
├───────────────┴─────────────────────────────────────────────────────────────┤
│  RAM > R1 > REC                                         [R1] [R2] [R3] [R4]│
└─────────────────────────────────────────────────────────────────────────────┘
```

### Record Parameters

**Sidebar (3 vertical sliders side by side):**
- **LEV** - Output level (vertical slider)
- **SRR** - Sample rate reduction (vertical slider, from ephemeraldsp/Ephemeral)
- **DST** - Distortion (vertical slider, from ephemeraldsp/Ephemeral)

**Main Grid (8 knobs):**
| Top Row | Bottom Row |
|---------|------------|
| MLEV - Main level | CUE1 - Pre-listen main/ext |
| MBAL - Main balance | CUE2 - Pre-listen recording |
| ILEV - Input level | LEN - Recording length |
| IBAL - Input balance | RATE - Sample rate/quality |

---

## Ultrawave Play

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            ULTRAWAVE PLAY                                   │
├───────────────┬─────────────────────────────────────────────────────────────┤
│  LEV  SRR DST │  ┌──────────┬──────────┬──────────┬──────────┐              │
│ ┃███┃░░░┃███┃ │  │   PTCH   │   DEC    │   HOLD   │   BRR    │              │
│ ┃███┃░░░┃███┃ │  │    ◯     │    ◯     │    ◯     │    ◯     │              │
│ ┃█▓▓┃▓▓▓┃▓▓▓┃ │  │   +0.00  │  100ms   │   INF    │   OFF    │              │
│ ┃▓▓▓┃▓▓▓┃▓▓▓┃ │  ├──────────┼──────────┼──────────┼──────────┤              │
│  0.0  OFF OFF │  │   STRT   │   END    │   RTRG   │   RTIM   │              │
│               │  │    ◯     │    ◯     │    ◯     │    ◯     │              │
│ ▌▌ P01 ▐▐     │  │    0%    │   100%   │   OFF    │   1/16   │              │
│  SRC: R1      │  └──────────┴──────────┴──────────┴──────────┘              │
│               │                                                             │
│               │  ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅▄▃▂▁  [▌    ▐] STRT/END        │
├───────────────┴─────────────────────────────────────────────────────────────┤
│  RAM > R1 > PLAY                                        [R1] [R2] [R3] [R4]│
└─────────────────────────────────────────────────────────────────────────────┘
```

### Play Parameters

**Sidebar (3 vertical sliders side by side):**
- **LEV** - Output level (vertical slider)
- **SRR** - Sample rate reduction (vertical slider, from ephemeraldsp/Ephemeral)
- **DST** - Distortion (vertical slider, from ephemeraldsp/Ephemeral)

**Main Grid (8 knobs):**
| Top Row | Bottom Row |
|---------|------------|
| PTCH - Pitch/tuning | STRT - Sample start point |
| DEC - Decay time | END - Sample end point |
| HOLD - Hold/sustain | RTRG - Retrigger rate |
| BRR - Bit rate reduction | RTIM - Retrigger timing |

---

## Layout Notes

- **Sidebar (left ~20%):** Level meter + 3 vertical effect sliders (side by side, matching knob grid height) + buffer status
- **Main area (right ~80%):** 4×2 grid of rotary encoders
- **Footer:** Breadcrumb navigation + buffer selector tabs [R1-R4]
- **Waveform display:** Shows recorded/playback buffer with start/end markers
