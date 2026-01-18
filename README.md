# Ultrawave

[![Plugin Validation](https://github.com/EphemeralDSP/ultrawave/actions/workflows/plugin-validation.yml/badge.svg)](https://github.com/EphemeralDSP/ultrawave/actions/workflows/plugin-validation.yml)
[![Build](https://github.com/EphemeralDSP/ultrawave/actions/workflows/build.yml/badge.svg)](https://github.com/EphemeralDSP/ultrawave/actions/workflows/build.yml)
[![Release](https://github.com/EphemeralDSP/ultrawave/actions/workflows/release.yml/badge.svg)](https://github.com/EphemeralDSP/ultrawave/actions/workflows/release.yml)
![Verified by pluginval](https://img.shields.io/badge/Verified%20by-pluginval-blue)
![Verified by clap-validator](https://img.shields.io/badge/Verified%20by-clap--validator-green)

A wavetable synthesizer plugin built with Rust and [NIH-plug](https://github.com/robbert-vdh/nih-plug).

## Features

- Wavetable synthesis with RamPlay/RamRecord
- Resonant filter system
- Modulation matrix with LFOs
- Effects processing
- VST3, CLAP, and AU plugin formats

## Building

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/EphemeralDSP/ultrawave.git
cd ultrawave
cargo build --release

# Bundle plugins
cargo xtask bundle ultrawave --release
```

Built plugins are placed in `target/bundled/`.

## Plugin Formats

| Format | Windows | macOS | Linux |
|--------|---------|-------|-------|
| VST3   | ✓       | ✓     | ✓     |
| CLAP   | ✓       | ✓     | ✓     |
| AU     | -       | ✓     | -     |

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

### Validation

Ultrawave uses industry-standard plugin validators:

```bash
# Validate VST3
.\scripts\validate-vst3.ps1      # Windows
./scripts/validate-vst3.sh       # macOS/Linux

# Validate CLAP
.\scripts\validate-clap.ps1      # Windows
./scripts/validate-clap.sh       # macOS/Linux
```

See [docs/testing/plugin-validation.md](docs/testing/plugin-validation.md) for details.

## License

See [LICENSE](LICENSE) for details.
