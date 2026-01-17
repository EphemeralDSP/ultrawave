#!/usr/bin/env bash
# CLAP Plugin Validation Script using clap-validator
# This script automates CLAP plugin validation with configurable test selection
#
# Usage:
#   ./validate-clap.sh [--plugin <path>] [--tests <test-list>] [--verbose]
#
# References:
#   - https://github.com/free-audio/clap-validator
#   - https://cleveraudio.org/developers-getting-started/

set -e

# Default values
PLUGIN_PATH=""
TEST_SELECTION=""
VERBOSE=false
VALIDATOR_PATH=""

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
        --tests)
            TEST_SELECTION="$2"
            shift 2
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --validator-path)
            VALIDATOR_PATH="$2"
            shift 2
            ;;
        --list-tests)
            clap-validator list tests
            exit 0
            ;;
        -h|--help)
            echo "Usage: $0 [--plugin <path>] [--tests <test-list>] [--verbose] [--validator-path <path>]"
            echo ""
            echo "Options:"
            echo "  --plugin <path>           Path to CLAP plugin to validate"
            echo "  --tests <test-list>       Comma-separated list of tests to run (default: all)"
            echo "  --verbose                 Enable verbose output"
            echo "  --validator-path <path>   Path to clap-validator executable"
            echo "  --list-tests              List all available tests and exit"
            echo "  -h, --help                Show this help message"
            echo ""
            echo "Test Categories:"
            echo "  - Fuzzing tests"
            echo "  - Thread safety checks"
            echo "  - State validation"
            echo "  - Preset discovery"
            echo "  - Parameter validation"
            echo ""
            echo "Examples:"
            echo "  $0                                      # Validate with all tests"
            echo "  $0 --plugin path/to/plugin.clap         # Validate specific plugin"
            echo "  $0 --tests state-reproducibility-basic  # Run specific test"
            echo "  $0 --list-tests                         # Show available tests"
            exit 0
            ;;
        *)
            error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Find clap-validator executable
find_validator() {
    if [[ -n "$VALIDATOR_PATH" ]] && [[ -f "$VALIDATOR_PATH" ]]; then
        echo "$VALIDATOR_PATH"
        return 0
    fi

    # Try to find in PATH
    if command -v clap-validator &> /dev/null; then
        local validator_cmd=$(command -v clap-validator)
        info "Found clap-validator in PATH: $validator_cmd"
        echo "$validator_cmd"
        return 0
    fi

    # Check common cargo install location
    if [[ -f "$HOME/.cargo/bin/clap-validator" ]]; then
        info "Found clap-validator in cargo bin: $HOME/.cargo/bin/clap-validator"
        echo "$HOME/.cargo/bin/clap-validator"
        return 0
    fi

    error "clap-validator not found. Please install it from https://github.com/free-audio/clap-validator"
    info "Installation options:"
    info "  1. Build from source: cargo install --git https://github.com/free-audio/clap-validator"
    info "  2. Download from: https://github.com/free-audio/clap-validator/releases"
    info "  3. Or specify path with --validator-path parameter"
    exit 1
}

# Find CLAP plugin to test
find_plugin() {
    if [[ -n "$PLUGIN_PATH" ]] && [[ -e "$PLUGIN_PATH" ]]; then
        echo "$PLUGIN_PATH"
        return 0
    fi

    # Look for built CLAP in target directory
    BUILD_PATHS=(
        "./target/debug/Ultrawave.clap"
        "./target/release/Ultrawave.clap"
        "./target/bundled/Ultrawave.clap"
    )

    for build_path in "${BUILD_PATHS[@]}"; do
        if [[ -e "$build_path" ]]; then
            info "Found CLAP plugin at: $build_path"
            echo "$build_path"
            return 0
        fi
    done

    error "No CLAP plugin found. Please build the plugin first or specify path with --plugin"
    exit 1
}

# Main validation logic
main() {
    info "========================================="
    info "CLAP Plugin Validation"
    info "========================================="
    echo ""

    VALIDATOR=$(find_validator)
    PLUGIN=$(find_plugin)

    info "Validator: $VALIDATOR"
    info "Plugin: $PLUGIN"
    if [[ -n "$TEST_SELECTION" ]]; then
        info "Tests: $TEST_SELECTION"
    else
        info "Tests: All tests (default)"
    fi
    echo ""

    # Build validator arguments
    ARGS=("validate")

    if [[ "$VERBOSE" == true ]]; then
        ARGS+=("--verbose")
    fi

    if [[ -n "$TEST_SELECTION" ]]; then
        ARGS+=("--only" "$TEST_SELECTION")
    fi

    ARGS+=("$PLUGIN")

    info "Running validation..."
    info "Command: $VALIDATOR ${ARGS[*]}"
    echo ""

    # Run clap-validator and capture exit code
    START_TIME=$(date +%s)
    set +e
    "$VALIDATOR" "${ARGS[@]}"
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
        success "Plugin passed CLAP validation"
        echo ""
        info "Test categories validated:"
        info "  ✓ Fuzzing tests"
        info "  ✓ Thread safety checks"
        info "  ✓ State validation"
        info "  ✓ Parameter validation"
        if [[ -z "$TEST_SELECTION" ]]; then
            info "  ✓ All plugin tests"
        fi
        return 0
    else
        error "✗ Validation failed (exit code: $EXIT_CODE)"
        warning "Some tests did not pass"
        echo ""
        info "Next steps:"
        info "  1. Review the output above for specific failures"
        info "  2. Fix the reported issues in your plugin"
        info "  3. Run validation again"
        info "  4. Use --tests to run specific failing tests"
        info "  5. Use --list-tests to see all available tests"
        return $EXIT_CODE
    fi
}

main
exit $?
