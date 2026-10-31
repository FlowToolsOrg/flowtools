import copy
import gzip
import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("verify_sources", Path(__file__).with_name("verify_sources.py"))
verify = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verify)


def archive(records):
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:") as package:
        for name, kind, body in records:
            entry = tarfile.TarInfo(name)
            entry.type = kind
            if kind == tarfile.REGTYPE:
                entry.size = len(body)
            else:
                entry.linkname = "../../outside"
            package.addfile(entry, io.BytesIO(body))
    return gzip.compress(output.getvalue())


class SourceVerification(unittest.TestCase):
    def test_exact_signed_git_object_hashes_and_changed_metadata(self):
        tag = verify.read_json(verify.HERE / "ffmpeg-tag.json")
        commit = verify.read_json(verify.HERE / "ffmpeg-commit.json")
        pin = verify.read_json(verify.HERE / "source-lock.json")["ffmpeg"]
        payload, signature = verify.git_objects(tag, commit, pin)
        self.assertTrue(payload.startswith(b"object " + pin["commit"].encode()))
        self.assertTrue(signature.startswith(b"-----BEGIN PGP SIGNATURE-----"))
        for target in ("tag", "commit"):
            changed_tag, changed_commit = copy.deepcopy(tag), copy.deepcopy(commit)
            document = changed_tag if target == "tag" else changed_commit
            document["verification"]["payload"] += "changed"
            with self.assertRaises(ValueError):
                verify.git_objects(changed_tag, changed_commit, pin)

    def test_duplicate_json_and_git_tree_ordering(self):
        with self.assertRaises(ValueError):
            verify.unique_object([("version", 1), ("version", 2)])
        self.assertEqual(verify.git_hash("blob", b"hello\n"), "ce013625030ba8dba906f756967f9e9ca394464a")
        self.assertEqual(verify.tree_hash([]), "4b825dc642cb6eb9a060e54bf8d69288fbee4904")
        entries = [(("a", "b"), b"body", False), (("a.c",), b"other", True)]
        self.assertEqual(verify.tree_hash(entries), verify.tree_hash(list(reversed(entries))))
        self.assertNotEqual(verify.tree_hash(entries), verify.tree_hash([(p, b, False) for p, b, _ in entries]))

    def test_hostile_archive_paths_links_duplicates_and_expansion(self):
        good = ("source/a", tarfile.REGTYPE, b"body")
        self.assertEqual(verify.read_archive(archive([good]), "source"), [(("a",), b"body", False)])
        for records in (
            [("source/../escape", tarfile.REGTYPE, b"body")],
            [("/source/a", tarfile.REGTYPE, b"body")],
            [("source/link", tarfile.SYMTYPE, b"")],
            [("source/link", tarfile.LNKTYPE, b"")],
            [good, good],
            [good, ("source/A", tarfile.REGTYPE, b"other")],
        ):
            with self.assertRaises(ValueError):
                verify.read_archive(archive(records), "source")
        with patch.object(verify, "MAX_EXPANDED", 64):
            with self.assertRaisesRegex(ValueError, "expanded"):
                verify.read_archive(gzip.compress(b"x" * 128), "source")

    def test_existing_source_changes_refuse_without_overwrite(self):
        entries = [(("part", "source.c"), b"source", False)]
        with tempfile.TemporaryDirectory(prefix="flowtools-source-check-") as temp:
            destination = Path(temp) / "source"
            verify.verify_or_extract(destination, entries, True)
            verify.verify_or_extract(destination, entries, False)
            source = destination / "part" / "source.c"
            source.write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "changed"):
                verify.verify_or_extract(destination, entries, True)
            self.assertEqual(source.read_bytes(), b"changed")
            source.write_bytes(b"source")
            (destination / "unknown").write_bytes(b"extra")
            with self.assertRaisesRegex(ValueError, "inventory"):
                verify.verify_or_extract(destination, entries, True)
            self.assertEqual((destination / "unknown").read_bytes(), b"extra")


def load_tests(loader, tests, pattern):
    verify.require(tests.countTestCases() == 4, "exactly four source regressions are required")
    return tests


if __name__ == "__main__":
    unittest.main()
