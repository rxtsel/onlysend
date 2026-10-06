import json
import base64
import subprocess
from unittest.mock import patch
import tempfile
import tomllib
import unittest
from pathlib import Path
import release


class VersionTests(unittest.TestCase):
    def test_semver_rejects_injection_prereleases_and_leading_zeros(self):
        for value in ["v1.2.3", "1.2.3;exit", "1.2.3-beta", "01.2.3", "1.2", "1.2.3\n"]:
            with self.assertRaises(ValueError): release.semver(value)
        self.assertEqual(release.semver("0.2.0"), (0, 2, 0))

    def test_a_failed_publish_retry_cannot_rollback_latest(self):
        release.assert_newest("0.2.0", ["v0.1.0", "v0.2.0"])
        with self.assertRaises(ValueError): release.assert_newest("0.2.0", ["v0.3.0"])

    def test_versions_are_aligned_without_changing_dependency_versions(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); (root / "src-tauri").mkdir()
            (root / "package.json").write_text('{"version":"0.1.1"}')
            (root / "src-tauri/tauri.conf.json").write_text('{"version":"0.1.1","identifier":"com.test.app"}')
            (root / "src-tauri/Cargo.toml").write_text('[package]\nname = "only-send"\nversion = "0.1.1"\n[dependencies]\nserde = "1"\n')
            (root / "src-tauri/Cargo.lock").write_text('version = 4\n[[package]]\nname = "other"\nversion = "0.1.1"\n[[package]]\nname = "only-send"\nversion = "0.1.1"\n')
            release.prepare(root, "0.2.0", ["v0.1.0"])
            self.assertEqual(json.loads((root / "package.json").read_text())["version"], "0.2.0")
            config = json.loads((root / "src-tauri/tauri.conf.json").read_text())
            self.assertEqual(config["version"], "0.2.0"); self.assertEqual(config["identifier"], "com.test.app")
            cargo = tomllib.loads((root / "src-tauri/Cargo.toml").read_text())
            self.assertEqual(cargo["package"]["version"], "0.2.0")
            entries = tomllib.loads((root / "src-tauri/Cargo.lock").read_text())["package"]
            self.assertEqual([entry["version"] for entry in entries], ["0.1.1", "0.2.0"])
            with self.assertRaises(ValueError): release.prepare(root, "0.2.1", ["v0.3.0"])
            self.assertEqual(json.loads((root / "package.json").read_text())["version"], "0.2.0")


class ArtifactTests(unittest.TestCase):
    def fixtures(self, root, missing=None):
        for platform, extension in release.PLATFORMS.items():
            directory = root / (platform + "-bundle"); directory.mkdir()
            name = "OnlySend" + extension
            (directory / name).write_bytes(b"signed fixture bytes")
            if platform != missing: (directory / (name + ".sig")).write_text("signature-" + platform)
            if platform == "linux-x86_64": (directory / "OnlySend.deb").write_bytes(b"deb")
            if platform.startswith("darwin"): (directory / "OnlySend.dmg").write_bytes(b"dmg")

    def test_complete_manifest_uses_arch_specific_immutable_urls_and_signature_contents(self):
        with tempfile.TemporaryDirectory() as temp:
            source = Path(temp) / "source"; source.mkdir(); self.fixtures(source)
            output = Path(temp) / "out"
            release.collect(source, output, "0.2.0", "rxtsel/onlysend", "Notes")
            manifest = json.loads((output / "latest.json").read_text())
            self.assertEqual(set(manifest["platforms"]), set(release.PLATFORMS))
            for platform, data in manifest["platforms"].items():
                self.assertEqual(data["signature"], "signature-" + platform)
                self.assertIn("/releases/download/v0.2.0/" + platform, data["url"])
            self.assertNotEqual(manifest["platforms"]["darwin-aarch64"]["url"], manifest["platforms"]["darwin-x86_64"]["url"])

    def test_missing_signature_never_generates_a_publishable_manifest(self):
        with tempfile.TemporaryDirectory() as temp:
            source = Path(temp) / "source"; source.mkdir(); self.fixtures(source, missing="windows-x86_64")
            output = Path(temp) / "out"
            with self.assertRaises(FileNotFoundError): release.collect(source, output, "0.2.0", "rxtsel/onlysend", "")
            self.assertFalse((output / "latest.json").exists())

    def test_missing_installer_never_generates_a_publishable_manifest(self):
        with tempfile.TemporaryDirectory() as temp:
            source = Path(temp) / "source"; source.mkdir(); self.fixtures(source)
            (source / "linux-x86_64-bundle/OnlySend.deb").unlink()
            output = Path(temp) / "out"
            with self.assertRaises(ValueError): release.collect(source, output, "0.2.0", "rxtsel/onlysend", "")
            self.assertFalse((output / "latest.json").exists())


class StagingTests(unittest.TestCase):
    fixtures = ArtifactTests.fixtures

    def test_staged_workflow_artifacts_feed_the_complete_release_manifest(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "builds"; source.mkdir(); self.fixtures(source)
            downloaded = root / "downloaded"
            for platform in release.PLATFORMS:
                release.stage(source / (platform + "-bundle"), downloaded / (platform + "-bundles"), platform)
            output = root / "release"
            release.collect(downloaded, output, "0.2.0", "rxtsel/onlysend", "Notes")
            manifest = json.loads((output / "latest.json").read_text())
            self.assertEqual(set(manifest["platforms"]), set(release.PLATFORMS))
            self.assertEqual(len(list((downloaded / "linux-x86_64-bundles").iterdir())), 3)
            self.assertEqual(len(list((downloaded / "windows-x86_64-bundles").iterdir())), 2)
            self.assertEqual(len(list((downloaded / "darwin-aarch64-bundles").iterdir())), 3)

    def test_stage_rejects_missing_empty_or_duplicate_bundles_before_upload(self):
        for failure in ("missing", "empty", "duplicate", "signature", "blank-signature"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temp:
                root = Path(temp); self.fixtures(root)
                source = root / "linux-x86_64-bundle"
                if failure == "missing": (source / "OnlySend.deb").unlink()
                elif failure == "empty": (source / "OnlySend.AppImage").write_bytes(b"")
                elif failure == "duplicate": (source / "Other.AppImage").write_bytes(b"duplicate")
                elif failure == "signature": (source / "OnlySend.AppImage.sig").unlink()
                else: (source / "OnlySend.AppImage.sig").write_text(" ")
                output = root / "upload"
                with self.assertRaises(ValueError): release.stage(source, output, "linux-x86_64")
                self.assertFalse(output.exists())

    def test_stage_does_not_include_other_files_or_updater_secrets(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); self.fixtures(root)
            source = root / "windows-x86_64-bundle"
            (source / "private.key").write_text("fixture only")
            (source / "OnlySend.msi").write_bytes(b"not selected")
            output = root / "upload"
            release.stage(source, output, "windows-x86_64")
            self.assertEqual({file.name for file in output.iterdir()}, {"OnlySend.exe", "OnlySend.exe.sig"})


class SignatureTests(unittest.TestCase):
    def test_verification_decodes_tauri_wrappers_and_uses_the_matching_asset(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            asset = output / "linux-x86_64-test.AppImage"
            asset.write_bytes(b"fixture")
            Path(str(asset) + ".sig").write_text(base64.b64encode(b"signature fixture").decode())
            key = base64.b64encode(b"public fixture").decode()
            def inspect(command, check):
                self.assertTrue(check)
                self.assertEqual(command[:3], ["minisign", "-V", "-q"])
                self.assertEqual(Path(command[command.index("-p") + 1]).read_bytes(), b"public fixture")
                self.assertEqual(Path(command[command.index("-x") + 1]).read_bytes(), b"signature fixture")
                self.assertEqual(command[-1], str(asset))
            with patch.object(release.subprocess, "run", side_effect=inspect) as run:
                release.verify(output, key)
                self.assertEqual(run.call_count, 1)
            with patch.object(release.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "minisign")):
                with self.assertRaises(subprocess.CalledProcessError): release.verify(output, key)

    def test_unsigned_assets_and_malformed_wrappers_cannot_pass_verification(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            with self.assertRaises(ValueError): release.verify(output, base64.b64encode(b"public").decode())
            with self.assertRaises(ValueError): release.verify(output, "not a base64 key")


if __name__ == "__main__": unittest.main()
