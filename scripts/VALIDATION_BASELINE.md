# Validation Baseline Results

This document tracks baseline validation results for the Ultrawave plugin across different formats and platforms.

## Status

**Current Status**: ⏳ Pending first build and validation run

The validation scripts are in place and ready to use. Baseline results will be documented here once:
1. The plugin is built in release mode
2. Initial validation is run on all supported formats
3. Any critical issues are resolved

## How to Establish Baseline

### Step 1: Build the plugin
```bash
# Build release version
cargo build --release

# Or build all formats
cargo xtask bundle ultrawave --release
```

### Step 2: Run validators

**VST3 (Windows):**
```powershell
.\scripts\validate-vst3.ps1 -Strictness 5
```

**VST3 (macOS/Linux):**
```bash
./scripts/validate-vst3.sh --strictness 5
```

**CLAP (Windows):**
```powershell
.\scripts\validate-clap.ps1
```

**CLAP (macOS/Linux):**
```bash
./scripts/validate-clap.sh
```

**CLAP (Direct CLI):**
```bash
clap-validator validate ./target/bundled/Ultrawave.clap
```

### Step 3: Document results below

## VST3 Validation (pluginval)

### Windows

- **Date**: TBD
- **Plugin Version**: TBD
- **Strictness Level**: 5
- **Status**: ⏳ Pending
- **Exit Code**: TBD
- **Duration**: TBD
- **Notes**: TBD

### macOS

- **Date**: TBD
- **Plugin Version**: TBD
- **Strictness Level**: 5
- **Status**: ⏳ Pending
- **Exit Code**: TBD
- **Duration**: TBD
- **Formats Tested**: VST3, AU
- **Notes**: TBD

### Linux

- **Date**: TBD
- **Plugin Version**: TBD
- **Strictness Level**: 5
- **Status**: ⏳ Pending
- **Exit Code**: TBD
- **Duration**: TBD
- **Notes**: TBD

## CLAP Validation (clap-validator)

### All Platforms

- **Date**: TBD
- **Plugin Version**: TBD
- **Status**: ⏳ Pending
- **Exit Code**: TBD
- **Duration**: TBD
- **Test Categories**:
  - [ ] Fuzzing
  - [ ] Thread Safety
  - [ ] State Validation
  - [ ] Preset Discovery
  - [ ] Parameter Validation
- **Notes**: TBD

## Known Issues

Issues discovered during validation will be tracked here and linked to beads issues:

- None yet (validation pending)

## Validation History

### Future Template

```
#### YYYY-MM-DD - Version X.Y.Z

**Changes**: Brief description of changes since last validation

**VST3 Results**:
- Windows: ✅ Pass / ❌ Fail (details)
- macOS: ✅ Pass / ❌ Fail (details)
- Linux: ✅ Pass / ❌ Fail (details)

**CLAP Results**:
- All Platforms: ✅ Pass / ❌ Fail (details)

**Issues Found**: Link to beads issues if any
**Resolution**: How issues were resolved
```

## Release Checklist

Before releasing, ensure the following validation gates pass:

- [ ] All validators pass at strictness level 5+
- [ ] Validation reports generated and archived
- [ ] No regression from previous release
- [ ] All platforms tested (Windows, macOS, Linux)
- [ ] All formats validated (VST3, CLAP, AU on macOS)

### Automated Release Process

Releases are automated via GitHub Actions. To create a release:

1. Tag the commit: `git tag v1.0.0 && git push --tags`
2. The release workflow runs validation on all platforms
3. **Validation failures block the release**
4. On success, artifacts are uploaded with validation reports
5. Release includes "Verified by pluginval" badges

### Manual Pre-Release Validation

```bash
# Build release
cargo build --release
cargo xtask bundle ultrawave --release

# Run validators locally
.\scripts\validate-vst3.ps1 -Strictness 5   # Windows
.\scripts\validate-clap.ps1                  # Windows

./scripts/validate-vst3.sh --strictness 5    # macOS/Linux
./scripts/validate-clap.sh                   # macOS/Linux
```

## Next Steps

1. Complete core DSP implementation
2. Build release version of plugin
3. Run initial validation with scripts
4. Document baseline results above
5. Address any critical failures
6. Re-validate until all tests pass at strictness level 5+
7. ~~Set up CI automation (ultrawave-45r)~~ ✅ Complete
8. ~~Integrate validators into release pipeline (ultrawave-cz4)~~ ✅ Complete
