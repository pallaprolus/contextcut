import hashlib
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile

import release


class ReleasePackagingTests(unittest.TestCase):
    def test_archives_contain_only_distribution_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "built-binary"
            binary.write_bytes(b"standalone executable")
            binary.chmod(0o755)
            for target in release.TARGETS:
                release.package(binary, target, root)
                archive = root / release.filename(target)
                if "windows" in target:
                    with zipfile.ZipFile(archive) as contents:
                        self.assertEqual(set(contents.namelist()), {"contextcut.exe", "LICENSE", "README.md"})
                        self.assertEqual(contents.read("contextcut.exe"), binary.read_bytes())
                else:
                    with tarfile.open(archive) as contents:
                        self.assertEqual(set(contents.getnames()), {"contextcut", "LICENSE", "README.md"})
                        self.assertTrue(contents.getmember("contextcut").mode & 0o111)
                        self.assertEqual(contents.extractfile("contextcut").read(), binary.read_bytes())
            release.manifest(root)
            for line in (root / "SHA256SUMS").read_text().splitlines():
                expected, name = line.split("  ", 1)
                self.assertEqual(expected, hashlib.sha256((root / name).read_bytes()).hexdigest())
            formula = (root / "contextcut.rb").read_text()
            for target in release.TARGETS:
                if "windows" not in target:
                    self.assertIn(release.filename(target), formula)
                    self.assertIn(release.digest(root / release.filename(target)), formula)

    def test_manifest_refuses_partial_releases(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(ValueError, "missing release archive"):
                release.manifest(root)
            self.assertFalse((root / "SHA256SUMS").exists())
            self.assertFalse((root / "contextcut.rb").exists())

    def test_smoke_rejects_unexpected_archive_members_before_extracting(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "unexpected.zip"
            with zipfile.ZipFile(archive, "w") as contents:
                contents.writestr("../unexpected.txt", "not a distribution file")
            with self.assertRaises(AssertionError):
                release.smoke(archive)


if __name__ == "__main__":
    unittest.main()
