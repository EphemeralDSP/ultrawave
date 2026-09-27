#!/usr/bin/env python3
"""Buildkite adapter for the native release gate; never installs or publishes."""

import argparse
import os
from pathlib import Path
import subprocess
import sys

import release


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upload", action="store_true", help="upload the repository pipeline only")
    args = parser.parse_args(argv)
    selected = os.environ.get("ULTRAWAVE_PLATFORM")
    if os.environ.get("ULTRAWAVE_VALIDATE") != "1" or selected not in (*release.PLATFORMS, "all"):
        raise ValueError("Set ULTRAWAVE_VALIDATE=1 and ULTRAWAVE_PLATFORM to a native target or all")
    job = os.environ["BUILDKITE_JOB_ID"]
    if args.upload:
        subprocess.run(["buildkite-agent", "pipeline", "upload", "--no-interpolation",
                        ".buildkite/pipeline.yml"], cwd=release.ROOT, check=True)
        return 0

    target = release.native_platform()
    if selected not in (target, "all"):
        raise ValueError("This agent does not match ULTRAWAVE_PLATFORM")
    if release.clean_commit() != os.environ["BUILDKITE_COMMIT"]:
        raise ValueError("Agent checkout does not match the requested build commit")
    # Use provisioned tools without changing the user's default Rust toolchain.
    env = os.environ.copy()
    env["PATH"] = os.pathsep.join([str(Path.home() / ".cargo/bin"),
                                  str(Path.home() / ".local/bin"), env["PATH"]])
    output = release.ROOT / "target/buildkite" / f"{job}-{env.get('BUILDKITE_RETRY_COUNT', '0')}"
    output.mkdir(parents=True, exist_ok=True)
    # release.build() inherits the process environment, as on a manual native run.
    os.environ["PATH"] = env["PATH"]
    try:
        release.checked([sys.executable, "ci/test_release.py"], output / "release-tests.log", env)
        release.checked([sys.executable, "ci/test_buildkite.py"], output / "buildkite-tests.log", env)
        release.prepare(argparse.Namespace(output=output))
        release.build(argparse.Namespace(output=output,
                      pluginval=env.get("PLUGINVAL_BIN", "pluginval"),
                      clap_validator=env.get("CLAP_VALIDATOR_BIN", "clap-validator")))
    finally:
        # Upload only this job/attempt, including failure logs. Never pick up
        # another build's ZIPs from a reused agent checkout.
        pattern = output.relative_to(release.ROOT).as_posix() + "/**/*"
        subprocess.run(["buildkite-agent", "artifact", "upload", pattern],
                       cwd=release.ROOT, env=env, check=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
