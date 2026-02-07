#!/usr/bin/env bash
set -euo pipefail

TAG="${BUILDKITE_TAG:-}"
ARTIFACT_DIR="${ARTIFACT_DIR:-artifacts}"

if [[ -z "${TAG}" ]]; then
    echo "BUILDKITE_TAG is required"
    exit 1
fi

if [[ -z "${GH_TOKEN:-}" && -n "${GITHUB_TOKEN:-}" ]]; then
    export GH_TOKEN="${GITHUB_TOKEN}"
fi

if [[ -z "${GH_TOKEN:-}" && -f /secrets/github-pat ]]; then
    export GH_TOKEN="$(cat /secrets/github-pat)"
fi

if [[ -z "${GH_TOKEN:-}" ]]; then
    echo "GH_TOKEN or GITHUB_TOKEN is required"
    exit 1
fi

REPO_SLUG="${GITHUB_REPOSITORY:-}"
if [[ -z "${REPO_SLUG}" && -n "${BUILDKITE_REPO:-}" ]]; then
    if [[ "${BUILDKITE_REPO}" =~ github.com[:/]+([^/]+/[^/.]+)(\.git)?$ ]]; then
        REPO_SLUG="${BASH_REMATCH[1]}"
    fi
fi

if [[ -z "${REPO_SLUG}" ]]; then
    echo "Unable to determine repository slug"
    exit 1
fi

mapfile -t FILES < <(find "${ARTIFACT_DIR}" -type f -name "Ultrawave-${TAG}-*")
if [[ ${#FILES[@]} -eq 0 ]]; then
    echo "No release files found under ${ARTIFACT_DIR}"
    exit 1
fi

if gh release view "${TAG}" -R "${REPO_SLUG}" >/dev/null 2>&1; then
    gh release upload "${TAG}" -R "${REPO_SLUG}" --clobber "${FILES[@]}"
else
    gh release create "${TAG}" -R "${REPO_SLUG}" \
        --title "Ultrawave ${TAG}" \
        --notes "Ultrawave ${TAG}" \
        "${FILES[@]}"
fi
