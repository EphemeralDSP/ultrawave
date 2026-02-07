#!/usr/bin/env bash
set -euo pipefail

STRICTNESS="${VALIDATION_STRICTNESS:-5}"
export PATH="${HOME}/.local/bin:${PATH}"

require_file() {
    local path="$1"
    if [[ ! -e "$path" ]]; then
        echo "Missing expected plugin artifact: $path"
        exit 1
    fi
}

require_file "target/bundled/ultrawave-record.vst3"
require_file "target/bundled/ultrawave-play.vst3"
require_file "target/bundled/ultrawave-record.clap"
require_file "target/bundled/ultrawave-play.clap"

if [[ "$(uname -s)" == "Linux" ]]; then
    xvfb-run -a pluginval --validate-in-process --strictness-level "${STRICTNESS}" target/bundled/ultrawave-record.vst3
    xvfb-run -a pluginval --validate-in-process --strictness-level "${STRICTNESS}" target/bundled/ultrawave-play.vst3
else
    pluginval --validate-in-process --strictness-level "${STRICTNESS}" target/bundled/ultrawave-record.vst3
    pluginval --validate-in-process --strictness-level "${STRICTNESS}" target/bundled/ultrawave-play.vst3
fi

clap-validator validate target/bundled/ultrawave-record.clap
clap-validator validate target/bundled/ultrawave-play.clap
