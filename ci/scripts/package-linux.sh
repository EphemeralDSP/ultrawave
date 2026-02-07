#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIST_DIR="${PROJECT_ROOT}/dist"
VERSION="${VERSION:-${BUILDKITE_TAG:-$(git -C "${PROJECT_ROOT}" describe --tags --always --dirty)}}"

mkdir -p "${DIST_DIR}"

TAR_PATH="${DIST_DIR}/Ultrawave-${VERSION}-linux.tar.gz"
ZIP_PATH="${DIST_DIR}/Ultrawave-${VERSION}-linux.zip"

rm -f "${TAR_PATH}" "${ZIP_PATH}"

tar -czf "${TAR_PATH}" -C "${PROJECT_ROOT}" target/bundled
(
    cd "${PROJECT_ROOT}"
    zip -rq "${ZIP_PATH}" target/bundled
)

echo "Created ${TAR_PATH}"
echo "Created ${ZIP_PATH}"
