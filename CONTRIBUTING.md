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
cargo xtask bundle ultrawave-plugin --release
```

### Testing

```bash
# Quick offline audio/state regression loop
./scripts/dev-loop.sh fast

# Release tests, Clippy, validators, benchmark and editor lifecycle
./scripts/dev-loop.sh full --editor
```

## Plugin Validation Requirements

**All pull requests must pass plugin validation at strictness level 5+.**

Before submitting a PR, validate your changes locally:

```bash
# Build the plugin
cargo xtask bundle ultrawave-plugin --release

# Validate VST3
./scripts/validate-vst3.sh --plugin target/bundled/ultrawave-plugin.vst3

# Validate CLAP
./scripts/validate-clap.sh --plugin target/bundled/ultrawave-plugin.clap
```

See [docs/testing/plugin-validation.md](docs/testing/plugin-validation.md) for detailed validation instructions.

### CI Validation

The [Buildkite pipeline](.buildkite/pipeline.yml) dispatches the same native
release gate on explicitly selected platforms. Set `ULTRAWAVE_VALIDATE=1` and
`ULTRAWAVE_PLATFORM` as described in the [release workflow](docs/releasing.md).
It uploads candidates/logs, never publishes or installs plugins. The hosted
pipeline must use the checked-in bootstrap with automatic provider triggers
disabled. Run local checks before proposing changes; CI configuration alone
does not prove a DAW works.

### Releases

Use the [manual native release workflow](docs/releasing.md) for Linux x86-64,
Apple Silicon macOS and Windows x86-64 VST3/CLAP builds. `ci/release.py` separates
version bump, source export, native validation/packaging and explicit GitHub
publication. Publication requires all three platforms from the same commit and
an exact remote version tag; ordinary commits never invoke this publisher.
Run `python3 ci/test_release.py` and `python3 ci/test_buildkite.py` when modifying
the release tooling.

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
