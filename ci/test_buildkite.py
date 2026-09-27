"""Buildkite orchestration tests. No agent, build or network call is made."""

import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import buildkite
import release


class BuildkiteTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="ultrawave buildkite test ")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        env = {"ULTRAWAVE_VALIDATE": "1", "ULTRAWAVE_PLATFORM": "macos-arm64",
               "BUILDKITE_JOB_ID": "job-123", "BUILDKITE_RETRY_COUNT": "2",
               "HOME": str(self.root), "USERPROFILE": str(self.root),
               "BUILDKITE_COMMIT": "a" * 40, "PATH": os.environ.get("PATH", "")}
        for item in (patch.dict(os.environ, env, clear=True),
                     patch.object(release, "ROOT", self.root)):
            item.start()
            self.addCleanup(item.stop)

    def test_upload_requires_explicit_valid_selection(self):
        with patch.object(buildkite.subprocess, "run") as run:
            for key, value in (("ULTRAWAVE_VALIDATE", "0"), ("ULTRAWAVE_PLATFORM", "macos")):
                with patch.dict(os.environ, {key: value}), self.assertRaises(ValueError):
                    buildkite.main(["--upload"])
            run.assert_not_called()
            buildkite.main(["--upload"])
            self.assertEqual(run.call_args.args[0], ["buildkite-agent", "pipeline", "upload",
                                                    "--no-interpolation", ".buildkite/pipeline.yml"])

    def test_success_and_failure_upload_only_this_attempt(self):
        with patch.object(release, "native_platform", return_value="macos-arm64"), \
                patch.object(release, "clean_commit", return_value="a" * 40), \
                patch.object(release, "checked"), patch.object(release, "prepare"), \
                patch.object(release, "build") as build, \
                patch.object(buildkite.subprocess, "run") as run:
            for failure in (None, RuntimeError("native validator failed")):
                build.side_effect = failure
                if failure:
                    with self.assertRaisesRegex(RuntimeError, "native validator failed"):
                        buildkite.main([])
                else:
                    self.assertEqual(buildkite.main([]), 0)
                self.assertEqual(build.call_args.args[0].output,
                                 self.root / "target/buildkite/job-123-2")
                self.assertEqual(run.call_args.args[0], ["buildkite-agent", "artifact", "upload",
                                                        "target/buildkite/job-123-2/**/*"])
            build.side_effect = None
            run.side_effect = buildkite.subprocess.CalledProcessError(1, "artifact upload")
            with self.assertRaises(buildkite.subprocess.CalledProcessError):
                buildkite.main([])

    def test_wrong_agent_or_commit_stops_before_build_or_upload(self):
        with patch.object(release, "build") as build, \
                patch.object(buildkite.subprocess, "run") as run:
            with patch.object(release, "native_platform", return_value="windows-x86_64"):
                with self.assertRaisesRegex(ValueError, "agent does not match"):
                    buildkite.main([])
            with patch.object(release, "native_platform", return_value="macos-arm64"), \
                    patch.object(release, "clean_commit", return_value="b" * 40):
                with self.assertRaisesRegex(ValueError, "checkout does not match"):
                    buildkite.main([])
            build.assert_not_called()
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
