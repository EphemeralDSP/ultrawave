#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
    echo "Editor lifecycle regression test is Linux-only."
    exit 0
fi

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly PLUGINVAL="${PLUGINVAL:-${ROOT_DIR}/target/tools/pluginval}"
readonly REPEATS="${EDITOR_LIFECYCLE_REPEATS:-10}"
readonly STRICTNESS="${VALIDATION_STRICTNESS:-5}"
readonly LOG_DIR="${EDITOR_LIFECYCLE_LOG_DIR:-${TMPDIR:-/tmp}/ultrawave-editor-lifecycle-$$}"
readonly FAILURE_PATTERN='BadDrawable|BadWindow|glXSwapBuffers|panicked at|thread .* panicked|X Error of failed request|Error while closing window'

if [[ ! -x "${PLUGINVAL}" ]]; then
    echo "pluginval is not executable: ${PLUGINVAL}" >&2
    exit 1
fi

mkdir -p "${LOG_DIR}"

for plugin in ultrawave-plugin; do
    bundle="${ROOT_DIR}/target/bundled/${plugin}.vst3"
    log="${LOG_DIR}/${plugin}.log"

    if [[ ! -e "${bundle}" ]]; then
        echo "Missing plugin bundle: ${bundle}" >&2
        exit 1
    fi

    echo "Stress-testing ${plugin} editor lifecycle (${REPEATS} repeats)"
    set +e
    xvfb-run -a "${PLUGINVAL}" \
            --validate-in-process \
            --strictness-level "${STRICTNESS}" \
            --repeat "${REPEATS}" \
            "${bundle}" 2>&1 | tee "${log}"
    status=${PIPESTATUS[0]}
    set -e

    if (( status != 0 )); then
        echo "pluginval failed for ${plugin} (status ${status}); log: ${log}" >&2
        exit "${status}"
    fi

    if grep -E -q "${FAILURE_PATTERN}" "${log}"; then
        echo "Editor teardown failure found for ${plugin}; log: ${log}" >&2
        grep -E -n "${FAILURE_PATTERN}" "${log}" >&2
        exit 1
    fi
done

echo "Editor lifecycle regression passed; logs: ${LOG_DIR}"
