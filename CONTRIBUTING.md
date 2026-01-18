# Contributing to Ultrawave

Thank you for your interest in contributing to Ultrawave!

## Getting Started

1. Fork and clone the repository
2. Install Rust via [rustup](https://rustup.rs/)
3. Build the project: `cargo build`
4. Run tests: `cargo test`

## Development Workflow

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Bundle all plugin formats
cargo xtask bundle ultrawave --release
```

### Testing

```bash
# Run unit tests
cargo test

# Run clippy
cargo clippy

# Check formatting
cargo fmt --check
```

## Plugin Validation Requirements

**All pull requests must pass plugin validation at strictness level 5+.**

Before submitting a PR, validate your changes locally:

```bash
# Build the plugin
cargo xtask bundle ultrawave --release

# Validate VST3
.\scripts\validate-vst3.ps1      # Windows
./scripts/validate-vst3.sh       # macOS/Linux

# Validate CLAP
.\scripts\validate-clap.ps1      # Windows
./scripts/validate-clap.sh       # macOS/Linux
```

See [docs/testing/plugin-validation.md](docs/testing/plugin-validation.md) for detailed validation instructions.

### CI Validation

GitHub Actions automatically validates plugins on:
- Windows (VST3, CLAP)
- macOS (VST3, CLAP, AU)
- Linux (VST3, CLAP)

PRs are blocked if validation fails.

## Pull Request Guidelines

1. **Create focused PRs**: One feature or fix per PR
2. **Test locally**: Run `cargo test` and plugin validation
3. **Update docs**: Update documentation if changing behavior
4. **Follow code style**: Match existing patterns and formatting
5. **Write clear commit messages**: Describe what and why

## Code Style

- Follow Rust conventions and idioms
- Run `cargo fmt` before committing
- Address `cargo clippy` warnings
- No `unsafe` without justification and documentation

## Reporting Issues

When reporting bugs, include:
- OS and version
- Steps to reproduce
- Expected vs actual behavior
- Plugin validation output (if applicable)

## Questions?

Open an issue for questions about contributing.
