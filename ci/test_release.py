"""Release safety regressions; all repositories/artifacts are disposable and gh is mocked."""

import argparse
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import release


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="ultrawave release test ")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        root_patch = patch.object(release, "ROOT", self.root)
        root_patch.start()
        self.addCleanup(root_patch.stop)
        self.out = self.root / "target/artifacts with spaces"
        self.out.mkdir(parents=True)
        (self.root / ".gitignore").write_text("/target/\n")
        for name, package in zip(release.MANIFESTS, ("ultrawave", "ultrawave-plugin")):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(f'[package]\nname = "{package}"\nversion = "0.3.7"\n')
        (self.root / "Cargo.lock").write_text('version = 4\n' + "".join(
            f'[[package]]\nname = "{name}"\nversion = "0.3.7"\n'
            for name in ("ultrawave", "ultrawave-plugin")))
        self.git("init")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "user.name", "Release Test")
        self.git("add", ".")
        self.git("commit", "-m", "fixture")
        self.commit = self.git("rev-parse", "HEAD")

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, text=True,
                                       stderr=subprocess.DEVNULL).strip()

    def candidates(self):
        for target in ("source", *release.PLATFORMS):
            path = self.out / release.artifact_name("0.3.7", target)
            path.write_bytes(target.encode())
            release.receipt(self.out, {"version": "0.3.7", "commit": self.commit}, target, path)

    def test_clean_export_excludes_untracked_files_and_detects_source_edits(self):
        (self.out / "secret-ignored-file").write_text("must not be exported")
        release.prepare(argparse.Namespace(output=self.out))
        dest = self.out / "unpacked"
        dest.mkdir()
        with tarfile.open(self.out / release.artifact_name("0.3.7", "source")) as archive:
            self.assertNotIn(".git", archive.getnames())
            self.assertFalse(any("secret-ignored" in name for name in archive.getnames()))
            archive.extractall(dest, filter="data")
        with patch.object(release, "ROOT", dest):
            self.assertEqual(release.source_state(), {"version": "0.3.7", "commit": self.commit})
            (dest / "Cargo.toml").write_text('changed')
            with self.assertRaisesRegex(ValueError, "source changed"):
                release.source_state()

    def test_dirty_source_and_mismatched_versions_are_rejected(self):
        (self.root / "new-source.rs").write_text("uncommitted")
        with self.assertRaisesRegex(ValueError, "clean worktree"):
            release.prepare(argparse.Namespace(output=self.out))
        (self.root / "new-source.rs").unlink()
        plugin = self.root / release.MANIFESTS[1]
        plugin.write_text(plugin.read_text().replace("0.3.7", "0.3.8"))
        with self.assertRaisesRegex(ValueError, "versions must match"):
            release.version()

    def test_all_platforms_must_match_and_remain_unchanged(self):
        self.candidates()
        self.assertEqual(release.release_files(self.out, "0.3.7")[0], self.commit)
        manifest = self.out / "windows-x86_64.json"
        original = manifest.read_text()
        for key, wrong in (("commit", "a" * 40), ("version", "0.3.8"),
                           ("platform", "macos-arm64"), ("artifact", "../other.zip")):
            with self.subTest(key=key):
                info = json.loads(original)
                info[key] = wrong
                manifest.write_text(json.dumps(info))
                with self.assertRaises(ValueError):
                    release.release_files(self.out, "0.3.7")
        manifest.write_text(original)
        artifact = self.out / release.artifact_name("0.3.7", "windows-x86_64")
        artifact.write_bytes(b"changed after validation")
        with self.assertRaisesRegex(ValueError, "changed after validation"):
            release.release_files(self.out, "0.3.7")
        manifest.unlink()
        with self.assertRaises(FileNotFoundError):
            release.release_files(self.out, "0.3.7")

    def test_publication_is_draft_first_with_exact_tag_and_no_clobber(self):
        self.candidates()
        notes = self.out / "notes.md"
        notes.write_text("Release notes")
        args = argparse.Namespace(tag="v0.3.7", artifacts=self.out, notes=notes, publish=False)
        annotated = json.dumps({"object": {"type": "tag", "sha": "b" * 40}})
        commit = json.dumps({"object": {"type": "commit", "sha": self.commit}})
        with patch.object(release, "run", side_effect=[annotated, commit, "", "created"]) as command:
            release.publish(args)
        self.assertEqual(command.call_args_list[1].args[0][-1],
                         f"repos/{release.REPO}/git/tags/{'b' * 40}")
        create = command.call_args_list[-1].args[0]
        self.assertEqual(create[:3], ["gh", "release", "create"])
        self.assertIn("--draft", create)
        self.assertIn("--verify-tag", create)
        self.assertNotIn("--clobber", create)
        self.assertEqual(sum(str(a).endswith((".zip", ".tar.gz")) for a in create), 4)
        args.publish = True
        with patch.object(release, "run", side_effect=[commit, "", "created", "published"]) as command:
            release.publish(args)
        self.assertIn("--draft=false", command.call_args.args[0])
        for obj in ({"type": "commit", "sha": "c" * 40}, {"type": "tree", "sha": self.commit}):
            response = json.dumps({"object": obj})
            with patch.object(release, "run", return_value=response) as command:
                with self.assertRaisesRegex(ValueError, "tested commit"):
                    release.publish(args)
                self.assertEqual(command.call_count, 1)
        with patch.object(release, "run", side_effect=subprocess.CalledProcessError(1, "gh")) as command:
            with self.assertRaises(subprocess.CalledProcessError):
                release.publish(args)
            self.assertEqual(command.call_count, 1, "missing tag must not create a release")
        with patch.object(release, "run", side_effect=[commit, "v0.3.7"]) as command:
            with self.assertRaisesRegex(ValueError, "already exists"):
                release.publish(args)
            self.assertEqual(command.call_count, 2)
        with patch.object(release, "run", side_effect=[commit, "", subprocess.CalledProcessError(1, "gh")]) as command:
            with self.assertRaises(subprocess.CalledProcessError):
                release.publish(args)
            self.assertEqual(command.call_count, 3, "failed upload must never publish")

    def test_bump_changes_both_packages_and_lock_without_tagging(self):
        for name in release.MANIFESTS:
            path = self.root / name
            path.write_text(path.read_text() + '\n[lib]\npath = "lib.rs"\n')
            (path.parent / "lib.rs").write_text("")
        manifest = self.root / "Cargo.toml"
        manifest.write_text(manifest.read_text() + '\n[workspace]\nmembers = ["plugins/ultrawave"]\n')
        # Native Windows checkouts may use CRLF; preserve manifest line endings.
        manifest.write_bytes(manifest.read_text().replace("\n", "\r\n").encode())
        self.git("add", ".")
        self.git("commit", "-m", "buildable workspace")
        before = self.git("rev-parse", "HEAD")
        release.bump(argparse.Namespace(version="0.4.0-rc.1"))
        self.assertEqual(release.version(), "0.4.0-rc.1")
        self.assertIn(b'version = "0.4.0-rc.1"\r\n', manifest.read_bytes())
        self.assertEqual(self.git("rev-parse", "HEAD"), before)
        self.assertEqual(self.git("tag"), "")
        self.assertEqual(set(self.git("diff", "--name-only").splitlines()),
                         {*release.MANIFESTS, "Cargo.lock"})

    def test_failed_bump_restores_manifests_and_lock(self):
        original = {name: (self.root / name).read_bytes()
                    for name in (*release.MANIFESTS, "Cargo.lock")}
        real_run = release.run

        def fail_metadata(command, **kwargs):
            if command[:2] == ["cargo", "metadata"]:
                (self.root / "Cargo.lock").write_bytes(b"partially updated")
                raise subprocess.CalledProcessError(1, command)
            return real_run(command, **kwargs)

        with patch.object(release, "run", side_effect=fail_metadata):
            with self.assertRaises(subprocess.CalledProcessError):
                release.bump(argparse.Namespace(version="0.3.8"))
        for name, data in original.items():
            self.assertEqual((self.root / name).read_bytes(), data)
        self.assertEqual(self.git("status", "--porcelain"), "")

    def test_build_packages_only_after_checks_and_never_overwrites(self):
        for name in ("README.md", "LICENSE", "vendor/README.md", "vendor/nice-plug/LICENSE",
                     "vendor/nice-plug-core/LICENSE", "vendor/baseview/LICENSE-MIT",
                     "vendor/baseview/LICENSE-APACHE"):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("notice")
        ignore = self.root / ".gitignore"
        ignore.write_text(ignore.read_text() + "/vendor/nice-plug/Cargo.lock\n")
        self.git("add", ".")
        self.git("commit", "-m", "packaging inputs")
        bundles = self.root / "target/bundled"
        vst = bundles / "ultrawave-plugin.vst3/Contents/x86_64-linux/ultrawave-plugin.so"
        vst.parent.mkdir(parents=True)
        vst.write_bytes(b"vst3 fixture")
        (bundles / "ultrawave-plugin.clap").write_bytes(b"clap fixture")
        args = argparse.Namespace(output=self.out, pluginval="pluginval", clap_validator="clap-validator")
        with patch.object(release, "native_platform", return_value="linux-x86_64"), \
                patch.object(release.shutil, "which", return_value="tool"), \
                patch.dict(release.os.environ, {"CARGO_BUILD_TARGET": "", "CARGO_TARGET_DIR": ""}), \
                patch.object(release, "checked") as checked:
            checked.side_effect = RuntimeError("validator failed")
            with self.assertRaisesRegex(RuntimeError, "validator failed"):
                release.build(args)
            self.assertFalse(list(self.out.glob("*.zip")))
            self.assertFalse(list(self.out.glob("*.json")))
            checked.side_effect = None
            release.build(args)
            commands = [call.args[0] for call in checked.call_args_list]
            self.assertIn(["cargo", "xtask", "bundle", "ultrawave-plugin", "--release", "--locked"], commands)
            pluginval = next(cmd for cmd in commands if "--validate" in cmd)
            self.assertEqual(pluginval[pluginval.index("--strictness-level") + 1], "10")
            before = checked.call_count
            with self.assertRaisesRegex(ValueError, "already exists"):
                release.build(args)
            self.assertEqual(checked.call_count, before)
        info = json.loads((self.out / "linux-x86_64.json").read_text())
        archive = self.out / info["artifact"]
        self.assertEqual(info["commit"], self.git("rev-parse", "HEAD"))
        self.assertEqual(info["sha256"], release.digest(archive))
        with zipfile.ZipFile(archive) as package:
            self.assertEqual(package.read("ultrawave-plugin.clap"), b"clap fixture")
            self.assertEqual(package.read(str(vst.relative_to(bundles)).replace("\\", "/")), b"vst3 fixture")
            self.assertIn("vendor/nice-plug/LICENSE", package.namelist())
            self.assertFalse(any(name.startswith("target/") for name in package.namelist()))

    def test_validator_exit_and_caught_panics_both_fail(self):
        for code in ("raise SystemExit(7)", "print('thread panicked at audio callback')"):
            with self.subTest(code=code), self.assertRaises(RuntimeError):
                release.checked([sys.executable, "-c", code], self.out / "validator.log", None)
        release.checked([sys.executable, "-c", "print('validated')"], self.out / "validator.log", None)


if __name__ == "__main__":
    unittest.main()
