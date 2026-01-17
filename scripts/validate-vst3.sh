#!/usr/bin/env bash
# VST3 Plugin Validation Script using pluginval
# This script automates VST3 plugin validation with configurable strictness levels
#
# Usage:
#   ./validate-vst3.sh [--plugin <path>] [--strictness <1-10>] [--verbose]
#
# References:
#   - https://github.com/Tracktion/pluginval
#   - https://github.com/Tracktion/pluginval/blob/develop/docs/Testing%20plugins%20with%20pluginval.md

set -e

# Default values
PLUGIN_PATH=""
STRICTNESS=5
VERBOSE=false
PLUGINVAL_PATH=""

# ANSI color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --plugin)
            PLUGIN_PATH="$2"
            shift 2
            ;;
        --strictness)
            STRICTNESS="$2"
            shift 2
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --pluginval-path)
            PLUGINVAL_PATH="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [--plugin <path>] [--strictness <1-10>] [--verbose] [--pluginval-path <path>]"
            echo ""
            echo "Options:"
            echo "  --plugin <path>          Path to VST3 plugin to validate"
            echo "  --strictness <1-10>      Validation strictness level (default: 5)"
            echo "  --verbose                Enable verbose output"
            echo "  --pluginval-path <path>  Path to pluginval executable"
            echo "  -h, --help               Show this help message"
            exit 0
            ;;
        *)
            error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Validate strictness level
if [[ $STRICTNESS -lt 1 ]] || [[ $STRICTNESS -gt 10 ]]; then
    error "Strictness must be between 1 and 10"
    exit 1
fi

# Find pluginval executable
find_pluginval() {
    if [[ -n "$PLUGINVAL_PATH" ]] && [[ -f "$PLUGINVAL_PATH" ]]; then
        echo "$PLUGINVAL_PATH"
        return 0
    fi

    # Check common locations based on OS
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        POSSIBLE_PATHS=(
            "./tools/pluginval"
            "$HOME/pluginval/pluginval.app/Contents/MacOS/pluginval"
            "/Applications/pluginval.app/Contents/MacOS/pluginval"
            "/usr/local/bin/pluginval"
        )
    else
        # Linux
        POSSIBLE_PATHS=(
            "./tools/pluginval"
            "$HOME/pluginval/pluginval"
            "/usr/local/bin/pluginval"
            "/usr/bin/pluginval"
        )
    fi

    for path in "${POSSIBLE_PATHS[@]}"; do
        if [[ -f "$path" ]]; then
            info "Found pluginval at: $path"
            echo "$path"
            return 0
        fi
    done

    # Try to find in PATH
    if command -v pluginval &> /dev/null; then
        local pluginval_cmd=$(command -v pluginval)
        info "Found pluginval in PATH: $pluginval_cmd"
        echo "$pluginval_cmd"
        return 0
    fi

    error "pluginval not found. Please install it from https://github.com/Tracktion/pluginval"
    info "Installation options:"
    info "  1. Download from: https://github.com/Tracktion/pluginval/releases"
    info "  2. Place in ./tools/pluginval"
    info "  3. Or specify path with --pluginval-path parameter"
    exit 1
}

# Find VST3 plugin to test
find_plugin() {
    if [[ -n "$PLUGIN_PATH" ]] && [[ -e "$PLUGIN_PATH" ]]; then
        echo "$PLUGIN_PATH"
        return 0
    fi

    # Look for built VST3 in target directory
    BUILD_PATHS=(
        "./target/debug/Ultrawave.vst3"
        "./target/release/Ultrawave.vst3"
        "./target/bundled/Ultrawave.vst3"
    )

    for build_path in "${BUILD_PATHS[@]}"; do
        if [[ -e "$build_path" ]]; then
            info "Found VST3 plugin at: $build_path"
            echo "$build_path"
            return 0
        fi
    done

    error "No VST3 plugin found. Please build the plugin first or specify path with --plugin"
    exit 1
}

# Main validation logic
main() {
    info "========================================="
    info "VST3 Plugin Validation"
    info "========================================="
    echo ""

    PLUGINVAL=$(find_pluginval)
    PLUGIN=$(find_plugin)

    info "Pluginval: $PLUGINVAL"
    info "Plugin: $PLUGIN"
    info "Strictness Level: $STRICTNESS (1=lenient, 10=strict)"
    echo ""

    # Build pluginval arguments
    ARGS=(
        "--validate-in-process"
        "--strictness-level" "$STRICTNESS"
    )

    if [[ "$VERBOSE" == true ]]; then
        ARGS+=("--verbose")
    fi

    ARGS+=("$PLUGIN")

    info "Running validation..."
    info "Command: $PLUGINVAL ${ARGS[*]}"
    echo ""

    # Run pluginval and capture exit code
    START_TIME=$(date +%s)
    set +e
    "$PLUGINVAL" "${ARGS[@]}"
    EXIT_CODE=$?
    set -e
    END_TIME=$(date +%s)
    DURATION=$((END_TIME - START_TIME))

    echo ""
    info "========================================="
    info "Validation Results"
    info "========================================="
    info "Duration: ${DURATION} seconds"
    echo ""

    if [[ $EXIT_CODE -eq 0 ]]; then
        success "✓ All tests passed!"
        success "Plugin is compatible with strictness level $STRICTNESS"
        echo ""
        info "Strictness level meanings:"
        info "  5+ : Recommended for broad DAW compatibility"
        info "  8+ : Suitable for 'Verified by pluginval' badge"
        info "  10 : Maximum strictness (all tests)"
        return 0
    else
        error "✗ Validation failed (exit code: $EXIT_CODE)"
        warning "Some tests did not pass at strictness level $STRICTNESS"
        echo ""
        info "Next steps:"
        info "  1. Review the output above for specific failures"
        info "  2. Fix the reported issues in your plugin"
        info "  3. Run validation again"
        info "  4. Consider reducing strictness temporarily if needed"
        return $EXIT_CODE
    fi
}

main
exit $?
