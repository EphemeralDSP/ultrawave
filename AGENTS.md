# Agent Instructions

## Compatibility Policy

Until Jeff explicitly says otherwise, backward compatibility is not a requirement:
there are no existing patches to preserve. When making changes, prioritize
correctness and simplicity over preserving old presets/projects, parameter or
plugin IDs, state/shared-memory formats, or APIs. Compatibility shims and
migrations are unnecessary unless Jeff requests them.

## Project Overview

**Ultrawave** is one MIDI-controlled VST3/CLAP audio effect for real-time RAM
sampling, inspired by the Elektron Machinedrum UW. Each instance owns private,
volatile RAM for one mono sample. Stereo main/sidechain feed capture before the
sample-only output is written. Output is silent unless a sample is playing.
Renoise controls the existing effect through an FX alias instrument, not a
second sampler. See README.md for routing, state and trigger semantics.

### Key Files

- `src/plugin/sampler.rs` - Combined effect and MIDI/automation callback
- `src/plugin/editor.rs` - Single-pane Capture/Play editor and MIDI dropdowns
- `src/machines/memory.rs` - Private atomic sample RAM and UI snapshots
- `src/machines/ram_record.rs` - Recording DSP
- `src/machines/ram_play.rs` - Playback DSP with pitch/envelope
- `technical_specs.md` - Elektron hardware specs for reference

### Build Commands

```bash
cargo build                                    # Debug build
cargo build --release                          # Release build
cargo xtask bundle ultrawave-plugin --release  # Bundle VST3 + CLAP effect
cargo test                                     # Run tests
```

### Screenshot Tests (Linux/CI)

Capture GUI screenshots for LLM feedback on plugin UI. Requires Linux or WSL with Xvfb + ImageMagick.

```bash
# Install deps (Ubuntu/Debian)
sudo apt install xvfb imagemagick

# Capture screenshots locally
./scripts/screenshot.sh              # Empty and loaded sample
./scripts/screenshot.sh empty
./scripts/screenshot.sh loaded

# Custom output directory
SCREENSHOT_DIR=/tmp/shots ./scripts/screenshot.sh
```

Screenshots save to `screenshots/ultrawave_empty.png` and `ultrawave.png`.

#### Comparing Against Wireframes

For GUI work, read `docs/gui_wireframes.md`. The approved design is one 920×650
pane with Capture and Play always visible, one shared waveform and small MIDI
dropdowns. One instance owns one take; a new capture replaces it and stops old
playback. Hardware artwork is inspiration, not a pixel-match requirement.

Immediately after GUI changes:
1. Build with `cargo build --release` and run `./scripts/screenshot.sh`.
2. **Open and inspect both images**, checking clipping, labels and action visibility.
3. Exercise changed controls in the running editor. Inspect non-default states:
   open MIDI/dropdowns, capture details, reverse markers and Record once protection.
4. Use the exported CLAP harness and isolated Renoise fixture for audio/state;
   a screenshot alone does not verify those behaviors.

---

## Issue Tracking

This project uses **bd (beads)** for issue tracking.
Run `bd prime` for workflow context, or install hooks (`bd hooks install`) for auto-injection.

**Quick reference:**
- `bd ready` - Find unblocked work
- `bd create "Title" --type task --priority 2` - Create issue
- `bd close <id>` - Complete work
- `bd sync` - Sync with git (run at session end)

For full workflow details: `bd prime`

## Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --status in_progress  # Claim work
bd close <id>         # Complete work
bd sync               # Sync with git
```

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd sync
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
