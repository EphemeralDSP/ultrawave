# Plugin Validation Guide

Ultrawave uses industry-standard plugin validators to ensure compatibility across all major DAWs. This guide covers running validators locally, interpreting results, and integrating with CI.

## Quick Start

```bash
# Build the plugin
cargo xtask bundle ultrawave --release

# Validate VST3 (Windows)
.\scripts\validate-vst3.ps1

# Validate VST3 (macOS/Linux)
./scripts/validate-vst3.sh

# Validate CLAP (Windows)
.\scripts\validate-clap.ps1

# Validate CLAP (macOS/Linux)
./scripts/validate-clap.sh
```

## Validation Tools

### pluginval (VST3/AU)

[pluginval](https://github.com/Tracktion/pluginval) is the industry-standard validator created by Tracktion. It tests VST2, VST3, and AU plugins for host compatibility.

#### Installation

| Platform | Installation |
|----------|--------------|
| Windows | Download from [releases](https://github.com/Tracktion/pluginval/releases), extract to `.\tools\pluginval.exe` |
| macOS | Download from [releases](https://github.com/Tracktion/pluginval/releases), extract to `./tools/pluginval.app` |
| Linux | Download from [releases](https://github.com/Tracktion/pluginval/releases), extract to `./tools/pluginval` |

The scripts also check common system locations (`~/.local/bin`, `/usr/local/bin`, PATH).

#### Strictness Levels

| Level | Description | Use Case |
|-------|-------------|----------|
| 1-4 | Basic validation | Development debugging |
| **5** | **Recommended minimum** | **PR requirements** |
| 8+ | Strict validation | "Verified by pluginval" badge |
| 10 | Maximum strictness | Release validation |

### clap-validator (CLAP)

[clap-validator](https://github.com/free-audio/clap-validator) is the official CLAP plugin validator, written in Rust with parallel test execution and crash isolation.

#### Installation

```bash
# Via cargo (recommended)
cargo install clap-validator

# Or download from releases
https://github.com/free-audio/clap-validator/releases
```

#### Test Categories

- **Fuzzing tests**: Random parameter changes
- **Thread safety**: Concurrent access validation
- **State validation**: Save/load, presets
- **Preset discovery**: Factory preset enumeration
- **Parameter validation**: Bounds, steps, formatting

## Running Validators

### VST3 Validation

**Windows (PowerShell):**
```powershell
# Default (strictness 5)
.\scripts\validate-vst3.ps1

# Custom strictness
.\scripts\validate-vst3.ps1 -Strictness 8

# Verbose output
.\scripts\validate-vst3.ps1 -Detailed

# Specify plugin path
.\scripts\validate-vst3.ps1 -PluginPath ".\target\release\Ultrawave.vst3"
```

**macOS/Linux (Bash):**
```bash
# Default (strictness 5)
./scripts/validate-vst3.sh

# Custom strictness
./scripts/validate-vst3.sh --strictness 8

# Verbose output
./scripts/validate-vst3.sh --verbose

# Specify plugin path
./scripts/validate-vst3.sh --plugin ./target/release/Ultrawave.vst3
```

### CLAP Validation

**Windows (PowerShell):**
```powershell
# All tests
.\scripts\validate-clap.ps1

# List available tests
.\scripts\validate-clap.ps1 -ListTests

# Run specific tests
.\scripts\validate-clap.ps1 -Tests "state-reproducibility-basic"

# Verbose output
.\scripts\validate-clap.ps1 -Detailed
```

**macOS/Linux (Bash):**
```bash
# All tests
./scripts/validate-clap.sh

# List available tests
./scripts/validate-clap.sh --list-tests

# Run specific tests
./scripts/validate-clap.sh --tests state-reproducibility-basic

# Verbose output
./scripts/validate-clap.sh --verbose
```

## Interpreting Results

### Success Output

```
[INFO] =========================================
[INFO] VST3 Plugin Validation
[INFO] =========================================

[INFO] Pluginval: ./tools/pluginval.exe
[INFO] Plugin: ./target/release/Ultrawave.vst3
[INFO] Strictness Level: 5

[INFO] Running validation...
[... test output ...]

[SUCCESS] ✓ All tests passed!
[SUCCESS] Plugin is compatible with strictness level 5
```

**Exit code: `0`** - Plugin passed all tests.

### Failure Output

```
[ERROR] ✗ Validation failed (exit code: 1)
[WARNING] Some tests did not pass at strictness level 5
```

**Exit code: `1`** - Some tests failed. Review the detailed output above the summary for specific failures.

### Common Validation Output Patterns

| Pattern | Meaning |
|---------|---------|
| `✓ PASSED` | Test passed |
| `✗ FAILED` | Test failed - needs investigation |
| `⚠ WARNING` | Non-fatal issue - consider fixing |
| `SKIPPED` | Test not applicable to this plugin |

## CI Integration

Plugin validation runs automatically on every push and pull request via GitHub Actions. See `.github/workflows/plugin-validation.yml`.

### CI Matrix

| Platform | Formats Tested |
|----------|---------------|
| Windows | VST3, CLAP |
| macOS | VST3, CLAP, AU |
| Linux | VST3, CLAP |

### CI Behavior

- **Exit code 0**: All tests pass → PR can merge
- **Exit code 1**: Tests failed → PR blocked

### Validation Artifacts

CI uploads validation logs and built plugins as artifacts:
- `validation-logs-{os}`: Full validation output
- `ultrawave-validated-{os}`: Validated plugin binaries

## Release Validation Checklist

Before releasing a new version:

- [ ] Build release version: `cargo xtask bundle ultrawave --release`
- [ ] VST3 passes at strictness level 5+ on Windows
- [ ] VST3 passes at strictness level 5+ on macOS
- [ ] VST3 passes at strictness level 5+ on Linux
- [ ] AU passes at strictness level 5+ on macOS
- [ ] CLAP passes all tests on all platforms
- [ ] All CI validation checks pass
- [ ] Update `scripts/VALIDATION_BASELINE.md` with results

## See Also

- [Troubleshooting Guide](troubleshooting.md) - Common validation failures and fixes
- [scripts/TESTING.md](../../scripts/TESTING.md) - Low-level script documentation
- [scripts/VALIDATION_BASELINE.md](../../scripts/VALIDATION_BASELINE.md) - Baseline results
