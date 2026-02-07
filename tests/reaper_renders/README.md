# Reaper Render Regression Tests

This directory holds Reaper project files (`.rpp`) and their expected rendered
output (`.wav`) for automated regression testing.

## How it works

`cargo xtask render-test` will:

1. Find every `.rpp` file in this directory
2. Render each project headlessly via `reaper -renderproject`
3. Compare the rendered WAV output against the matching reference WAV
4. Report pass/fail with RMS and peak difference metrics

## Adding a test case

1. Create your Reaper project using the Ultrawave plugins
2. Configure the render settings inside Reaper (format, sample rate, etc.)
3. Set `RENDER_FILE` in the project so the output lands in a known location
4. Render the project once to produce the reference output
5. Place both files here with matching names:

```
my_test_case.rpp          # Reaper project
my_test_case.wav          # Reference rendered output
```

The test runner pairs files by stem name: `foo.rpp` expects `foo.wav`.

## Render output resolution

The test runner reads `RENDER_FILE` and `RENDER_PATTERN` from each `.rpp` to
determine where Reaper will write its output. Common setups:

- **Relative path**: resolved relative to the `.rpp` file's directory
- **Empty RENDER_FILE**: output goes to the project's directory
- **RENDER_PATTERN with `$project`**: substituted with the project filename stem

## Running

```bash
# Default: tests in this directory, auto-detect Reaper
cargo xtask render-test

# Custom directory and thresholds
cargo xtask render-test --dir path/to/tests --threshold-rms 0.005 --verbose

# Specify Reaper path explicitly
cargo xtask render-test --reaper /usr/local/bin/reaper
```

## Options

| Flag                    | Default                | Description                            |
|-------------------------|------------------------|----------------------------------------|
| `--dir <path>`          | `tests/reaper_renders` | Directory with .rpp/.wav pairs         |
| `--reaper <path>`       | auto-detected          | Path to Reaper executable              |
| `--threshold-rms <f>`   | `0.001`                | Max RMS difference (0.0–1.0)           |
| `--threshold-peak <f>`  | `0.01`                 | Max peak sample difference (0.0–1.0)   |
| `--verbose`             | off                    | Show detailed per-file comparison info |

## Thresholds

The default thresholds are strict enough to catch audible regressions while
allowing for minor floating-point variance across platforms:

- **RMS 0.001** (~-60 dB) catches any meaningful change in overall level
- **Peak 0.01** (~-40 dB) catches individual sample spikes

Loosen these if your tests involve non-deterministic elements (e.g., noise
generators, different plugin versions across machines).
