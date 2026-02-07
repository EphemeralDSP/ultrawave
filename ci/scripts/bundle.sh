#!/usr/bin/env bash
set -euo pipefail

cargo xtask bundle ultrawave-record --release
cargo xtask bundle ultrawave-play --release
