#!/usr/bin/env python3
"""Regression tests for the documentation checker."""
from pathlib import Path
import importlib.util
import unittest
from unittest import mock

SCRIPT = Path(__file__).with_name('check-documentation.py')
spec = importlib.util.spec_from_file_location('check_documentation', SCRIPT)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class RedactDependencyDescriptionTests(unittest.TestCase):
    def check_with(self, english, chinese, dependency, table=False):
        root = Path('/isolated/rs-fs')
        contents = {
            root / 'Cargo.toml': '',
            root / 'README.md': english,
            root / 'README.zh_CN.md': chinese,
        }
        contents[root / 'Cargo.toml'] = '[dependencies]\nqubit-redact = ' + (
            '{ version = "' + dependency + '", features = ["uri"] }' if table else '"' + dependency + '"'
        ) + '\n'

        def read_text(path, *args, **kwargs):
            return contents[path]

        with mock.patch.object(Path, 'read_text', autospec=True, side_effect=read_text) as read:
            checker.check_redact_dependency_description(root)
        read.assert_any_call(root / 'Cargo.toml', encoding='utf-8')

    def test_correct_bilingual_text_uses_core_manifest_series_string_form(self):
        self.check_with('`rs-redact` (`qubit-redact` 0.10 series)', '`rs-redact`（`qubit-redact` 0.10 系列）', '^0.10')

    def test_correct_bilingual_text_uses_core_manifest_series_table_form(self):
        self.check_with('`rs-redact` (`qubit-redact` 0.12 series)', '`rs-redact`（`qubit-redact` 0.12 系列）', '^0.12', table=True)

    def test_english_wrong_package_fails(self):
        with self.assertRaises(AssertionError):
            self.check_with('`rs-redact` (`qubit-fs` 0.10)', '`rs-redact`（`qubit-redact` 0.10 系列）', '^0.10')

    def test_chinese_wrong_package_fails(self):
        with self.assertRaises(AssertionError):
            self.check_with('`rs-redact` (`qubit-redact` 0.10 series)', '`rs-redact`（`qubit-fs` 0.10）', '^0.10')

    def test_english_wrong_version_fails(self):
        with self.assertRaises(AssertionError):
            self.check_with('`rs-redact` (`qubit-redact` 0.9 series)', '`rs-redact`（`qubit-redact` 0.10 系列）', '^0.10')

    def test_chinese_wrong_version_fails(self):
        with self.assertRaises(AssertionError):
            self.check_with('`rs-redact` (`qubit-redact` 0.10 series)', '`rs-redact`（`qubit-redact` 0.9 系列）', '^0.10')


if __name__ == '__main__':
    unittest.main()
