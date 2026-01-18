# Validation Troubleshooting Guide

Common plugin validation failures and how to fix them.

## Tool Setup Issues

### "pluginval not found"

**Cause**: pluginval executable not in expected location.

**Fix**:
1. Download from [pluginval releases](https://github.com/Tracktion/pluginval/releases)
2. Place in one of these locations:
   - `./tools/pluginval` (project-local)
   - `~/.local/bin/pluginval` or `$HOME/pluginval/` (user)
   - `/usr/local/bin/pluginval` (system)
3. Or specify path: `.\scripts\validate-vst3.ps1 -PluginvalPath "C:\path\to\pluginval.exe"`

### "clap-validator not found"

**Cause**: clap-validator not installed.

**Fix**:
```bash
cargo install clap-validator
```

Or download from [clap-validator releases](https://github.com/free-audio/clap-validator/releases).

### "No VST3/CLAP plugin found"

**Cause**: Plugin not built yet.

**Fix**:
```bash
cargo xtask bundle ultrawave --release
```

Then retry validation.

## Common VST3 Failures (pluginval)

### Thread Safety Violations

**Symptom**: "Deadlock detected" or "Thread safety violation"

**Common Causes**:
- Locking mutexes in audio thread
- Calling non-realtime-safe functions from process block
- UI updates blocking audio

**Fix**:
- Use lock-free data structures for audio thread communication
- Move allocations and locks to parameter change callbacks
- Use message queues between UI and audio threads

### State Save/Load Failures

**Symptom**: "State recall failed" or "Parameters differ after state load"

**Common Causes**:
- Not serializing all parameters
- Floating-point comparison issues
- Version mismatch in state format

**Fix**:
- Ensure all parameters are included in state serialization
- Use tolerance when comparing floats
- Implement versioned state loading

### Parameter Range Issues

**Symptom**: "Parameter out of range" or "Invalid parameter value"

**Common Causes**:
- Parameter min/max not matching actual range
- Normalized values not clamped to 0-1
- Default values outside valid range

**Fix**:
- Verify all parameter ranges in plugin definition
- Clamp normalized values before use
- Set defaults within valid ranges

### Latency Reporting

**Symptom**: "Latency mismatch" or "Reported latency differs from actual"

**Common Causes**:
- Not updating latency when changing buffer size
- Latency changes mid-stream without notification
- Incorrect latency calculation

**Fix**:
- Update latency in prepare/reset callbacks
- Notify host when latency changes
- Verify latency calculation against actual delay

### Processing Stability

**Symptom**: "Output contains NaN" or "Output contains Inf"

**Common Causes**:
- Division by zero in DSP code
- Uninitialized filter states
- Feedback loops without limiting

**Fix**:
- Add guards for division operations
- Initialize all DSP state on reset
- Add soft clipping or limiting in feedback paths

## Common CLAP Failures (clap-validator)

### Fuzzing Test Failures

**Symptom**: "Fuzzing test failed" with random parameter values

**Common Causes**:
- Edge cases not handled at parameter extremes
- Interactions between parameters causing issues
- Rapid parameter changes causing glitches

**Fix**:
- Test parameters at 0%, 50%, 100% manually
- Add parameter smoothing for rapid changes
- Handle edge cases explicitly

### State Reproducibility

**Symptom**: "State not reproducible" or "Output differs after state reload"

**Common Causes**:
- Random number generators not seeded consistently
- Time-dependent state not saved
- LFO phases not preserved

**Fix**:
- Save/restore RNG state
- Include all time-dependent state in serialization
- Preserve oscillator/LFO phases in state

### Preset Discovery Issues

**Symptom**: "Preset discovery failed" or "Invalid preset format"

**Common Causes**:
- Malformed preset metadata
- Missing preset files
- Invalid preset paths

**Fix**:
- Verify preset metadata format
- Check preset file locations
- Validate preset paths are accessible

## Debugging Strategies

### Reduce Strictness First

When facing many failures, start at a lower strictness level:

```powershell
.\scripts\validate-vst3.ps1 -Strictness 1
```

Fix failures at each level before increasing.

### Run Specific Tests

For CLAP, isolate specific test categories:

```powershell
.\scripts\validate-clap.ps1 -Tests "param-fuzzing"
```

### Enable Verbose Output

Get detailed logs for debugging:

```powershell
.\scripts\validate-vst3.ps1 -Detailed
.\scripts\validate-clap.ps1 -Detailed
```

### Check CI Logs

CI validation logs are uploaded as artifacts. Download and search for specific failure patterns.

## Platform-Specific Issues

### Windows

- **Long paths**: Ensure plugin paths don't exceed Windows path limits
- **Code signing**: Unsigned plugins may fail on some security configurations
- **Visual C++ Runtime**: Ensure correct runtime is installed/bundled

### macOS

- **Gatekeeper**: Plugin may need to be notarized for distribution
- **AU validation**: macOS-specific; use `auval` for additional AU testing
- **Hardened runtime**: May need entitlements for certain operations

### Linux

- **Dependencies**: Ensure all .so dependencies are available
- **X11/Wayland**: GUI tests may fail without display server
- **Permissions**: Plugin file must be executable

## Getting Help

1. Check the detailed validation output for specific error messages
2. Search [pluginval issues](https://github.com/Tracktion/pluginval/issues)
3. Search [clap-validator issues](https://github.com/free-audio/clap-validator/issues)
4. Check [NIH-plug discussions](https://github.com/robbert-vdh/nih-plug/discussions)
5. File an issue in this repository with:
   - Full validation output
   - Platform and OS version
   - Plugin version
   - Steps to reproduce
