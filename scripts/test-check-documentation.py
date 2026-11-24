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


class DocumentScopeTests(unittest.TestCase):
    def test_design_drafts_are_not_release_documentation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'doc').mkdir()
            for name in ('user_guide.md', 'design-0.4.0.zh_CN.md', 'implementation-plan-20261118.md'):
                (root / 'doc' / name).touch()
            with mock.patch.object(checker, 'ROOT', root):
                documents = checker.markdown_documents()
            self.assertIn(root / 'doc/user_guide.md', documents)
            self.assertNotIn(root / 'doc/design-0.4.0.zh_CN.md', documents)
            self.assertNotIn(root / 'doc/implementation-plan-20261118.md', documents)

    def test_package_manifest_must_exclude_internal_records(self):
        checker.check_package_scope({'README.md', 'doc/user_guide.md'})
        for internal in (
            'doc/design-unique-regression.md',
            'doc/implementation-plan-unique-regression.md',
        ):
            with self.subTest(internal=internal), self.assertRaisesRegex(AssertionError, internal):
                checker.check_package_scope({'README.md', internal})


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


class QuickStartDependencyMarkerTests(unittest.TestCase):
    def test_real_bilingual_readmes_and_guides_have_matching_marked_dependencies(self):
        dependencies = checker.check_marked_quick_start_dependencies()
        self.assertIn('qubit-fs = { version = "0.4", path = "../rs-fs" }', dependencies)

    def test_marker_requires_one_closed_correct_fence(self):
        marker = checker.QUICK_START_DEPENDENCIES
        valid = marker + '\n```toml\n[dependencies]\nqubit-fs = { path = "../rs-fs" }\n```'
        self.assertIn('path = "../rs-fs"', checker.marked_fence(valid, marker, 'toml', 'fixture'))
        for invalid in (
            '```toml\n[dependencies]\n```',
            valid + '\n' + valid,
            marker + '\n```text\nvalue\n```',
            marker + '\n```toml\n[dependencies]',
        ):
            with self.subTest(invalid=invalid), self.assertRaises(AssertionError):
                checker.marked_fence(invalid, marker, 'toml', 'fixture')

    def test_marked_dependencies_require_source_paths_and_match_all_documents(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'doc').mkdir()
            valid = '''<!-- dependencies: quick-start -->
```toml
[dependencies]
qubit-fs = { version = "0.4", path = "../rs-fs" }
qubit-fs-local = { path = "../rs-fs-local" }
tempfile = "3"
```

<!-- dependencies: quick-start-async -->
```toml
[dependencies]
qubit-fs = { version = "0.4", path = "../rs-fs", features = ["async"] }
```

<!-- release-status: quick-start -->
```toml
[release]
core_status = "not-published"
checked = "2026-11-24"
```
Create a sibling caller, copy the program to `src/main.rs`, then run
`cargo run --manifest-path ../report-app/Cargo.toml`; it prints `report ready`.
'''
            for relative in ('README.md', 'README.zh_CN.md', 'doc/user_guide.md', 'doc/user_guide.zh_CN.md'):
                (root / relative).write_text(valid, encoding='utf-8')
            self.assertIn('path = "../rs-fs"', checker.check_marked_quick_start_dependencies(root))
            wrong_path = valid.replace('../rs-fs-local', '../rs-local-files')
            (root / 'README.md').write_text(wrong_path, encoding='utf-8')
            with self.assertRaisesRegex(AssertionError, 'differs from README'):
                checker.check_marked_quick_start_dependencies(root)
            registry = valid.replace('qubit-fs = { version = "0.4", path = "../rs-fs" }', 'qubit-fs = "0.4"')
            for relative in ('README.md', 'README.zh_CN.md', 'doc/user_guide.md', 'doc/user_guide.zh_CN.md'):
                (root / relative).write_text(registry, encoding='utf-8')
            with self.assertRaisesRegex(AssertionError, 'required sibling source paths'):
                checker.check_marked_quick_start_dependencies(root)
            wrong_sibling = valid.replace('../rs-fs-local', '../rs-local-files')
            for relative in ('README.md', 'README.zh_CN.md', 'doc/user_guide.md', 'doc/user_guide.zh_CN.md'):
                (root / relative).write_text(wrong_sibling, encoding='utf-8')
            with self.assertRaisesRegex(AssertionError, 'required sibling source paths'):
                checker.check_marked_quick_start_dependencies(root)
            (root / 'README.md').write_text(valid, encoding='utf-8')
            (root / 'doc/user_guide.zh_CN.md').write_text(valid.replace('checked = "2026-11-24"', 'checked = "2026-11-23"'), encoding='utf-8')
            with self.assertRaisesRegex(AssertionError, 'differs from README'):
                checker.check_marked_quick_start_dependencies(root)


class CurrentPackageVersionTests(unittest.TestCase):
    def test_local_files_06_is_not_mistaken_for_a_stale_core_version(self):
        self.assertIsNone(checker.STALE_CORE_VERSION.search('qubit-local-files = "0.6.0"'))
        self.assertIsNotNone(checker.STALE_CORE_VERSION.search('qubit-fs = "0.6"'))
        self.assertIsNotNone(checker.STALE_CORE_VERSION.search('qubit-fs = { version = "0.6.0" }'))


class SourceLayoutDependencyTests(unittest.TestCase):
    def make_project(self, root):
        (root / 'tests/fixtures/documentation_examples').mkdir(parents=True)
        (root / '.github/workflows').mkdir(parents=True)
        (root / 'Cargo.toml').write_text('[dependencies]\nqubit-operation = "^0.3.1"\n', encoding='utf-8')
        fixture = root / 'tests/fixtures/documentation_examples/Cargo.toml'
        fixture.write_text(
            '[dependencies]\nqubit-fs = { path = "../../../" }\n'
            'qubit-operation = "^0.3.1"\n'
            '[patch.crates-io]\nqubit-fs-local = { path = "../../../../rs-fs-local" }\n'
            'qubit-local-files = { path = "../../../../rs-local-files" }\n', encoding='utf-8')
        (root.parent / 'rs-fs-local').mkdir()
        (root.parent / 'rs-fs-local/Cargo.toml').write_text(
            '[dependencies]\nqubit-fs-registry = { path = "../rs-fs-registry" }\n'
            '[dev-dependencies]\nqubit-fs-testkit = { path = "../rs-fs-testkit" }\n', encoding='utf-8')
        for name in ('rs-fs-registry', 'rs-fs-testkit', 'rs-local-files'):
            (root.parent / name).mkdir(exist_ok=True)
            (root.parent / name / 'Cargo.toml').write_text('[package]\nname = "fixture"\n', encoding='utf-8')
        names = ('rs-fs', 'rs-fs-local', 'rs-fs-registry', 'rs-fs-testkit', 'rs-local-files')
        tree = '<!-- source-layout -->\n```text\n' + '\n'.join(f'├── {name}/' for name in names) + '\n```\n'
        for relative in ('README.md', 'README.zh_CN.md'):
            (root / relative).write_text(tree, encoding='utf-8')
        workflow = '\n'.join(f'path: rust-common/{name}' for name in names)
        (root / '.github/workflows/documentation-examples.yml').write_text(workflow, encoding='utf-8')

    def test_complete_bilingual_layout_and_workflow_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'rs-fs'
            root.mkdir()
            self.make_project(root)
            self.assertEqual(
                {'rs-fs-local', 'rs-local-files', 'rs-fs-registry', 'rs-fs-testkit'},
                checker.required_sibling_path_dependencies(root),
            )
            checker.check_source_layout_dependencies(root)

    def test_each_language_must_list_every_required_sibling(self):
        for relative in ('README.md', 'README.zh_CN.md'):
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as directory:
                root = Path(directory) / 'rs-fs'
                root.mkdir()
                self.make_project(root)
                (root / relative).write_text('<!-- source-layout -->\n```text\n├── rs-fs/\n```\n', encoding='utf-8')
                with self.assertRaisesRegex(AssertionError, relative):
                    checker.check_source_layout_dependencies(root)

    def test_new_sibling_path_dependency_must_be_documented(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'rs-fs'
            root.mkdir()
            self.make_project(root)
            (root / 'Cargo.toml').write_text(
                '[dependencies]\nqubit-operation = "^0.3.1"\nnew-provider = { path = "../rs-new-provider" }\n',
                encoding='utf-8')
            with self.assertRaisesRegex(AssertionError, 'rs-new-provider'):
                checker.check_source_layout_dependencies(root)

    def test_project_internal_paths_are_not_reported_as_sibling_repositories(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'rs-fs'
            root.mkdir()
            self.make_project(root)
            (root / 'Cargo.toml').write_text('[dependencies]\nself = { path = "tests/fixtures/documentation_examples" }\n', encoding='utf-8')
            self.assertNotIn('documentation_examples', checker.required_sibling_path_dependencies(root))


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
