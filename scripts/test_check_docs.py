"""Boundary regressions for local documentation validation (no network)."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("check_docs", Path(__file__).with_name("check-docs.py"))
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class LinkBoundaries(unittest.TestCase):
    def check_text(self, text, expected):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "file with spaces.md").touch()
            document = root / "index.md"
            document.write_text(text)
            self.assertEqual(checker.broken_links(document, root), expected)

    def test_encoded_path_and_heading(self):
        self.check_text("[ok](file%20with%20spaces.md#heading)", [])

    def test_missing_file_is_reported(self):
        self.check_text("[bad](missing.md)", ["missing.md"])

    def test_external_and_fenced_examples_are_ignored(self):
        self.check_text("[web](https://example.invalid/x) [heading](#x)\n```md\n[x](missing.md)\n```", [])

    def test_outside_repository_is_rejected_even_if_it_exists(self):
        self.check_text("[outside](../)", ["../"])


if __name__ == "__main__":
    unittest.main()
