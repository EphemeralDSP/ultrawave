# Plugin Validation Testing

This directory contains scripts for automated plugin validation using industry-standard tools.

## Tools

### pluginval - VST3/AU Validator

**pluginval** is the industry-standard plugin validator created by Tracktion. It validates VST2, VST3, and AU plugins for compatibility with major DAWs.

- **Repository**: https://github.com/Tracktion/pluginval
- **Documentation**: https://github.com/Tracktion/pluginval/blob/develop/docs/Testing%20plugins%20with%20pluginval.md
- **Exit Codes**: 0 = all tests pass, 1 = failures detected

#### Installation

**Windows:**
1. Download from: https://github.com/Tracktion/pluginval/releases
2. Extract `pluginval.exe` to one of:
   - `.\tools\pluginval.exe` (project directory)
   - `%USERPROFILE%\pluginval\pluginval.exe`
   - `%LOCALAPPDATA%\pluginval\pluginval.exe`
   - Add to PATH

**macOS:**
1. Download from: https://github.com/Tracktion/pluginval/releases
2. Extract `pluginval.app` to one of:
   - `./tools/pluginval`
   - `$HOME/pluginval/pluginval.app`
   - `/Applications/pluginval.app`
   - Add to PATH

**Linux:**
1. Download from: https://github.com/Tracktion/pluginval/releases
2. Extract `pluginval` to one of:
   - `./tools/pluginval`
   - `$HOME/pluginval/pluginval`
   - `/usr/local/bin/pluginval`
   - Add to PATH

#### Strictness Levels

pluginval supports strictness levels 1-10:

- **Level 1-4**: Basic validation (lenient)
- **Level 5**: **Recommended minimum** for broad DAW compatibility
- **Level 8+**: Required for "Verified by pluginval" badge
- **Level 10**: Maximum strictness (all tests enabled)

**Target for this project: Level 5+**

### clap-validator - CLAP Validator

**clap-validator** is the official validator for CLAP (CLever Audio Plugin) format, written in Rust with parallel test execution and crash isolation.

- **Repository**: https://github.com/free-audio/clap-validator
- **Documentation**: https://cleveraudio.org/developers-getting-started/
- **Features**: Fuzzing, thread safety, state validation, parameter checks

#### Installation

**All Platforms** (requires Rust/Cargo):
```bash
cargo install clap-validator
```

Or download pre-built binaries from: https://github.com/free-audio/clap-validator/releases

#### Test Categories

- Fuzzing tests (random parameter changes)
- Thread safety checks (concurrent access validation)
- State validation (save/load, presets)
- Preset discovery
- Parameter validation (bounds, steps, formatting)

## Usage

### VST3 Validation

**Windows (PowerShell):**
```powershell
# Validate with default settings (strictness level 5)
.\scripts\validate-vst3.ps1

# Specify plugin path
.\scripts\validate-vst3.ps1 -PluginPath ".\target\release\Ultrawave.vst3"

# Increase strictness
.\scripts\validate-vst3.ps1 -Strictness 8

# Verbose output
.\scripts\validate-vst3.ps1 -Verbose

# Specify pluginval location
.\scripts\validate-vst3.ps1 -PluginvalPath "C:\tools\pluginval.exe"
```

**macOS/Linux (Bash):**
```bash
# Validate with default settings (strictness level 5)
./scripts/validate-vst3.sh

# Specify plugin path
./scripts/validate-vst3.sh --plugin ./target/release/Ultrawave.vst3

# Increase strictness
./scripts/validate-vst3.sh --strictness 8

# Verbose output
./scripts/validate-vst3.sh --verbose

# Specify pluginval location
./scripts/validate-vst3.sh --pluginval-path /usr/local/bin/pluginval
```

### CLAP Validation

**Windows (PowerShell):**
```powershell
# Validate with default settings (all tests)
.\scripts\validate-clap.ps1

# Specify plugin path
.\scripts\validate-clap.ps1 -PluginPath ".\target\release\Ultrawave.clap"

# Run specific tests
.\scripts\validate-clap.ps1 -Tests "state-reproducibility-basic"

# List available tests
.\scripts\validate-clap.ps1 -ListTests

# Verbose output
.\scripts\validate-vst3.ps1 -Detailed
```

**macOS/Linux (Bash):**
```bash
# Validate with default settings (strictness level 5)
./scripts/validate-vst3.sh

# Specify plugin path
./scripts/validate-vst3.sh --plugin ./target/release/Ultrawave.vst3

# Increase strictness
./scripts/validate-vst3.sh --strictness 8

# Verbose output
./scripts/validate-vst3.sh --verbose
```

### CLAP Validation

**Windows (PowerShell):**
```powershell
# Validate with default settings (all tests)
.\scripts\validate-clap.ps1

# Specify plugin path
.\scripts\validate-clap.ps1 -PluginPath ".\target\release\Ultrawave.clap"

# Run specific tests
.\scripts\validate-clap.ps1 -Tests "state-reproducibility-basic"

# List available tests
.\scripts\validate-clap.ps1 -ListTests

# Verbose output
.\scripts\validate-clap.ps1 -Detailed
```

**macOS/Linux (Bash):**
```bash
# Validate with default settings (all tests)
./scripts/validate-clap.sh

# Specify plugin path
./scripts/validate-clap.sh --plugin ./target/release/Ultrawave.clap

# Run specific tests
./scripts/validate-clap.sh --tests state-reproducibility-basic

# List available tests
./scripts/validate-clap.sh --list-tests

# Verbose output
./scripts/validate-clap.sh --verbose
```

**Direct CLI usage:**
```bash
# List available tests
clap-validator list tests

# Validate CLAP plugin (all tests)
clap-validator validate ./target/release/Ultrawave.clap
```

## Expected Output

### Successful Validation

```
[INFO] =========================================
[INFO] VST3 Plugin Validation
[INFO] =========================================

[INFO] Pluginval: ./tools/pluginval.exe
[INFO] Plugin: ./target/release/Ultrawave.vst3
[INFO] Strictness Level: 5

[INFO] Running validation...
[... test output ...]

[INFO] =========================================
[INFO] Validation Results
[INFO] =========================================
[INFO] Duration: 12.5 seconds

[SUCCESS] ✓ All tests passed!
[SUCCESS] Plugin is compatible with strictness level 5
```

Exit code: `0`

### Failed Validation

```
[ERROR] ✗ Validation failed (exit code: 1)
[WARNING] Some tests did not pass at strictness level 5

[INFO] Next steps:
[INFO]   1. Review the output above for specific failures
[INFO]   2. Fix the reported issues in your plugin
[INFO]   3. Run validation again
[INFO]   4. Consider reducing strictness temporarily if needed
```

Exit code: `1` (non-zero indicates failures)

## CI Integration

These scripts are designed for CI integration:

- **Exit code 0**: All tests passed (build succeeds)
- **Exit code 1**: Tests failed (build fails)

See `.github/workflows/plugin-validation.yml` for CI configuration.

## Baseline Results

Initial validation results will be documented here after first successful run:

### VST3 (pluginval)
- **Date**: TBD
- **Strictness Level**: TBD
- **Status**: TBD
- **Notes**: TBD

### CLAP (clap-validator)
- **Date**: TBD
- **Test Suite**: TBD
- **Status**: TBD
- **Notes**: TBD

## Troubleshooting

### "pluginval not found"
- Install pluginval from releases page
- Place in one of the checked locations
- Or specify path with `-PluginvalPath` / `--pluginval-path`

### "No VST3 plugin found"
- Build the plugin first: `cargo build --release`
- Or specify path with `-PluginPath` / `--plugin`

### Validation failures
- Review test output for specific issues
- Common issues:
  - Thread safety violations
  - State save/load problems
  - Parameter range issues
  - Timing/latency reporting
- Fix reported issues and re-run validation
- Can temporarily reduce strictness during development

## References

- pluginval: https://github.com/Tracktion/pluginval
- clap-validator: https://github.com/free-audio/clap-validator
- CLAP Developer Docs: https://cleveraudio.org/developers-getting-started/
- NIH-plug testing: https://github.com/robbert-vdh/nih-plug
