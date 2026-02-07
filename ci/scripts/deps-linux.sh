#!/usr/bin/env bash
set -euo pipefail

if command -v sudo >/dev/null 2>&1 && [ "${EUID}" -ne 0 ]; then
    SUDO="sudo"
else
    SUDO=""
fi

PACKAGES=(
    curl
    wget
    unzip
    zip
    xvfb
    imagemagick
    pkg-config
    libgl1-mesa-dev
    libxcb1-dev
    libxcb-icccm4-dev
    libxcb-image0-dev
    libxcb-keysyms1-dev
    libxcb-render-util0-dev
    libxcb-xkb-dev
    libxkbcommon-x11-dev
    libxkbcommon-dev
    libasound2-dev
    libxcursor-dev
    libx11-dev
    libx11-xcb-dev
    libjack-dev
    libxcb-dri2-0-dev
    libfontconfig1-dev
)

${SUDO} apt-get update -qq
${SUDO} apt-get install -y --no-install-recommends "${PACKAGES[@]}"

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
        "https://github.com/Tracktion/pluginval/releases/download/v1.0.3/pluginval_Linux.zip"
    unzip -qo "${TOOLS_DIR}/pluginval.zip" -d "${TOOLS_DIR}"
    chmod +x "${TOOLS_DIR}/pluginval"
    ln -sf "${TOOLS_DIR}/pluginval" "${BIN_DIR}/pluginval"
fi

if ! command -v clap-validator >/dev/null 2>&1; then
    curl -fsSL -o "${TOOLS_DIR}/clap-validator.tar.gz" \
        "https://github.com/free-audio/clap-validator/releases/download/0.3.2/clap-validator-0.3.2-ubuntu-18.04.tar.gz"
    tar -xzf "${TOOLS_DIR}/clap-validator.tar.gz" -C "${TOOLS_DIR}"
    chmod +x "${TOOLS_DIR}/clap-validator"
    ln -sf "${TOOLS_DIR}/clap-validator" "${BIN_DIR}/clap-validator"
fi

export PATH="${BIN_DIR}:${PATH}"
echo "Linux dependencies ready"
