# Plugin validation

Ultrawave exports one VST3/CLAP audio effect. Validate the built bundle, not an
installed copy that may be stale. Native targets are Linux x86-64, Apple Silicon
macOS and Windows x86-64/MSVC; there is no AU export. The shell helpers below
target Linux/macOS; the [native release command](../releasing.md) also supports
Windows without Bash.

## Fast iteration and full checks

```bash
./scripts/dev-loop.sh fast
./scripts/dev-loop.sh fast --watch
./scripts/dev-loop.sh full --editor
```

The fast loop checks formatting, workspace tests, exported CLAP audio/state,
VST3 wrapper state regressions and six deterministic WAV renders. The full loop
also runs Clippy, release tests, bundles, validators, editor lifecycle stress
and a DSP benchmark. Every failing command stops that pass.

The exported CLAP harness exercises Master dry-through and post-playback resampling,
staged replacement boundaries, sample-only output, main/sidechain capture,
MIDI timing/assignment/velocity, automation, Record once/re-arm, reverse/retrigger,
single-sample replacement, private RAM and state recreation at 44.1/48/96 kHz.
Validators complement these audible-output assertions; neither replaces a real
DAW test or hardware A/B.

## Validators

Install [pluginval](https://github.com/Tracktion/pluginval/releases) and
[clap-validator](https://github.com/free-audio/clap-validator/releases).
Put them on PATH or use the explicit executable options below. The full loop
also adds `target/tools/` to PATH. Linux VST3 validation requires `xvfb-run`.

```bash
cargo xtask bundle ultrawave-plugin --release
./scripts/validate-vst3.sh --plugin target/bundled/ultrawave-plugin.vst3 \
  --pluginval-path target/tools/pluginval --strictness 5
./scripts/validate-clap.sh --plugin target/bundled/ultrawave-plugin.clap \
  --validator-path target/tools/clap-validator

# Optional focused CLAP check
./scripts/validate-clap.sh --list-tests
./scripts/validate-clap.sh --plugin target/bundled/ultrawave-plugin.clap \
  --tests state-reproducibility-basic
```

Use pluginval strictness 5 minimum for changes and 10 for releases. Both scripts
retain logs under `target/validation/`, propagate failures and reject caught
panic/window-error signatures even when a validator exits successfully. Missing
tools or plugin paths are errors, not skipped successes. Run `--help` for options.

## Editor and real host

```bash
cargo build --release
./scripts/screenshot.sh
./scripts/test-editor-lifecycle.sh
./scripts/renoise-test.sh
```

Inspect empty/loaded screenshots and changed interactive states, including MIDI
dropdown selection/scrolling, capture details and protected/re-arm feedback.
A screenshot file alone is not verification. The Renoise fixture uses a disposable HOME/display
and the unified bundle to test actual FX-alias notes, audio routing and project
save/reopen. See the [README](../../README.md#actual-renoise-regression) for
requirements, safety and artifact paths. Do not overwrite plugins loaded by a
personal DAW session.

## CI and release checks

The checked-in [Buildkite pipeline](../../.buildkite/pipeline.yml) enables
native validation only with `ULTRAWAVE_VALIDATE=1` and an explicit
`ULTRAWAVE_PLATFORM` selection. Its hosted bootstrap loads repository YAML;
GitHub provider triggers must be disabled. It uploads candidates and logs,
never publishes releases. The [manual native release workflow](../releasing.md)
describes both Buildkite dispatch and direct native commands. Publication still
requires all three platforms, matching receipts/hashes and the exact remote tag.

Before releasing, verify both formats on each target platform, inspect editor
states, run the exported audio harness and an actual host routing/state test,
and report any untested platform or host explicitly. Hardware character and DAC
response require separate Machinedrum measurements.
