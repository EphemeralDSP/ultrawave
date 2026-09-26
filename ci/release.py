#!/usr/bin/env python3
"""Manual native releases. Requires Python 3.11+, Cargo and installed validators."""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
REPO = "EphemeralDSP/ultrawave"
PLATFORMS = ("linux-x86_64", "macos-arm64", "windows-x86_64")
MANIFESTS = ("Cargo.toml", "plugins/ultrawave/Cargo.toml")
SOURCE_INFO = "release-source.json"
VERSION = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-rc\.[1-9][0-9]*)?")


def run(args, *, cwd=None, env=None):
    return subprocess.run(args, cwd=cwd or ROOT, env=env, check=True, text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.strip()


def digest(path):
    with Path(path).open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()


def version():
    versions = [tomllib.loads((ROOT / path).read_text())["package"]["version"]
                for path in MANIFESTS]
    if len(set(versions)) != 1 or not VERSION.fullmatch(versions[0]):
        raise ValueError("Root and plugin Cargo versions must match (X.Y.Z or X.Y.Z-rc.N)")
    locked = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
    for name in ("ultrawave", "ultrawave-plugin"):
        if [p["version"] for p in locked if p["name"] == name] != [versions[0]]:
            raise ValueError(f"Cargo.lock version mismatch for {name}; run cargo metadata")
    return versions[0]


def clean_commit():
    if Path(run(["git", "rev-parse", "--show-toplevel"])).resolve() != ROOT.resolve():
        raise ValueError("Run in the Ultrawave repository, not an enclosing checkout")
    if run(["git", "status", "--porcelain"]):
        raise ValueError("Commit changes first; release builds require a clean worktree")
    return run(["git", "rev-parse", "HEAD"])


def source_state():
    if (ROOT / ".git").exists():
        commit = clean_commit()
    else:
        info = json.loads((ROOT / SOURCE_INFO).read_text())
        for name, expected in info["files"].items():
            path = ROOT / name
            if not path.resolve().is_relative_to(ROOT) or digest(path) != expected:
                raise ValueError(f"Exported source changed: {name}")
        commit = info["commit"]
        if info["version"] != version():
            raise ValueError("Exported source version mismatch")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("Invalid source commit")
    return {"version": version(), "commit": commit}


def write_json(path, value):
    with path.open("x", encoding="utf-8") as file:
        json.dump(value, file, indent=2)
        file.write("\n")


def artifact_name(ver, target):
    suffix = "tar.gz" if target == "source" else "zip"
    return f"Ultrawave-{ver}-{target}.{suffix}"


def receipt(output, state, target, artifact, **extra):
    write_json(output / f"{target}.json", dict(state, platform=target,
               artifact=artifact.name, sha256=digest(artifact), **extra))


def bump(args):
    clean_commit()
    old = version()
    if not VERSION.fullmatch(args.version) or args.version == old:
        raise ValueError("Choose a new version: X.Y.Z or X.Y.Z-rc.N")
    originals = {name: (ROOT / name).read_bytes() for name in (*MANIFESTS, "Cargo.lock")}
    updates = {}
    for name in MANIFESTS:
        text = originals[name].decode()
        updated, count = re.subn(r'^version = "' + re.escape(old) + r'"(?=\r?$)',
                                f'version = "{args.version}"', text, count=1, flags=re.M)
        if count != 1:
            raise ValueError(f"Cannot update version in {name}")
        updates[name] = updated.encode()
    try:
        for name, data in updates.items():
            (ROOT / name).write_bytes(data)
        # --no-deps does not resolve/update Cargo.lock.
        run(["cargo", "metadata", "--format-version", "1"])
        version()
    except (OSError, ValueError, subprocess.CalledProcessError):
        for name, data in originals.items():
            (ROOT / name).write_bytes(data)
        raise
    print("Updated manifests and Cargo.lock only. Review and commit; no tag or push was made.")


def prepare(args):
    state = {"version": version(), "commit": clean_commit()}
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    path = output / artifact_name(state["version"], "source")
    if path.exists() or (output / "source.json").exists():
        raise ValueError("Source output already exists; choose a fresh output directory")
    archive = subprocess.check_output(["git", "archive", "--format=tar", state["commit"]], cwd=ROOT)
    files = {}
    with tarfile.open(fileobj=io.BytesIO(archive)) as source, tarfile.open(path, "w:gz") as dest:
        for member in source:
            if not (member.isfile() or member.isdir()) or ".." in Path(member.name).parts:
                raise ValueError(f"Unsupported source archive entry: {member.name}")
            data = source.extractfile(member).read() if member.isfile() else None
            if data is not None:
                files[member.name] = hashlib.sha256(data).hexdigest()
            dest.addfile(member, io.BytesIO(data) if data is not None else None)
        data = json.dumps(dict(state, files=files)).encode()
        member = tarfile.TarInfo(SOURCE_INFO)
        member.size = len(data)
        member.mode = 0o644
        dest.addfile(member, io.BytesIO(data))
    receipt(output, state, "source", path)
    print(f"Prepared {path}\nCommit: {state['commit']}\nSHA256: {digest(path)}")


def native_platform():
    names = {("Linux", "x86_64"): "linux-x86_64",
             ("Darwin", "arm64"): "macos-arm64",
             ("Windows", "AMD64"): "windows-x86_64"}
    try:
        return names[(platform.system(), platform.machine())]
    except KeyError:
        raise ValueError("Supported native targets: " + ", ".join(PLATFORMS)) from None


def checked(args, log, env):
    print("Running:", subprocess.list2cmdline([str(a) for a in args]), flush=True)
    with log.open("w", encoding="utf-8") as stream:
        result = subprocess.run(args, cwd=ROOT, env=env, stdout=stream, stderr=subprocess.STDOUT)
    text = log.read_text(errors="replace")
    if result.returncode or re.search(r"panicked at|BadDrawable|BadWindow|Error while closing window", text):
        raise RuntimeError(f"Check failed: {log}\n{text[-6000:]}")


def build(args):
    state = source_state()
    target = native_platform()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    artifact = output / artifact_name(state["version"], target)
    if artifact.exists() or (output / f"{target}.json").exists():
        raise ValueError("Build output already exists; use a fresh directory (never overwrite a candidate)")
    tools = [args.pluginval, args.clap_validator, "cargo", "rustc"]
    if target.startswith("linux"):
        tools += ["xvfb-run"]
    for tool in tools:
        if not shutil.which(tool):
            raise ValueError(f"Required tool not found: {tool}")
    log_dir = output / f"{target}-logs"
    log_dir.mkdir(exist_ok=True)
    env = dict(os.environ, ULTRAWAVE_SCREENSHOT_WAV=str(ROOT / "tests/vinyl909.wav"))
    # Prevent an inherited target/architecture override from packaging a stale
    # native bundle. Each checkout owns target/ and Cargo.lock remains pinned.
    if env.get("CARGO_BUILD_TARGET") or env.get("CARGO_TARGET_DIR"):
        raise ValueError("Unset CARGO_BUILD_TARGET and CARGO_TARGET_DIR for native releases")
    commands = [
        ["cargo", "fmt", "--all", "--check"],
        ["cargo", "clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"],
        ["cargo", "test", "--workspace", "--release", "--locked"],
        ["cargo", "build", "-p", "ultrawave-plugin", "--release", "--locked"],
        ["cargo", "run", "--example", "clap_audio_harness", "--release", "--locked"],
    ]
    for index, command in enumerate(commands):
        checked(command, log_dir / f"{index}-cargo.log", env)
    shutil.copyfile(ROOT / "Cargo.lock", ROOT / "vendor/nice-plug/Cargo.lock")
    checked(["cargo", "test", "--manifest-path", "vendor/nice-plug/Cargo.toml",
             "--target-dir", "target", "--no-default-features", "--features", "vst3",
             "--lib", "--release", "wrapper::vst3"], log_dir / "vst3-state.log", env)
    checked(["cargo", "xtask", "bundle", "ultrawave-plugin", "--release", "--locked"],
            log_dir / "bundle.log", env)
    bundles = ROOT / "target/bundled"
    paths = [bundles / f"ultrawave-plugin.{ext}" for ext in ("vst3", "clap")]
    for path in paths:
        if not path.exists():
            raise ValueError(f"Missing bundle: {path}")
    prefix = ["xvfb-run", "-a"] if target.startswith("linux") else []
    checked(prefix + [args.pluginval, "--validate-in-process", "--strictness-level", "10",
                      "--validate", str(paths[0])], log_dir / "pluginval.log", env)
    checked([args.clap_validator, "validate", str(paths[1])], log_dir / "clap-validator.log", env)
    if target == "macos-arm64":
        for path in paths:
            checked(["codesign", "--verify", "--deep", "--strict", str(path)],
                    log_dir / f"codesign-{path.suffix[1:]}.log", env)
    if source_state() != state:
        raise ValueError("Source changed during build")
    with zipfile.ZipFile(artifact, "x", compression=zipfile.ZIP_DEFLATED) as zip_file:
        for path in paths:
            for file in sorted(path.rglob("*")) if path.is_dir() else [path]:
                if file.is_file():
                    zip_file.write(file, file.relative_to(bundles))
        for name in ("README.md", "LICENSE", "vendor/README.md", "vendor/nice-plug/LICENSE",
                     "vendor/nice-plug-core/LICENSE", "vendor/baseview/LICENSE-MIT",
                     "vendor/baseview/LICENSE-APACHE"):
            zip_file.write(ROOT / name, name)
    receipt(output, state, target, artifact, rustc=run(["rustc", "-vV"]),
            system=platform.platform(), libc=platform.libc_ver(),
            signing="ad-hoc, not notarized" if target == "macos-arm64" else "unsigned")
    print(f"Validated native candidate: {artifact}\nSHA256: {digest(artifact)}")


def release_files(directory, ver):
    files, commits = [], set()
    for target in ("source", *PLATFORMS):
        manifest = directory / f"{target}.json"
        info = json.loads(manifest.read_text())
        name = artifact_name(ver, target)
        if info["version"] != ver or info["platform"] != target or info["artifact"] != name:
            raise ValueError(f"Wrong version/platform/artifact in {manifest}")
        if not re.fullmatch(r"[0-9a-f]{40}", info["commit"]):
            raise ValueError(f"Invalid commit in {manifest}")
        path = directory / name
        if digest(path) != info["sha256"]:
            raise ValueError(f"Artifact changed after validation: {path}")
        commits.add(info["commit"])
        files += [path, manifest]
    if len(commits) != 1:
        raise ValueError("All native builds and source must come from the same commit")
    return commits.pop(), files


def publish(args):
    if not args.tag.startswith("v") or not VERSION.fullmatch(args.tag[1:]):
        raise ValueError("Use an exact version tag, vX.Y.Z or vX.Y.Z-rc.N")
    directory = args.artifacts.resolve()
    commit, files = release_files(directory, args.tag[1:])
    # --verify-tag alone verifies existence, not that its commit was tested.
    obj = json.loads(run(["gh", "api", f"repos/{REPO}/git/ref/tags/{args.tag}"]))["object"]
    while obj["type"] == "tag":
        obj = json.loads(run(["gh", "api", f"repos/{REPO}/git/tags/{obj['sha']}"]))["object"]
    if obj["type"] != "commit" or obj["sha"] != commit:
        raise ValueError("Remote version tag does not point to the tested commit; no tag is created or moved")
    tags = run(["gh", "api", f"repos/{REPO}/releases", "--paginate", "--jq", ".[].tag_name"])
    if args.tag in tags.splitlines():
        raise ValueError("Release already exists (including drafts); inspect it manually, never clobber")
    notes = args.notes.resolve()
    if not notes.is_file() or not notes.read_text().strip():
        raise ValueError("A nonempty release notes file is required")
    with tempfile.TemporaryDirectory(prefix="ultrawave-publish-") as tmp:
        checksums = Path(tmp) / "SHA256SUMS"
        checksums.write_text("".join(f"{digest(p)}  {p.name}\n" for p in files))
        run(["gh", "release", "create", args.tag, "--repo", REPO, "--verify-tag", "--draft",
             "--title", f"Ultrawave {args.tag}", "--notes-file", str(notes),
             *map(str, files), str(checksums)])
    if args.publish:
        run(["gh", "release", "edit", args.tag, "--repo", REPO, "--draft=false",
             "--prerelease=" + str("-rc." in args.tag).lower()])
    print(f"{'Published' if args.publish else 'Created draft'}: https://github.com/{REPO}/releases")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    p = commands.add_parser("bump", help="edit Cargo versions/lock only; no commit/tag/push")
    p.add_argument("version")
    for action in ("prepare", "build"):
        p = commands.add_parser(action, help="export committed source" if action == "prepare"
                                else "test, validate and package this native platform")
        p.add_argument("--output", type=Path, default=ROOT / "target/release-candidate")
        if action == "build":
            p.add_argument("--pluginval", default="pluginval")
            p.add_argument("--clap-validator", default="clap-validator")
    p = commands.add_parser("publish", help="verify all three platforms and create a GitHub draft")
    p.add_argument("--tag", required=True)
    p.add_argument("--artifacts", type=Path, required=True)
    p.add_argument("--notes", type=Path, required=True)
    p.add_argument("--publish", action="store_true", help="also make the completed draft visible")
    args = parser.parse_args()
    try:
        {"bump": bump, "prepare": prepare, "build": build, "publish": publish}[args.command](args)
    except (ValueError, OSError, KeyError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Release stopped: {error}", file=sys.stderr)
        if isinstance(error, subprocess.CalledProcessError):
            print(error.stderr or "", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
