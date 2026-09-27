# Manual native releases

Normal commits, version edits and tags do **not** publish binaries. Run
`ci/release.py` explicitly. It needs no CI service or inbound SSH on the nodes.
One command builds each native platform; collect their outputs before publishing.
The repository stays private unless its owner separately changes visibility.

Targets are **Linux x86-64, Apple Silicon macOS, and Windows x86-64/MSVC**.
Each ZIP contains the unified VST3 and CLAP, README and license notices.
There is no Intel Mac/universal, AU, standalone executable or installer package.

## Prerequisites

- Python 3.11+, Git, Rust 1.95+ with rustfmt and Clippy. On nodes with an older
  default Rust, set `RUSTUP_TOOLCHAIN=1.95.0` for the commands below.
- Access to the pinned private `EphemeralDSP/ephemeral` dependency through the
  node's existing Git authentication. Do not copy credentials into source archives.
- Native toolchains: Linux packages in `ci/scripts/deps-linux.sh` plus Xvfb;
  Xcode command-line tools on Mac; Visual Studio 2022 C++ tools/Windows SDK on
  Windows (use a Developer PowerShell if MSVC isn't discoverable).
- [pluginval](https://github.com/Tracktion/pluginval/releases) and
  [clap-validator](https://github.com/free-audio/clap-validator/releases).
  Put them on PATH or pass `--pluginval` and `--clap-validator` executable paths.
- GitHub CLI with repository access on the publishing machine only.

No command here installs plugins, provisions services or changes firewall rules.
Run one build at a time per checkout. Do not edit source while a build runs.
Unset `CARGO_BUILD_TARGET` and `CARGO_TARGET_DIR`; builds are native, not cross builds.

## Buildkite dispatch (manual, no publisher)

Buildkite can run the same native gate instead of invoking it on each node.
The hosted configuration must match [bootstrap.yml](../.buildkite/bootstrap.yml),
which loads [pipeline.yml](../.buildkite/pipeline.yml) from the checked-out commit.
Set the GitHub provider's `trigger_mode` to `none` and `build_branches`,
`build_pull_requests`, and `build_tags` to `false`. Updating repository YAML
alone does not change an old hosted inline pipeline. Changing these hosted
settings requires the owner's approval.

Create a build in the UI or with the authenticated Buildkite CLI, specifying
the exact committed revision and both variables:

```bash
BUILDKITE_ORGANIZATION_SLUG=jmlynn bk build create --yes \
  --pipeline ultrawave --branch main --commit COMMIT_SHA \
  --env ULTRAWAVE_VALIDATE=1 --env ULTRAWAVE_PLATFORM=macos-arm64 \
  --message "Manual native validation; no publication"
```

`ULTRAWAVE_PLATFORM` accepts `linux-x86_64`, `macos-arm64`, `windows-x86_64`
or `all`. With validation enabled, an invalid/missing platform fails the loader
instead of producing an empty green build. Only UI/API builds are allowed by
the step conditions.
The loader currently uses the Mac agent. Native jobs select `queue=juce-build`
and their OS; Mac also requires `arch=arm64`. Do not select `all` until all three
agents are connected and provisioned. Agent/service changes are separate work.

Jobs use existing Rust **1.95.0** (with rustfmt and Clippy), Python 3.11+ and
validators. Mac invokes Homebrew Python, Windows `py -3.12`, Linux `python3`.
The adapter adds the user's `.cargo/bin` and `.local/bin` to PATH. Set
`PLUGINVAL_BIN`/`CLAP_VALIDATOR_BIN` for validator executables elsewhere.
No dependency installer, default-toolchain change or plugin installer runs.

Each attempt uploads its source archive/receipt, native ZIP/receipt and logs
under `target/buildkite/JOB_ID-RETRY/`. Failure logs are uploaded too; a failed
validation never becomes successful because artifact upload succeeded. No
previous job's output is selected from a reused checkout. Download artifacts
from successful jobs of the **same build/commit**; collect all three native
ZIPs/receipts plus one matching source archive/receipt before using `publish`.
Buildkite never creates a GitHub release or pushes a tag. Publishing remains
the separate approved operation in step 4 below.

## 1. Choose and commit the version

From a clean checkout (example version; choose the intended one):

```bash
python3 ci/release.py bump 0.2.0
git diff
# Review and commit Cargo.toml, plugins/ultrawave/Cargo.toml and Cargo.lock.
```

`bump` updates both package versions and the lockfile. It does not commit, tag,
push or build. An already-versioned clean commit can go straight to `prepare`.
Release candidates use versions such as `0.2.0-rc.1`.

## 2. Export one exact source revision

```bash
python3 ci/test_release.py
python3 ci/release.py prepare --output target/release-candidate
```

This requires clean, committed source and matching manifest/lockfile versions.
It writes `Ultrawave-VERSION-source.tar.gz` and `source.json`. The archive
contains tracked files only, no `.git`, ignored files or credentials. Its
`release-source.json` records the commit and per-file hashes. Exported builds
verify these before and after validation.

Copy the archive to Mac and Windows through your existing file-transfer path.
Verify its SHA-256 against `source.json` before extracting to a fresh directory.
On Windows, `Get-FileHash ARCHIVE -Algorithm SHA256` and `tar -xf ARCHIVE` work.
Do not extract over a working checkout or mix revisions. Keep the original
source receipt/archive on the publishing machine.

## 3. Run the same native build on each node

From the source root on each machine:

```bash
# Linux/macOS; add explicit validator paths when they aren't on PATH.
python3 ci/release.py build --output target/release-candidate \
  --pluginval /path/to/pluginval --clap-validator /path/to/clap-validator
```

```powershell
# Windows Developer PowerShell; use your actual validator paths.
$env:RUSTUP_TOOLCHAIN = "1.95.0"
py -3 ci/release.py build --output target/release-candidate --pluginval C:\Tools\pluginval.exe --clap-validator C:\Tools\clap-validator.exe
```

The gate runs formatting, warning-free Clippy, locked release workspace tests,
the exported CLAP audio harness, vendored VST3 state tests, locked bundling,
pluginval **strictness 10** and clap-validator. Mac also verifies bundle code
signatures. The wrapper-state test uses a disposable ignored vendor lockfile
seeded from the root lockfile; Cargo may add that package's test-only dependencies.
Logs live in `PLATFORM-logs/` under the output directory.

Only a successful pass produces `Ultrawave-VERSION-PLATFORM.zip` and
`PLATFORM.json`. The receipt records source commit/version, artifact hash,
Rust/toolchain, OS/libc and signing status. Failures stop the command; do not
publish logs as a substitute for a missing receipt. Existing candidates are
never overwritten: use a fresh output directory when retrying a completed or
partially packaged candidate.

Copy each platform ZIP **and receipt** into the publishing machine's candidate
directory beside the source archive/receipt. Retain validation logs for review.
All four receipts must identify the same version and commit.

## 4. Explicitly tag and publish

This step changes GitHub. Obtain the owner's release approval first. Review the
three native results, run relevant real-host smoke tests (Renoise on Linux,
REAPER on Mac), inspect changed editor states and write release notes including
known limitations. Windows validator success is not a Windows DAW test.

Push an exact version tag pointing to the tested commit using the normal Git
workflow. Never move a published tag. The release command **will not create a
tag**, and checks the remote tag's commit, including annotated tags.

```bash
# Creates a DRAFT with all assets and SHA256SUMS, not a visible release.
python3 ci/release.py publish --tag v0.2.0 \
  --artifacts target/release-candidate --notes /path/to/release-notes.md
```

Inspect that draft in GitHub and publish it manually. Alternatively, add
`--publish` to the initial command to make it visible only after the draft and
all uploads succeed. `-rc.N` versions are marked prereleases when using
`--publish`. Existing releases **including drafts** are rejected; this is not an
upload/resume command. After a network/upload failure, inspect GitHub for a
partial draft before doing anything else. Never blindly retry or use `--clobber`.

## Distribution and CI limits

Mac bundles are ad-hoc signed, **not Developer ID signed or notarized**; Windows
bundles are unsigned. OS trust prompts may occur. Signing/notarization requires
a separately authorized identity/setup. Linux builds use the build host's libc;
an Arch build is not a claim of compatibility with older distributions.

The old automatic tag publisher and platform-specific package scripts have
been removed. The checked-in Buildkite pipeline only produces validated
candidates and logs. Verify the hosted bootstrap and provider settings above
before use; a repository change alone cannot disable a hosted inline publisher.
