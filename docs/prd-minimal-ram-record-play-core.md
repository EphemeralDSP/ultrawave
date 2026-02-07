# PRD: Ultrawave Minimal RAM Record/Play Core (No Custom GUI)

## Overview
Build a minimal, reliable core workflow for Ultrawave where `Record` instances capture audio input on MIDI trigger into shared RAM buffers (`R1-R4`), and `Play` instances on the same configured RAM channel play that captured audio on MIDI trigger. This milestone intentionally defers custom GUI work and advanced DSP features.

## Goals
- Deliver a working end-to-end RAM sampling loop with shared buffers.
- Ensure deterministic channel mapping (`R1 REC` feeds `R1 PLAY`, etc.).
- Keep scope minimal to reduce integration risk.
- Ship CLAP + VST3 for macOS/Linux first.

## Quality Gates
These commands must pass for every user story:
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo build --release`

For UI stories only (if any are added later):
- `./scripts/screenshot.sh`
- Visual comparison against `docs/gui_wireframes.md` and `reference/gui_reference.png`

## User Stories

### US-001: Shared RAM Buffer Registry
**Description:** As a plugin instance, I want access to a global shared buffer registry (`R1-R4`) so that Record and Play plugins can exchange audio by channel.

**Acceptance Criteria:**
- [ ] Global registry exposes exactly 4 addressable buffers (`R1-R4`).
- [ ] Record and Play instances can read/write the same underlying channel buffer.
- [ ] Concurrent access is thread-safe and real-time safe (no blocking in audio thread).
- [ ] Unit tests verify write/read correctness across instances for each channel.

### US-002: MIDI-Triggered Fixed-Length Recording
**Description:** As a user, I want `Record` to start capture on MIDI trigger and stop automatically at knob-defined length so that recording is predictable.

**Acceptance Criteria:**
- [ ] `Note On` triggers recording start.
- [ ] Recording duration is fixed by Record `LEN` parameter at trigger time.
- [ ] Recording source is plugin audio input.
- [ ] Recording auto-stops exactly at configured duration.
- [ ] New trigger behavior is defined and tested when recording is already active (ignore/restart policy explicitly implemented).

### US-003: Channel Assignment and Routing
**Description:** As a user, I want each Record/Play instance assigned to one RAM channel (`R1-R4`) so that playback uses the matching recorded content.

**Acceptance Criteria:**
- [ ] Record instance writes only to its configured RAM channel.
- [ ] Play instance reads only from its configured RAM channel.
- [ ] Cross-channel isolation is verified (`R2` data never appears in `R1` playback).
- [ ] Channel selection parameters are host-automatable and stable.

### US-004: MIDI-Triggered One-Shot Playback (Minimal)
**Description:** As a user, I want Play to trigger one-shot mono playback at fixed pitch so I can immediately hear captured audio.

**Acceptance Criteria:**
- [ ] `Note On` triggers playback.
- [ ] Playback is mono one-shot from channel buffer start to end.
- [ ] No envelope/filter/pitch modulation beyond fixed baseline behavior.
- [ ] Empty-buffer trigger behavior is deterministic and non-crashing (silence/no-op).
- [ ] Tests cover trigger and end-of-buffer behavior.

### US-005: Non-Persistent RAM Lifecycle
**Description:** As a user, I expect RAM buffers to be temporary so that state resets after plugin/host reload in this milestone.

**Acceptance Criteria:**
- [ ] Buffer contents are not serialized into plugin state/presets.
- [ ] Host/project reload starts with empty RAM buffers.
- [ ] Behavior is documented in README/notes for this milestone.

### US-006: Platform Build Target (macOS/Linux First)
**Description:** As a maintainer, I want this milestone verified for CLAP+VST3 on macOS/Linux first so delivery is focused.

**Acceptance Criteria:**
- [ ] Release builds succeed for CLAP+VST3 on macOS.
- [ ] Release builds succeed for CLAP+VST3 on Linux.
- [ ] Windows is explicitly marked out-of-scope for this milestone.
- [ ] CI matrix/docs reflect macOS/Linux-first scope.

## Functional Requirements
- FR-1: System must provide a shared, global RAM buffer registry with channels `R1-R4`.
- FR-2: Record must start capture on MIDI `Note On`.
- FR-3: Record must stop automatically after duration determined by Record `LEN` control.
- FR-4: Record must capture from plugin audio input.
- FR-5: Each Record instance must write only to configured channel.
- FR-6: Each Play instance must read only from configured channel.
- FR-7: Play must perform mono one-shot playback on MIDI `Note On`.
- FR-8: Playback must use fixed minimal pitch behavior (no advanced pitch/envelope features).
- FR-9: Empty/uninitialized channel playback must be safe and deterministic.
- FR-10: RAM buffer data must be ephemeral (non-persistent across reload).

## Non-Goals (Out of Scope)
- Custom GUI implementation/styling.
- Time-stretching and pitch-shifting features.
- Modulation matrix/LFO system.
- Advanced effects/filter chains.
- Buffer persistence in presets/project state.
- Windows platform parity in this milestone.

## Technical Considerations
- Shared memory design should remain real-time safe in audio callbacks.
- Existing `src/machines/shared.rs` is the expected integration point.
- Behavior for repeated triggers while active recording must be explicit to avoid undefined state.
- Keep parameter surface minimal but compatible with later expansion.

## Success Metrics
- A user can record from input with `Record` on MIDI trigger and hear playback from matching `Play` channel on trigger.
- No crashes in core trigger/capture/play workflow during normal host use on macOS/Linux.
- All defined quality gates pass for merged stories.

## Open Questions
- Exact voice policy for overlapping playback triggers (retrigger vs ignore vs voice-steal) for minimal mode.
- Exact mapping curve from `LEN` knob (0-127) to recording duration for milestone baseline.
