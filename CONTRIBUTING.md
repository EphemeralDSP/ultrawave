# Contributing to Ultrawave

Thank you for your interest in contributing to Ultrawave!

## Getting Started

1. Fork and clone the repository
2. Install Rust via [rustup](https://rustup.rs/)
3. Build the project: `cargo build`
4. Run tests: `cargo test`

## Development Workflow

### Building

Requires Rust 1.95+. Linux development packages are listed in
[`ci/scripts/deps-linux.sh`](ci/scripts/deps-linux.sh). No private repository
access or dependency credentials are needed.

NICE-plug/core 0.4.2 and egui adapter 0.5.1 are pinned. The framework is ISC;
VST3 bindings are MIT/Apache-2.0. See the [vendor patches](vendor/README.md).

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Bundle all plugin formats
cargo xtask bundle ultrawave-plugin --release

# Install the bundles locally (close your host first)
cargo xtask install
```

Bundles are written to `target/bundled/ultrawave-plugin.{vst3,clap}`. See the
[installation guide](docs/manual.md#installation) for manual install paths.
Rescan plugins in your host after installation.

### Testing

```bash
# Quick offline audio/state regression loop
./scripts/dev-loop.sh fast
./scripts/dev-loop.sh fast --watch

# Release tests, Clippy, validators, benchmark and editor lifecycle
./scripts/dev-loop.sh full --editor

# Empty/loaded screenshots and deterministic audition WAVs
./scripts/screenshot.sh
./scripts/audition.sh
```

The CLAP harness loads the actual cdylib and checks routing, timing, MIDI,
Record once, replacement, reverse/retrigger, saved controls, private RAM and
instance recreation at 44.1/48/96 kHz. These are software regressions, not hardware A/Bs.

Failures stop the pass. Validators belong on PATH or in `target/tools/`. Inspect
both generated screenshots in `screenshots/`; captures alone do not verify
control interaction or audio behavior.

Capture mixing uses SSE2 on x86-64 and NEON on supported AArch64 targets, with
scalar tails/fallback. Stateful SRR and envelopes remain sequential. Allocation
and scalar-equivalence tests protect the real-time path.

Run `cargo run --release --example dsp_bench` for local measurements. Linux
x86-64 and native macOS/ARM have workspace and exported CLAP coverage.

### Offline audio tools

Individual renders use the same DSP and private RAM as the plugin:

```bash
target/release/ultrawave record --input tests/vinyl909.wav --output target/capture.wav
target/release/ultrawave play --input target/capture.wav --output target/crunch.wav \
  --pitch 52 --brr 32 --srr 0.5 --dist 16
target/release/ultrawave play --input target/capture.wav --output target/reverse.wav \
  --start 127 --end 0
target/release/ultrawave play --input target/capture.wav --output target/retrig.wav \
  --retrigger 127 --retrig-time 127 --tempo 120 --output-secs 4
```

WAV tools require 44.1 kHz input. Integer synthesis controls range 0–127; pitch
64 is unity, 52/76 one octave down/up. SRR ranges 0–0.9629. By default, playback
renders the finite retrigger train plus one pitch-adjusted retained take and a
one-second settling tail. Only the first ten seconds of input fit in RAM.
`--output-secs` sets a hard render limit and is required for infinite retriggers
(`--retrigger 127`). Render duration must be in (0, 600] seconds.
Start quietly: drive raises loudness.

For live standalone audio/MIDI device options, use `target/release/ultrawave --help`.
On Linux JACK/PipeWire, connect source audio and MIDI through your patchbay.

### Actual Renoise regression

```bash
cargo xtask bundle ultrawave-plugin --release
./scripts/renoise-test.sh
```

Requires Renoise 3.5.4, Xvfb, xdotool, ImageMagick and Python. Uses a disposable
HOME/display and only the newly built bundle, leaving personal sessions and
installed plugins untouched.

Tests FX-alias notes, main/sidechain routing, Master-track resampling, sample-only
output, Record once, replacement, reverse/retrigger, project save and process
reopen. Artifacts and logs: `target/renoise-test/`.

This exercises VST3; the CLAP harness and clap-validator exercise CLAP.
`tests/test.RPP` remains unverified historical scratch, not a supported setup.

### Native macOS host regression

```bash
cargo xtask bundle ultrawave-plugin --release
./scripts/mac-host-smoke.sh
```

Requires Apple Silicon macOS, REAPER and Python. Refuses to run alongside an
existing REAPER process, uses a private HOME/config and verifies the loaded
VST3 path.

Real MIDI captures and plays audio. A fresh-process project reopen restores
controls but plays silence until recaptured. Editor open/close is checked through
the host API. Artifacts: `target/mac-host-smoke/`.

## Plugin Validation Requirements

**All pull requests must pass plugin validation at strictness level 5+.**

Before submitting a PR, validate your changes locally:

```bash
# Build the plugin
cargo xtask bundle ultrawave-plugin --release

# Validate VST3
./scripts/validate-vst3.sh --plugin target/bundled/ultrawave-plugin.vst3

# Validate CLAP
./scripts/validate-clap.sh --plugin target/bundled/ultrawave-plugin.clap
```

See [docs/testing/plugin-validation.md](docs/testing/plugin-validation.md) for detailed validation instructions.

### CI Validation

The [Buildkite pipeline](.buildkite/pipeline.yml) dispatches the same native
release gate on explicitly selected platforms. Set `ULTRAWAVE_VALIDATE=1` and
`ULTRAWAVE_PLATFORM` as described in the [release workflow](docs/releasing.md).
It uploads candidates/logs, never publishes or installs plugins. The hosted
pipeline must use the checked-in bootstrap with automatic provider triggers
disabled. Run local checks before proposing changes; CI configuration alone
does not prove a DAW works.

### Releases

Use the [manual native release workflow](docs/releasing.md) for Linux x86-64,
Apple Silicon macOS and Windows x86-64 VST3/CLAP builds. `ci/release.py` separates
version bump, source export, native validation/packaging and explicit GitHub
publication. Publication requires all three platforms from the same commit and
an exact remote version tag; ordinary commits never invoke this publisher.
Run `python3 ci/test_release.py` and `python3 ci/test_buildkite.py` when modifying
the release tooling.

## Pull Request Guidelines

1. **Create focused PRs**: One feature or fix per PR
2. **Test locally**: Run `cargo test` and plugin validation
3. **Update docs**: Update documentation if changing behavior
4. **Follow code style**: Match existing patterns and formatting
5. **Write clear commit messages**: Describe what and why

## Code Style

- Follow Rust conventions and idioms
- Run `cargo fmt` before committing
- Address `cargo clippy` warnings
- No `unsafe` without justification and documentation

## Reporting Issues

When reporting bugs, include:
- OS and version
- Steps to reproduce
- Expected vs actual behavior
- Plugin validation output (if applicable)

## Questions?

Open an issue for questions about contributing.
