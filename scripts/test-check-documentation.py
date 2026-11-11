#!/usr/bin/env python3
"""Regression tests for the documentation checker."""
from pathlib import Path
import importlib.util
import os
import tempfile
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


class MarkdownAnchorTests(unittest.TestCase):
    def test_chinese_and_percent_encoded_fragments(self):
        anchors = checker.markdown_anchors('## 元数据保留\n')
        self.assertIn('元数据保留', anchors)
        checker.check_markdown_fragment('## 元数据保留\n', '元数据保留', 'README.md -> guide.md#%E5%85%83%E6%95%B0%E6%8D%AE%E4%BF%9D%E7%95%99')

    def test_duplicate_headings_and_natural_suffix_collision(self):
        anchors = checker.markdown_anchors('## Read\n## Read\n## Read 1\n')
        self.assertTrue({'read', 'read-1', 'read-1-1'} <= anchors)

    def test_explicit_id_and_legacy_name_anchors(self):
        anchors = checker.markdown_anchors('<div id="custom-id">\n<a name="legacy-name"></a>\n')
        self.assertTrue({'custom-id', 'legacy-name'} <= anchors)

    def test_headings_support_atx_setext_formatting_and_punctuation(self):
        anchors = checker.markdown_anchors('## `Read` **This!**\n\nSetext & More\n---\n')
        self.assertTrue({'read-this', 'setext--more'} <= anchors)

    def test_fenced_code_headings_are_ignored(self):
        anchors = checker.markdown_anchors('```md\n## Fake heading\n```\n## Real heading\n')
        self.assertNotIn('fake-heading', anchors)
        self.assertIn('real-heading', anchors)

    def test_missing_fragment_reports_location_and_fragment(self):
        with self.assertRaisesRegex(AssertionError, r'guide\.md#absent.*absent'):
            checker.check_markdown_fragment('## Present\n', 'absent', 'README.md -> guide.md#absent')

    def test_package_links_check_same_file_and_cross_file_fragments(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Cargo.toml').write_text('[package]\nname="fixture"\nversion="1.0.0"\n', encoding='utf-8')
            (root / 'README.md').write_text('[self](#here) [other](guide.md#%E7%9B%AE%E6%A0%87) [empty](#) [external](https://example.com/#x) [no fragment](guide.md)\n## Here\n', encoding='utf-8')
            (root / 'guide.md').write_text('目标\n---\n', encoding='utf-8')
            files = '\n'.join(['Cargo.toml', 'README.md', 'README.zh_CN.md', 'doc/user_guide.md', 'doc/user_guide.zh_CN.md', 'doc/provider_guide.md', 'doc/provider_guide.zh_CN.md', 'guide.md'])
            def run(*args, **kwargs):
                return type('Result', (), {'returncode': 0, 'stdout': files, 'stderr': ''})()
            with mock.patch.dict(os.environ, {'QUBIT_FS_SIBLING_ROOT': str(root.parent)}), \
                    mock.patch.object(checker.subprocess, 'run', side_effect=run), \
                    mock.patch.object(checker, 'DOCUMENTS', {'README.md': set()}), \
                    mock.patch.object(checker, 'ROOT', root), \
                    mock.patch.object(checker, 'markdown_documents', return_value=[root / 'README.md']):
                checker.check_package_links()

    def test_existing_file_with_missing_fragment_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Cargo.toml').write_text('[package]\nname="fixture"\nversion="1.0.0"\n', encoding='utf-8')
            (root / 'README.md').write_text('[bad](guide.md#missing)\n', encoding='utf-8')
            (root / 'guide.md').write_text('## Present\n', encoding='utf-8')
            files = '\n'.join(['Cargo.toml', 'README.md', 'guide.md'])
            def run(*args, **kwargs):
                return type('Result', (), {'returncode': 0, 'stdout': files, 'stderr': ''})()
            with mock.patch.dict(os.environ, {'QUBIT_FS_SIBLING_ROOT': str(root.parent)}), \
                    mock.patch.object(checker.subprocess, 'run', side_effect=run), \
                    mock.patch.object(checker, 'DOCUMENTS', {'README.md': set()}), \
                    mock.patch.object(checker, 'ROOT', root), \
                    mock.patch.object(checker, 'markdown_documents', return_value=[root / 'README.md']):
                with self.assertRaisesRegex(AssertionError, r'README\.md.*guide\.md#missing'):
                    checker.check_package_links()

    def test_package_links_reject_paths_outside_package(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'rs-fs'
            root.mkdir()
            (root / 'Cargo.toml').write_text('[package]\nname="fixture"\nversion="1.0.0"\n', encoding='utf-8')
            (root / 'README.md').write_text('[escape](../outside.md#x)\n', encoding='utf-8')
            files = 'Cargo.toml\nREADME.md'
            def run(*args, **kwargs):
                return type('Result', (), {'returncode': 0, 'stdout': files, 'stderr': ''})()
            with mock.patch.dict(os.environ, {'QUBIT_FS_SIBLING_ROOT': str(root.parent)}), \
                    mock.patch.object(checker.subprocess, 'run', side_effect=run), \
                    mock.patch.object(checker, 'DOCUMENTS', {'README.md': set()}), \
                    mock.patch.object(checker, 'ROOT', root), \
                    mock.patch.object(checker, 'markdown_documents', return_value=[root / 'README.md']):
                with self.assertRaisesRegex(AssertionError, 'escapes package'):
                    checker.check_package_links()


if __name__ == '__main__':
    unittest.main()
