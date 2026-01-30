# Ultrawave Play - Error Tracking

## Critical Errors

### 1. Audio Buffer Size Mismatch (CRITICAL) - RESOLVED
**Error:** `thread 'cpal_wasapi_out' panicked at 'Received 1126 samples, while the configured buffer size is 512'`
- **Location:** `nih_plug::wrapper::standalone::backend::cpal:832`
- **Impact:** Audio output thread crashes, causing application failure
- **Details:** The WASAPI backend receives 1126 samples but expects 512. This is a buffer size configuration mismatch in the nih_plug standalone wrapper.
- **Root Cause:** nih_plug uses a fixed buffer size (default 512) but WASAPI may provide a different buffer size (e.g., 1126) depending on the audio device and driver.
- **Workaround:** Run the application with a larger period size that matches or exceeds what WASAPI provides:
  ```bash
  ultrawave_play --period-size 1126
  # or
  ultrawave_play --period-size 2048
  ```
- **Priority:** HIGH
- **Status:** **Complete** (Workaround documented - upstream issue in nih_plug)

## Warnings

### 2. Vizia CSS Style Warnings
Multiple CSS custom properties not recognized by vizia_core::style:
- Custom Property: `background`
- Custom Property: `padding`
- Custom Property: `align-items`
- Custom Property: `text-transform`
- Custom Property: `letter-spacing`
- Custom Property: `text-shadow`
- Custom Property: `margin-top`
- Unparsed: `font-size`
- Unparsed: `box-shadow`

**Impact:** Log clutter, potential UI styling issues
**Priority:** LOW
**Status:** Open

### 3. Jack Audio Server Connection
**Errors:**
- `Cannot connect to named pipe after wait = \\.\pipe\server_jack_default_0 err = 2`
- `Cannot connect to server request channel`
- `jack server is not running or cannot be started`

**Impact:** App correctly falls back to WASAPI, but error messages are noisy
**Priority:** LOW
**Status:** Open (enhancement - handle more gracefully)

## Log Summary

**Timestamp:** 2026-01-29 20:42:37
**Application:** ultrawave_play.exe
**Backend:** WASAPI (after Jack fallback)
**UI Framework:** Vizia

### Key Events:
1. 20:42:37 - Jack server connection attempted and failed
2. 20:42:37 - Fallback to WASAPI backend
3. 20:42:37 - Multiple Vizia CSS warnings during UI initialization
4. 20:42:38 - **CRITICAL PANIC** - Buffer size mismatch in audio thread
5. 20:42:42+ - UI hover events continue (GUI thread unaffected)

## Next Steps

1. **Immediate:** Fix buffer size configuration to match WASAPI expectations
2. **Short-term:** Review and fix CSS custom properties in Vizia styles
3. **Long-term:** Improve Jack error handling to reduce log noise
