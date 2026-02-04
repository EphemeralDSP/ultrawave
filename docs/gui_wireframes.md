# Ultrawave GUI Wireframes

ASCII wireframes based on Elektron Machinedrum UW aesthetic (1-bit monochrome, hardware-inspired).

Reference image: [reference/gui_reference.png](../reference/gui_reference.png)

---

## Ultrawave Record

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           ULTRAWAVE RECORD                                  │
├────────────┬────────────────────────────────────────────────────────────────┤
│    ◯ LEV   │  ┌──────────┬──────────┬──────────┬──────────┐                 │
│     0.0    │  │   MLEV   │   MBAL   │   ILEV   │   IBAL   │                 │
│            │  │    ◯     │    ◯     │    ◯     │    ◯     │                 │
│    ◯ SRR   │  │    0     │   L-R    │    0     │   L-R    │                 │
│     OFF    │  ├──────────┼──────────┼──────────┼──────────┤                 │
│            │  │   CUE1   │   CUE2   │   LEN    │   RATE   │                 │
│    ◯ DIST  │  │    ◯     │    ◯     │    ◯     │    ◯     │                 │
│     OFF    │  │   -inf   │   -inf   │  2 BAR   │   HI     │                 │
│            │  └──────────┴──────────┴──────────┴──────────┘                 │
│ ▌▌ R1 ▐▐   │                                                                │
│  RECORDING │  ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅▄▃▂▁  TIME: 2.4s / 10.0s            │
├────────────┴────────────────────────────────────────────────────────────────┤
│  RAM > R1 > REC                                          [R1] [R2] [R3] [R4]│
└─────────────────────────────────────────────────────────────────────────────┘
```

### Record Parameters

**Sidebar (3 knobs):**
- **LEV** - Output level
- **SRR** - Sample rate reduction (from ephemeraldsp/Ephemeral)
- **DIST** - Distortion (from ephemeraldsp/Ephemeral)

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
├────────────┬────────────────────────────────────────────────────────────────┤
│    ◯ LEV   │  ┌──────────┬──────────┬──────────┬──────────┐                 │
│     0.0    │  │   PTCH   │   DEC    │   HOLD   │   BRR    │                 │
│            │  │    ◯     │    ◯     │    ◯     │    ◯     │                 │
│    ◯ SRR   │  │   +0.00  │  100ms   │   INF    │   OFF    │                 │
│     OFF    │  ├──────────┼──────────┼──────────┼──────────┤                 │
│            │  │   STRT   │   END    │   RTRG   │   RTIM   │                 │
│    ◯ DIST  │  │    ◯     │    ◯     │    ◯     │    ◯     │                 │
│     OFF    │  │    0%    │   100%   │   OFF    │   1/16   │                 │
│            │  └──────────┴──────────┴──────────┴──────────┘                 │
│ ▌▌ P01 ▐▐  │                                                                │
│  SRC: R1   │  ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅▄▃▂▁  [▌    ▐] STRT/END             │
├────────────┴────────────────────────────────────────────────────────────────┤
│  RAM > R1 > PLAY                                         [R1] [R2] [R3] [R4]│
└─────────────────────────────────────────────────────────────────────────────┘
```

### Play Parameters

**Sidebar (3 knobs):**
- **LEV** - Output level
- **SRR** - Sample rate reduction (from ephemeraldsp/Ephemeral)
- **DIST** - Distortion (from ephemeraldsp/Ephemeral)

**Main Grid (8 knobs):**
| Top Row | Bottom Row |
|---------|------------|
| PTCH - Pitch/tuning | STRT - Sample start point |
| DEC - Decay time | END - Sample end point |
| HOLD - Hold/sustain | RTRG - Retrigger rate |
| BRR - Bit rate reduction | RTIM - Retrigger timing |

---

## Layout Notes

- **Sidebar (left ~20%):** Level meter + 3 effect knobs + buffer status
- **Main area (right ~80%):** 4×2 grid of rotary encoders
- **Footer:** Breadcrumb navigation + buffer selector tabs [R1-R4]
- **Waveform display:** Shows recorded/playback buffer with start/end markers
