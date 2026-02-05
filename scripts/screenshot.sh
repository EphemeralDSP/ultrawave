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
    command -v Xvfb >/dev/null || missing+=("Xvfb")
    command -v import >/dev/null || missing+=("ImageMagick")
    
    if [[ ${#missing[@]} -gt 0 ]]; then
        echo "Missing dependencies: ${missing[*]}"
        echo "Install with: sudo apt install xvfb imagemagick"
        exit 1
    fi
}

start_xvfb() {
    if [[ -z "${DISPLAY:-}" ]] || [[ "$DISPLAY" == ":$DISPLAY_NUM" ]]; then
        echo "Starting Xvfb on :$DISPLAY_NUM..."
        Xvfb ":$DISPLAY_NUM" -screen 0 "$SCREEN_RES" &
        XVFB_PID=$!
        sleep 1
    fi
    export DISPLAY=":$DISPLAY_NUM"
}

screenshot_plugin() {
    local name="$1"
    local bin="$2"
    local output="$SCREENSHOT_DIR/${name}_$(date +%Y%m%d_%H%M%S).png"
    
    if [[ ! -x "$bin" ]]; then
        echo "Binary not found: $bin"
        echo "Run: cargo build --release"
        return 1
    fi
    
    echo "Launching $name..."
    "$bin" &
    local pid=$!
    
    echo "Waiting ${STARTUP_DELAY}s for GUI to initialize..."
    sleep "$STARTUP_DELAY"
    
    echo "Capturing screenshot..."
    import -window root "$output"
    
    echo "Saved: $output"
    
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
}

main() {
    local target="${1:-both}"
    
    check_deps
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
