#!/usr/bin/env bash
set -euo pipefail

CARGO_BIN="${CARGO:-cargo}"
"${CARGO_BIN}" xtask bundle ultrawave-record --release
"${CARGO_BIN}" xtask bundle ultrawave-play --release
