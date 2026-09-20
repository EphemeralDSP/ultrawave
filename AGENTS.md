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
volatile R1–R4 mono RAM. Stereo main/sidechain feed capture before playback is
added; dry stereo passes through by default. Playback Only suppresses dry.
Renoise controls the existing effect through an FX alias instrument, not a
second sampler. See README.md for routing, state and trigger semantics.

### Key Files

- `src/plugin/sampler.rs` - Combined effect and MIDI/automation callback
- `src/plugin/editor.rs` - Record/Play pages in one egui editor
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
./scripts/screenshot.sh              # Both pages
./scripts/screenshot.sh record       # Record page
./scripts/screenshot.sh play         # Play page

# Custom output directory
SCREENSHOT_DIR=/tmp/shots ./scripts/screenshot.sh
```

Screenshots save to `screenshots/ultrawave_record.png` and `ultrawave_play.png`.

#### Comparing Against Wireframes

The GUI should match the wireframes in `docs/gui_wireframes.md`. When reviewing screenshots, check:

**Record Page Layout:**
- Sidebar (left ~20%): Level meter + 3 effect knobs (LEV, SRR, DIST) + buffer status
- Main area (right ~80%): 4×2 grid (MAIN, BAL, SIDE, BAL / REC NOTE, PLAY NOTE, LEN, RATE)
- Footer: Record/Play pages, output routing, buffer selector tabs [R1-R4]
- Waveform display: Shows recorded buffer with time indicator

**Play Page Layout:**
- Sidebar (left ~20%): Level meter + 3 effect knobs (LEV, SRR, DIST) + buffer status
- Main area (right ~80%): 4×2 grid (PTCH, DEC, HOLD, BRR / STRT, END, RTRG, RTIM)
- Footer: Same page/output/buffer controls as Record
- Waveform display: Shows playback buffer with start/end markers

**Acceptance Criteria for GUI Tasks:**
1. Build plugin: `cargo build --release`
2. Capture screenshot: `./scripts/screenshot.sh [record|play]`
3. **OPEN AND EXAMINE THE SCREENSHOT FILE** - Look at `screenshots/ultrawave_record.png` and/or `screenshots/ultrawave_play.png`
4. Compare against wireframe in `docs/gui_wireframes.md` AND the reference image at `reference/gui_reference.png`
5. All wireframe elements must be present and visually match

**⚠️ CRITICAL: YOU MUST VISUALLY VERIFY SCREENSHOTS ⚠️**

DO NOT mark GUI tasks as complete without ACTUALLY LOOKING at the screenshot files. Previous agents failed by claiming work was done while the GUI had major visual discrepancies:

- **Rounded corners** instead of sharp rectangular borders (1-bit hardware aesthetic)
- **Dark theme with cyan accents** instead of 1-bit monochrome
- **Missing brackets** around buffer selector tabs (`[R1]` vs `R1`)
- **Wrong knob style** - should be rotary encoders with tick marks, not circular outlines
- **Missing waveform markers** - start/end indicators required on Play plugin
- **Incorrect buffer status display** - needs bar indicators (`▌▌ R1 ▐▐`)

**THE REFERENCE IMAGE IS THE SOURCE OF TRUTH.** The GUI must match the Elektron Machinedrum UW 1-bit monochrome hardware aesthetic. When in doubt, favor sharp angles, rectangular borders, and stark contrast over rounded "modern" UI elements.

**FAILURE TO VERIFY SCREENSHOTS = WORK NOT DONE.**

### Agent Reminder: Always Screenshot Test GUI Changes

**When making ANY UI/GUI changes, you MUST:**
1. Build the plugin (`cargo build --release`)
2. Run screenshot test (`./scripts/screenshot.sh`)
3. **Visually inspect** both `screenshots/ultrawave_record.png` and `screenshots/ultrawave_play.png`
4. Compare against `reference/gui_reference.png`
5. Verify all elements are properly positioned and visually correct

**Do NOT consider GUI work complete without visual verification!**

**IMPORTANT:** Screenshot verification must be done *immediately* after making GUI changes, NOT deferred to "landing the plane". The build + screenshot + verify cycle should be part of the same task completion flow. Previous agents deferred this step and forgot to do it.

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
