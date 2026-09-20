#!/usr/bin/env bash
set -euo pipefail

STRICTNESS="${VALIDATION_STRICTNESS:-5}"
export PATH="${HOME}/.local/bin:${PATH}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

require_file() {
    local path="$1"
    if [[ ! -e "$path" ]]; then
        echo "Missing expected plugin artifact: $path"
        exit 1
    fi
}

require_file "$ROOT/target/bundled/ultrawave-plugin.vst3"
require_file "$ROOT/target/bundled/ultrawave-plugin.clap"

for plugin in ultrawave-plugin; do
    "$ROOT/scripts/validate-vst3.sh" --plugin "$ROOT/target/bundled/$plugin.vst3" --strictness "$STRICTNESS"
    "$ROOT/scripts/validate-clap.sh" --plugin "$ROOT/target/bundled/$plugin.clap"
done
