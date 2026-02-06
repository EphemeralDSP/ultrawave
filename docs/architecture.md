# Ultrawave Audio Processing Architecture

This document describes the core audio processing architecture for the Ultrawave VST3/CLAP plugin, an emulation of the Elektron Machinedrum UW RAM machines.

## Overview

Ultrawave consists of two primary machine types working together:
- **RAM RECORD**: Captures incoming audio to shared buffers
- **RAM PLAY**: Plays back recorded samples with pitch, envelope, and effect control

## Signal Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              SIGNAL FLOW                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐                                                           │
│  │ Audio Input  │                                                           │
│  │ (Main + Ext) │                                                           │
│  └──────┬───────┘                                                           │
│         │                                                                    │
│         ▼                                                                    │
│  ┌──────────────┐     ┌──────────────┐                                     │
│  │   Mixer      │────▶│  RAM RECORD  │                                     │
│  │ (MLEV/MBAL)  │     │  (R1-R4)     │                                     │
│  └──────────────┘     └──────┬───────┘                                     │
│                              │                                              │
│                              ▼                                              │
│                    ┌──────────────────┐                                    │
│                    │  Shared Buffer   │                                    │
│                    │  Registry        │                                    │
│                    │  (global static) │                                    │
│                    └────────┬─────────┘                                    │
│                             │                                               │
│         ┌───────────────────┼───────────────────┐                          │
│         │                   │                   │                          │
│         ▼                   ▼                   ▼                          │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐                 │
│  │  RAM PLAY    │    │  RAM PLAY    │    │  RAM PLAY    │                 │
│  │  Machine 1   │    │  Machine 2   │    │  Machine N   │                 │
│  └──────┬───────┘    └──────┬───────┘    └──────┬───────┘                 │
│         │                   │                   │                          │
│         └───────────────────┼───────────────────┘                          │
│                             │                                              │
│                             ▼                                              │
│                    ┌──────────────────┐                                    │
│                    │  Sample Engine   │                                    │
│                    │  - Pitch shift   │                                    │
│                    │  - SRR           │                                    │
│                    │  - Envelope      │                                    │
│                    └────────┬─────────┘                                    │
│                             │                                              │
│                             ▼                                              │
│                    ┌──────────────────┐                                    │
│                    │  Effects Chain   │                                    │
│                    │  - Filter (LP/BP/│                                    │
│                    │    HP)           │                                    │
│                    │  - SRR           │                                    │
│                    │  - Distortion    │                                    │
│                    └────────┬─────────┘                                    │
│                             │                                              │
│                             ▼                                              │
│                    ┌──────────────────┐                                    │
│                    │  Output Mix      │                                    │
│                    └──────────────────┘                                    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Component Interaction Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          COMPONENT INTERACTION                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────┐         ┌─────────────────┐                           │
│  │   VST3/CLAP     │         │    DAW/host     │                           │
│  │    Plugin       │◄───────▶│   (Ableton,     │                           │
│  │   (Ultrawave)   │  MIDI   │   Reaper, etc)  │                           │
│  └────────┬────────┘         └─────────────────┘                           │
│           │                                                                  │
│           │ process() calls                                                  │
│           ▼                                                                  │
│  ┌─────────────────┐         ┌─────────────────┐                           │
│  │   Parameters    │◄───────▶│    nih-plug     │                           │
│  │   (Ultrawave    │         │   framework     │                           │
│  │    Params)      │         │                 │                           │
│  └────────┬────────┘         └─────────────────┘                           │
│           │                                                                  │
│           │ uses                                                             │
│           ▼                                                                  │
│  ┌─────────────────┐         ┌─────────────────┐                           │
│  │   RAM Record    │◄───────▶│  Shared Buffer  │                           │
│  │   Machine       │ writes  │    Registry     │                           │
│  │                 │         │   (global)      │                           │
│  └─────────────────┘         └────────┬────────┘                           │
│                                       │                                     │
│                                       │ reads                                 │
│                                       ▼                                     │
│                              ┌─────────────────┐                           │
│                              │    RAM Play     │                           │
│                              │    Machine      │                           │
│                              └────────┬────────┘                           │
│                                       │                                     │
│           ┌───────────────────────────┼───────────────────┐                │
│           │                           │                   │                │
│           ▼                           ▼                   ▼                │
│  ┌─────────────────┐      ┌─────────────────┐  ┌─────────────────┐        │
│  │  Sample Engine  │      │  Effects (SRR,  │  │  Filter (24dB   │        │
│  │  - Interpolation│      │  Distortion)    │  │  resonant)      │        │
│  │  - Pitch shift  │      │                 │  │                 │        │
│  └─────────────────┘      └─────────────────┘  └─────────────────┘        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow for Recording

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         RECORDING DATA FLOW                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. INPUT MIXING                                                             │
│     ┌──────────┐    ┌──────────┐                                            │
│     │ Main L/R │───▶│  MLEV    │───┐                                        │
│     └──────────┘    │  MBAL    │   │                                        │
│                     └──────────┘   │    ┌──────────┐                        │
│     ┌──────────┐    ┌──────────┐   └───▶│  Mixer   │                        │
│     │ Input L/R│───▶│  ILEV    │───────▶│  Output  │                        │
│     └──────────┘    │  IBAL    │        └────┬─────┘                        │
│                     └──────────┘             │                               │
│                                              ▼                               │
│  2. 12-BIT QUANTIZATION                      ┌──────────┐                   │
│     f32 sample ──▶ clamp(-1.0, 1.0) ──▶     │ 12-bit   │                   │
│     ──▶ (sample * 2048.0).round()           │  i16     │                   │
│     ──▶ clamp(-2048, 2047)                  └────┬─────┘                   │
│                                                  │                           │
│  3. RATE CONTROL                                 ▼                           │
│     ┌──────────┐    ┌──────────┐    ┌──────────────────┐                  │
│     │ rec_rate │───▶│ divisor  │───▶│ Counter-based    │                  │
│     │ (0-127)  │    │ (1-16x)  │    │ downsampling     │                  │
│     └──────────┘    └──────────┘    └──────────────────┘                  │
│                                                  │                           │
│  4. BUFFER STORAGE                               ▼                           │
│                                        ┌──────────────────┐                │
│                                        │  Vec<i16>        │                │
│                                        │  (per channel)   │                │
│                                        └──────────────────┘                │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow for Playback

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          PLAYBACK DATA FLOW                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. TRIGGER & SETUP                                                          │
│     MIDI Note On ──▶ Load buffer from registry ──▶ Set start position       │
│     ──▶ Initialize envelope (1.0) ──▶ Set hold/decay times                  │
│                                                                              │
│  2. SAMPLE READING                                                           │
│     ┌──────────┐    ┌──────────┐    ┌──────────┐                           │
│     │  pitch   │───▶│  ratio   │───▶│  index   │                           │
│     │ (0-127)  │    │calc      │    │ increment│                           │
│     └──────────┘    └──────────┘    └────┬─────┘                           │
│                                          │                                  │
│                                          ▼                                  │
│     ┌──────────┐    ┌──────────┐    ┌──────────┐                           │
│     │ Buffer   │◄───│  Linear  │◄───│ Position │                           │
│     │ (i16)    │    │  Interp  │    │ (f64)    │                           │
│     └────┬─────┘    └──────────┘    └──────────┘                           │
│          │                                                                  │
│          ▼                                                                  │
│     ┌──────────┐                                                           │
│     │ f32      │                                                           │
│     │ sample   │                                                           │
│     └────┬─────┘                                                           │
│          │                                                                  │
│  3. SRR (Sample Rate Reduction)                                            │
│          │           ┌──────────┐                                           │
│          ├──────────▶│  Hold &  │───▶ (repeated samples)                    │
│          │           │  count   │                                           │
│          │           └──────────┘                                           │
│          │                                                                  │
│  4. ENVELOPE PROCESSING                                                      │
│          │           ┌──────────┐                                           │
│          ├──────────▶│  Hold    │───▶ (maintains full volume)               │
│          │           │  phase   │                                           │
│          │           └──────────┘                                           │
│          │                   │                                               │
│          │                   ▼                                               │
│          │           ┌──────────┐                                           │
│          └──────────▶│  Decay   │───▶ (linear ramp to 0)                    │
│                      │  phase   │                                           │
│                      └──────────┘                                           │
│                              │                                               │
│  5. GAIN & OUTPUT              ▼                                             │
│                         ┌──────────┐    ┌──────────┐                        │
│                         │ envelope │───▶│  volume  │                        │
│                         │ * sample │    │  (0-127) │                        │
│                         └──────────┘    └────┬─────┘                        │
│                                              │                               │
│                                              ▼                               │
│                                         ┌──────────┐                        │
│                                         │  f32     │                        │
│                                         │  output  │                        │
│                                         └──────────┘                        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Core Data Structures

### Sample Buffer Representation

```rust
/// Maximum recording length in seconds
const MAX_RECORD_SECONDS: f32 = 10.0;

/// Internal sample storage - 12-bit samples stored as i16
/// Range: -2048 to 2047 (preserves Elektron's 12-bit character)
pub type SampleBuffer = Vec<i16>;

/// Shared buffer registry - global singleton for inter-plugin communication
pub struct SharedBufferRegistry {
    buffers: [Arc<RwLock<SampleBuffer>>; 4],  // R1-R4
    metadata: [BufferMetadata; 4],
}

pub struct BufferMetadata {
    pub sample_rate: f32,
    pub channels: u8,
    pub is_recording: AtomicBool,
    pub last_modified: Instant,
}
```

### Machine State

```rust
/// Machine types supported by Ultrawave
pub enum MachineType {
    /// Records audio to shared buffer
    RamRecord,
    /// Plays back from shared buffer
    RamPlay,
}

/// State for a single RAM RECORD machine
pub struct RamRecord {
    /// Per-channel sample buffers (up to 8 channels)
    buffers: [Vec<i16>; 8],
    write_positions: [usize; 8],
    max_length: usize,
    target_length: usize,
    is_recording: [bool; 8],
    sample_rate: f32,
    rec_rate_counter: f32,
    rec_rate_divisor: f32,
}

/// State for a single RAM PLAY machine  
pub struct RamPlay {
    /// Per-channel sample engines
    engines: [SampleEngine; 8],
    envelopes: [f32; 8],
    is_playing: [bool; 8],
    hold_time_samples: [usize; 8],
    hold_counters: [usize; 8],
    decay_rates: [f32; 8],
    sample_rate: f32,
    retrig_counters: [usize; 8],
    retrig_intervals: [usize; 8],
    retrigs_remaining: [usize; 8],
    current_params: [RamPlayParams; 8],
}
```

### Parameter Smoothing

```rust
/// Smooths parameter changes to avoid zipper noise
pub struct SmoothedParam {
    current: f32,
    target: f32,
    /// Time constant in samples (determines smoothing speed)
    coefficient: f32,
}

impl SmoothedParam {
    pub fn new(initial: f32, time_constant_ms: f32, sample_rate: f32) -> Self {
        let coefficient = 1.0 - (-1000.0 / (time_constant_ms * sample_rate)).exp();
        Self {
            current: initial,
            target: initial,
            coefficient,
        }
    }
    
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }
    
    pub fn next(&mut self) -> f32 {
        self.current += (self.target - self.current) * self.coefficient;
        self.current
    }
}
```

## Processing Chain Design

```
Input ──▶ RAM RECORD ──▶ Shared Buffer ──▶ RAM PLAY ──▶ Track Effects ──▶ Output
                                              │              │
                                              │              ▼
                                              │       ┌──────────────┐
                                              │       │ 24dB Filter  │
                                              │       │ - Low Pass   │
                                              │       │ - Band Pass  │
                                              │       │ - High Pass  │
                                              │       └──────────────┘
                                              │              │
                                              │              ▼
                                              │       ┌──────────────┐
                                              │       │     SRR      │
                                              │       └──────────────┘
                                              │              │
                                              │              ▼
                                              │       ┌──────────────┐
                                              └──────▶│  Distortion  │
                                                      └──────────────┘
```

### Processing Order

1. **RAM RECORD** (if enabled)
   - Mix main + external inputs
   - Apply level/balance controls
   - Quantize to 12-bit
   - Write to buffer with rate control

2. **RAM PLAY** (per triggered voice)
   - Read sample with linear interpolation
   - Apply pitch shift
   - Apply sample rate reduction (SRR)
   - Apply envelope (hold + decay phases)
   - Apply velocity/volume scaling

3. **Track Effects** (per sample)
   - 24dB resonant filter (LP/BP/HP)
   - Additional SRR stage
   - Distortion/saturation

4. **Output**
   - Stereo output
   - Master gain control

## Memory Layout Plan

### Shared Buffer Pool

```
┌────────────────────────────────────────────────────────────┐
│                    SHARED BUFFER POOL                       │
├────────────────────────────────────────────────────────────┤
│                                                             │
│  Buffer R1: Vec<i16>                                        │
│  ├─ Max capacity: sample_rate * 10s * sizeof(i16)          │
│  ├─ Typical: 44100 * 10 * 2 = 882 KB @ 44.1kHz             │
│  └─ Thread-safe: Arc<RwLock<_>>                             │
│                                                             │
│  Buffer R2: Vec<i16> (same layout)                         │
│  Buffer R3: Vec<i16> (same layout)                         │
│  Buffer R4: Vec<i16> (same layout)                         │
│                                                             │
│  Total Shared Memory: ~3.5 MB (44.1kHz)                    │
│                      ~7.7 MB (96kHz)                       │
│                                                             │
└────────────────────────────────────────────────────────────┘
```

### Per-Machine Allocation

```
┌────────────────────────────────────────────────────────────┐
│                  PER-MACHINE MEMORY                         │
├────────────────────────────────────────────────────────────┤
│                                                             │
│  RAM RECORD (per instance):                                │
│  ├─ 8x Vec<i16> buffers (pre-allocated with capacity)      │
│  ├─ 8x usize write positions                               │
│  ├─ 8x bool recording flags                                │
│  └─ ~16 KB metadata                                        │
│                                                             │
│  RAM PLAY (per instance):                                  │
│  ├─ 8x SampleEngine (reference to shared buffers)          │
│  ├─ 8x f32 envelope states                                 │
│  ├─ 8x bool playing flags                                  │
│  ├─ 8x usize timing counters                               │
│  └─ ~8 KB state                                            │
│                                                             │
│  SampleEngine (per voice):                                 │
│  ├─ buffer reference (shared)                              │
│  ├─ f64 playback position                                  │
│  ├─ f32 SRR counter                                        │
│  └─ ~32 bytes per engine                                   │
│                                                             │
└────────────────────────────────────────────────────────────┘
```

### Thread-Safety Approach

```rust
// Global singleton using once_cell for safe initialization
use once_cell::sync::Lazy;
use std::sync::{Arc, RwLock};

static BUFFER_REGISTRY: Lazy<SharedBufferRegistry> = Lazy::new(|| {
    SharedBufferRegistry::new()
});

// Read pattern (playback)
let buffer = BUFFER_REGISTRY.buffers[channel].read().unwrap();
let sample = buffer[position];

// Write pattern (recording)
let mut buffer = BUFFER_REGISTRY.buffers[channel].write().unwrap();
buffer.push(sample);

// Clone for playback engine
let buffer_clone = buffer.clone();
drop(buffer);  // Release lock
engine.load_buffer(buffer_clone);
```

## Technical Decisions

### 1. Buffer Format: 12-bit stored as i16 vs native f32

**Decision**: Store samples as `i16` with 12-bit range (-2048 to 2047)

**Rationale**:
- Authentic to Elektron Machinedrum UW hardware
- Reduces memory usage by 50% vs f32
- Slight quantization noise adds character
- Easy conversion: `(sample as f32 / 2048.0)` for processing

### 2. Interpolation: Linear vs Sinc

**Decision**: Linear interpolation

**Rationale**:
- Matches original hardware behavior
- Lower CPU usage
- Sufficient quality for the target sound
- Can be upgraded to Sinc later if needed

### 3. Parameter Smoothing Time Constant

**Decision**: 3ms default (configurable 1-5ms)

**Rationale**:
- Fast enough for musical response
- Slow enough to prevent zipper noise
- Tested with automation sweeps

### 4. LFO Rate Table Implementation

**Decision**: Pre-calculated table with 128 entries

**Rationale**:
- Matches Elektron's 0-127 parameter range
- Avoids runtime frequency calculations
- Allows non-linear curves (exp for fast rates, log for slow)

### 5. Filter Design: Biquad cascade vs Ladder

**Decision**: Single biquad stage per mode

**Rationale**:
- Simpler implementation
- Sufficient for LP/BP/HP modes
- Lower CPU than 4-stage ladder
- Can cascade for steeper slopes if needed

## File Organization

```
src/
├── lib.rs                    # Plugin entry point
├── params.rs                 # Parameter definitions
├── editor.rs                 # VIZIA GUI setup
├── dsp/
│   ├── mod.rs               # DSP module exports
│   ├── effects.rs           # SRR, distortion
│   ├── filter.rs            # 24dB resonant filter
│   └── sample_engine.rs     # Playback with pitch/SRR
├── machines/
│   ├── mod.rs               # Machine exports
│   ├── ram_record.rs        # Recording logic
│   ├── ram_play.rs          # Playback logic
│   └── shared.rs            # Global buffer registry
├── standalone/
│   ├── mod.rs               # Standalone binary exports
│   ├── record.rs            # Record standalone
│   ├── record_editor.rs     # Record GUI
│   ├── play.rs              # Play standalone
│   └── play_editor.rs       # Play GUI
└── widgets/
    ├── mod.rs               # Custom widgets
    └── waveform.rs          # Waveform display
```

## Performance Targets

- **CPU Usage**: < 5% single instance on modern hardware
- **Latency**: Sample-accurate MIDI timing
- **Polyphony**: 16 voices minimum
- **Memory**: < 10 MB total per plugin instance
- **Buffer Size**: Supports 16-2048 samples

## Future Considerations

1. **Multi-timbral support**: Multiple machines per track
2. **Time-stretching**: NORMAL/BEAT algorithms (Octatrack-style)
3. **Slice mode**: Automatic sample slicing
4. **Crossfade looping**: Smooth loop points
5. **MPE support**: Per-note expression
