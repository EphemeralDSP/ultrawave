#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIST_DIR="${PROJECT_ROOT}/dist"
VERSION="${VERSION:-${BUILDKITE_TAG:-$(git -C "${PROJECT_ROOT}" describe --tags --always --dirty)}}"

mkdir -p "${DIST_DIR}"

ZIP_PATH="${DIST_DIR}/Ultrawave-${VERSION}-macos.zip"
rm -f "${ZIP_PATH}"

(
    cd "${PROJECT_ROOT}"
    zip -rq "${ZIP_PATH}" target/bundled
)

echo "Created ${ZIP_PATH}"
