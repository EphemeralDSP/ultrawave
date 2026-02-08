#!/bin/bash
# Automated screenshot capture for LLM feedback
# Requires: Xvfb, ImageMagick (import), and built plugins
#
# Usage:
#   ./scripts/screenshot.sh                    # Both plugins
#   ./scripts/screenshot.sh record             # Record plugin only
#   ./scripts/screenshot.sh play               # Play plugin only
#   SCREENSHOT_DIR=/tmp ./scripts/screenshot.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SCREENSHOT_DIR="${SCREENSHOT_DIR:-$PROJECT_ROOT/screenshots}"
DISPLAY_NUM="${DISPLAY_NUM:-99}"
SCREEN_RES="${SCREEN_RES:-1280x720x24}"
STARTUP_DELAY="${STARTUP_DELAY:-3}"
PRELOAD_WAV="${PRELOAD_WAV:-$PROJECT_ROOT/tests/vinyl909.wav}"
PRELOAD_CHANNEL="${PRELOAD_CHANNEL:-0}"

RECORD_BIN="$PROJECT_ROOT/target/release/ultrawave_record"
PLAY_BIN="$PROJECT_ROOT/target/release/ultrawave_play"

cleanup() {
    echo "Cleaning up..."
    # Kill any plugin processes we started
    [[ -n "${RECORD_PID:-}" ]] && kill "$RECORD_PID" 2>/dev/null || true
    [[ -n "${PLAY_PID:-}" ]] && kill "$PLAY_PID" 2>/dev/null || true
    # Kill Xvfb if we started it
    [[ -n "${XVFB_PID:-}" ]] && kill "$XVFB_PID" 2>/dev/null || true
}
trap cleanup EXIT

check_deps() {
    local missing=()
    local has_capture_backend=0
    command -v Xvfb >/dev/null || missing+=("Xvfb")
    if command -v import >/dev/null || command -v magick >/dev/null; then
        has_capture_backend=1
    elif command -v xwd >/dev/null && command -v ffmpeg >/dev/null; then
        has_capture_backend=1
    fi
    [[ $has_capture_backend -eq 1 ]] || missing+=("ImageMagick or xwd+ffmpeg")

    if [[ ${#missing[@]} -gt 0 ]]; then
        echo "Missing dependencies: ${missing[*]}"
        echo "Install with: sudo apt install xvfb imagemagick"
        exit 1
    fi
}

capture_root_window() {
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

crop_image() {
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

screenshot_plugin() {
    local name="$1"
    local bin="$2"
    local output="$SCREENSHOT_DIR/${name}.png"
    local temp_output="$SCREENSHOT_DIR/${name}_temp.png"

    # Plugin window dimensions (must match default_state() in editor)
    local plugin_width=700
    local plugin_height=400

    if [[ ! -x "$bin" ]]; then
        echo "Binary not found: $bin"
        echo "Run: cargo build --release"
        return 1
    fi

    echo "Launching $name..."
    ULTRAWAVE_SCREENSHOT_WAV="$PRELOAD_WAV" \
    ULTRAWAVE_SCREENSHOT_CHANNEL="$PRELOAD_CHANNEL" \
    "$bin" &
    local pid=$!

    echo "Waiting ${STARTUP_DELAY}s for GUI to initialize..."
    sleep "$STARTUP_DELAY"

    echo "Capturing screenshot..."
    # Capture full screen first
    capture_root_window "$temp_output"

    # Crop to plugin window size (top-left corner where window manager places it)
    # Most WMs place new windows at 0,0 or with small offset
    crop_image "$temp_output" "$plugin_width" "$plugin_height" "$output"
    rm -f "$temp_output"

    local image_dims=""
    if command -v identify >/dev/null; then
        image_dims="$(identify -format '%w x %h' "$output" 2>/dev/null || true)"
    fi
    echo "Saved: $output (${image_dims:-unknown})"

    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
}

main() {
    local target="${1:-both}"
    
    check_deps
    if [[ ! -f "$PRELOAD_WAV" ]]; then
        echo "Preload WAV not found: $PRELOAD_WAV"
        exit 1
    fi
    mkdir -p "$SCREENSHOT_DIR"
    start_xvfb
    
    case "$target" in
        record)
            screenshot_plugin "ultrawave_record" "$RECORD_BIN"
            ;;
        play)
            screenshot_plugin "ultrawave_play" "$PLAY_BIN"
            ;;
        both)
            screenshot_plugin "ultrawave_record" "$RECORD_BIN"
            screenshot_plugin "ultrawave_play" "$PLAY_BIN"
            ;;
        *)
            echo "Usage: $0 [record|play|both]"
            exit 1
            ;;
    esac
    
    echo ""
    echo "Screenshots saved to: $SCREENSHOT_DIR"
    ls -la "$SCREENSHOT_DIR"/*.png 2>/dev/null | tail -5
}

main "$@"
