#!/usr/bin/env bash
set -euo pipefail

if ! command -v rustup >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi

if [[ -f "${HOME}/.cargo/env" ]]; then
    # shellcheck disable=SC1090
    source "${HOME}/.cargo/env"
fi
rustup toolchain install stable --profile minimal
rustup default stable

TOOLS_DIR="${HOME}/.local/ultrawave-ci-tools"
BIN_DIR="${HOME}/.local/bin"
mkdir -p "${TOOLS_DIR}" "${BIN_DIR}"

if ! command -v pluginval >/dev/null 2>&1; then
    curl -fsSL -o "${TOOLS_DIR}/pluginval.zip" \
        "https://github.com/Tracktion/pluginval/releases/download/v1.0.3/pluginval_macOS.zip"
    unzip -qo "${TOOLS_DIR}/pluginval.zip" -d "${TOOLS_DIR}"
    ln -sf "${TOOLS_DIR}/pluginval.app/Contents/MacOS/pluginval" "${BIN_DIR}/pluginval"
fi

if ! command -v clap-validator >/dev/null 2>&1; then
    curl -fsSL -o "${TOOLS_DIR}/clap-validator.tar.gz" \
        "https://github.com/free-audio/clap-validator/releases/download/0.3.2/clap-validator-0.3.2-macos-universal.tar.gz"
    tar -xzf "${TOOLS_DIR}/clap-validator.tar.gz" -C "${TOOLS_DIR}"
    chmod +x "${TOOLS_DIR}/binaries/clap-validator"
    ln -sf "${TOOLS_DIR}/binaries/clap-validator" "${BIN_DIR}/clap-validator"
fi

export PATH="${BIN_DIR}:${PATH}"
echo "macOS dependencies ready"
