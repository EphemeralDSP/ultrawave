#!/bin/bash
# Automated screenshot capture for LLM feedback.
#
# Linux path:
# - Requires: Xvfb and either ImageMagick (import/magick) or xwd+ffmpeg
#
# macOS path:
# - Requires: screencapture (built-in) and osascript (built-in)
# - Terminal may need Screen Recording + Accessibility permissions
#
# Usage:
#   ./scripts/screenshot.sh                    # Empty and loaded sample
#   ./scripts/screenshot.sh empty
#   ./scripts/screenshot.sh loaded
#   SCREENSHOT_DIR=/tmp ./scripts/screenshot.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SCREENSHOT_DIR="${SCREENSHOT_DIR:-$PROJECT_ROOT/screenshots}"
DISPLAY_NUM="${DISPLAY_NUM:-99}"
SCREEN_RES="${SCREEN_RES:-1280x720x24}"
STARTUP_DELAY="${STARTUP_DELAY:-3}"
PRELOAD_WAV="${PRELOAD_WAV:-$PROJECT_ROOT/tests/vinyl909.wav}"
OS_NAME="$(uname -s)"

PLUGIN_BIN="$PROJECT_ROOT/target/release/ultrawave"

cleanup() {
    echo "Cleaning up..."
    # Kill any plugin processes we started
    [[ -n "${PLUGIN_PID:-}" ]] && kill "$PLUGIN_PID" 2>/dev/null || true
    # Kill Xvfb if we started it
    [[ -n "${XVFB_PID:-}" ]] && kill "$XVFB_PID" 2>/dev/null || true
}
trap cleanup EXIT

check_deps() {
    local missing=()
    case "$OS_NAME" in
        Linux)
            local has_capture_backend=0
            command -v Xvfb >/dev/null || missing+=("Xvfb")
            if command -v import >/dev/null || command -v magick >/dev/null; then
                has_capture_backend=1
            elif command -v xwd >/dev/null && command -v ffmpeg >/dev/null; then
                has_capture_backend=1
            fi
            [[ $has_capture_backend -eq 1 ]] || missing+=("ImageMagick or xwd+ffmpeg")
            ;;
        Darwin)
            command -v screencapture >/dev/null || missing+=("screencapture")
            command -v osascript >/dev/null || missing+=("osascript")
            ;;
        *)
            echo "Unsupported OS: $OS_NAME"
            echo "Supported: Linux, macOS"
            exit 1
            ;;
    esac

    if [[ ${#missing[@]} -gt 0 ]]; then
        echo "Missing dependencies: ${missing[*]}"
        if [[ "$OS_NAME" == "Linux" ]]; then
            echo "Install with: sudo apt install xvfb imagemagick"
        else
            echo "On macOS these tools are built-in; ensure PATH is standard system PATH."
        fi
        exit 1
    fi
}

capture_root_window_linux() {
    local output="$1"
    if command -v import >/dev/null; then
        import -window root "$output"
    elif command -v magick >/dev/null; then
        magick import -window root "$output"
    else
        local xwd_output="${output%.png}.xwd"
        xwd -silent -root -display "$DISPLAY" -out "$xwd_output"
        ffmpeg -y -loglevel error -i "$xwd_output" "$output"
        rm -f "$xwd_output"
    fi
}

crop_image_linux() {
    local input="$1"
    local width="$2"
    local height="$3"
    local output="$4"
    if command -v convert >/dev/null; then
        convert "$input" -crop "${width}x${height}+0+0" "$output"
    else
        ffmpeg -y -loglevel error -i "$input" -vf "crop=${width}:${height}:0:0" "$output"
    fi
}

start_xvfb() {
    if [[ "$OS_NAME" != "Linux" ]]; then
        return
    fi

    local target_display="localhost:$DISPLAY_NUM"
    local server_display=":$DISPLAY_NUM"
    local display_ready=1
    if command -v xdpyinfo >/dev/null; then
        xdpyinfo -display "$target_display" >/dev/null 2>&1 || display_ready=0
    else
        display_ready=0
    fi

    if [[ $display_ready -eq 0 ]]; then
        echo "Starting Xvfb on :$DISPLAY_NUM..."
        Xvfb "$server_display" -screen 0 "$SCREEN_RES" -nolisten unix -listen tcp &
        XVFB_PID=$!
        sleep 1
        if command -v xdpyinfo >/dev/null && ! xdpyinfo -display "$target_display" >/dev/null 2>&1; then
            echo "Failed to start Xvfb on $target_display"
            exit 1
        fi
    fi
    export DISPLAY="$target_display"
}

capture_macos_window() {
    local process_name="$1"
    local output="$2"
    local plugin_width="$3"
    local plugin_height="$4"

    local bounds=""
    if ! bounds="$(osascript 2>/dev/null <<OSA
tell application "System Events"
    if not (exists process "$process_name") then
        return ""
    end if
    tell process "$process_name"
        if (count of windows) is 0 then
            return ""
        end if
        set p to position of window 1
        set s to size of window 1
        return (item 1 of p as text) & "," & (item 2 of p as text) & "," & (item 1 of s as text) & "," & (item 2 of s as text)
    end tell
end tell
OSA
    )"; then
        echo "Warning: could not query window bounds via osascript."
        echo "Hint: grant Accessibility access to your terminal in System Settings."
    fi

    if [[ -n "$bounds" && "$bounds" == *","* ]]; then
        local x y w h
        IFS=',' read -r x y w h <<<"$bounds"
        if [[ -n "${x:-}" && -n "${y:-}" && -n "${w:-}" && -n "${h:-}" ]]; then
            screencapture -x -R"${x},${y},${w},${h}" "$output"
            return 0
        fi
    fi

    # Fallback: capture top-left region of primary display.
    if screencapture -x -R"0,0,${plugin_width},${plugin_height}" "$output" 2>/dev/null; then
        return 0
    fi

    echo "Warning: region capture failed, falling back to full-screen capture."
    if ! screencapture -x "$output" 2>/dev/null; then
        echo "Error: macOS screencapture failed."
        echo "Hint: grant Screen Recording permission to your terminal in System Settings."
        return 1
    fi
}

screenshot_plugin() {
    local name="$1"
    local bin="$2"
    local preload="$3"
    local output="$SCREENSHOT_DIR/${name}.png"
    local process_name
    process_name="$(basename "$bin")"

    # Plugin window dimensions (must match default_state() in editor)
    local plugin_width=920
    local plugin_height=650

    if [[ ! -x "$bin" ]]; then
        echo "Binary not found: $bin"
        echo "Run: cargo build --release"
        return 1
    fi

    echo "Launching $name..."
    ULTRAWAVE_SCREENSHOT_WAV="$preload" \
    "$bin" &
    local pid=$!
    PLUGIN_PID=$pid

    echo "Waiting ${STARTUP_DELAY}s for GUI to initialize..."
    sleep "$STARTUP_DELAY"

    echo "Capturing screenshot..."
    if [[ "$OS_NAME" == "Linux" ]]; then
        local temp_output="$SCREENSHOT_DIR/${name}_temp.png"
        # Capture full virtual display and crop to plugin window size.
        capture_root_window_linux "$temp_output"
        crop_image_linux "$temp_output" "$plugin_width" "$plugin_height" "$output"
        rm -f "$temp_output"
    elif [[ "$OS_NAME" == "Darwin" ]]; then
        capture_macos_window "$process_name" "$output" "$plugin_width" "$plugin_height"
    else
        echo "Unsupported OS: $OS_NAME"
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
        return 1
    fi

    local image_dims=""
    if command -v identify >/dev/null; then
        image_dims="$(identify -format '%w x %h' "$output" 2>/dev/null || true)"
    fi
    echo "Saved: $output (${image_dims:-unknown})"

    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    PLUGIN_PID=""
}

main() {
    local target="${1:-both}"
    
    check_deps
    if [[ ! -f "$PRELOAD_WAV" ]]; then
        echo "Preload WAV not found: $PRELOAD_WAV"
        exit 1
    fi
    mkdir -p "$SCREENSHOT_DIR"
    echo "Detected platform: $OS_NAME"
    start_xvfb
    
    case "$target" in
        empty)
            screenshot_plugin "ultrawave_empty" "$PLUGIN_BIN" ""
            ;;
        loaded)
            screenshot_plugin "ultrawave" "$PLUGIN_BIN" "$PRELOAD_WAV"
            ;;
        both)
            screenshot_plugin "ultrawave_empty" "$PLUGIN_BIN" ""
            screenshot_plugin "ultrawave" "$PLUGIN_BIN" "$PRELOAD_WAV"
            ;;
        *)
            echo "Usage: $0 [empty|loaded|both]"
            exit 1
            ;;
    esac
    
    echo ""
    echo "Screenshots saved to: $SCREENSHOT_DIR"
    ls -la "$SCREENSHOT_DIR"/*.png 2>/dev/null | tail -5
}

main "$@"
