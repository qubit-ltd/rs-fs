#!/usr/bin/env python3
"""Verify bilingual examples and package-relative Markdown links."""
from pathlib import Path
import re
import os
import subprocess
import tomllib
import unicodedata
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/fixtures/documentation_examples/src'
SOURCES = {
    'quick-start': FIXTURE / 'bin/quick_start.rs',
    'sync-recovery': FIXTURE / 'sync_recovery.rs',
    'async-recovery': FIXTURE / 'async_recovery.rs',
    'provider-minimal': FIXTURE / 'provider_minimal.rs',
    'provider-minimal-async': FIXTURE / 'provider_minimal_async.rs',
    'random-access-sampling': FIXTURE / 'random_access_sampling.rs',
    'random-access-sampling-async': FIXTURE / 'random_access_sampling_async.rs',
}
DOCUMENTS = {
    'README.md': {'quick-start'},
    'README.zh_CN.md': {'quick-start'},
    'doc/user_guide.md': {'quick-start', 'sync-recovery', 'async-recovery', 'random-access-sampling', 'random-access-sampling-async'},
    'doc/user_guide.zh_CN.md': {'quick-start', 'sync-recovery', 'async-recovery', 'random-access-sampling', 'random-access-sampling-async'},
    'doc/provider_guide.md': {'provider-minimal', 'provider-minimal-async'},
    'doc/provider_guide.zh_CN.md': {'provider-minimal', 'provider-minimal-async'},
}
PATTERN = re.compile(r'<!-- example: ([a-z-]+) -->\n```rust\n(.*?)\n```', re.S)
RUST_FILE_HEADER = re.compile(
    r'^// =============================================================================\n'
    r'//    Copyright \(c\) [0-9]{4}(?: - [0-9]{4})? Haixing Hu\.\n'
    r'//\n'
    r'//    SPDX-License-Identifier: Apache-2\.0\n'
    r'//\n'
    r'//    Licensed under the Apache License, Version 2\.0\.\n'
    r'// =============================================================================\n\n'
)


def markdown_documents():
    """Return every Markdown document shipped by this crate."""
    return sorted((ROOT / 'doc').glob('*.md')) + [ROOT / 'README.md', ROOT / 'README.zh_CN.md']


def check_redact_dependency_description(root=ROOT):
    """Require both READMEs to name rs-redact's core dependency series."""
    manifest = tomllib.loads((root / 'Cargo.toml').read_text(encoding='utf-8'))
    requirement = manifest['dependencies']['qubit-redact']
    if isinstance(requirement, dict):
        requirement = requirement['version']
    version = re.search(r'(?<![0-9])([0-9]+)\.([0-9]+)', requirement)
    assert version, f'Cargo.toml: cannot derive qubit-redact version series from {requirement!r}'
    series = f'{version.group(1)}\\.{version.group(2)}'
    description = re.compile(r'rs-redact.{0,100}qubit-redact.{0,40}' + series + r'(?![0-9])', re.S)
    for relative in ('README.md', 'README.zh_CN.md'):
        content = (root / relative).read_text(encoding='utf-8')
        assert description.search(content), (
            f'{relative}: rs-redact must be described as the qubit-redact {version.group(1)}.'
            f'{version.group(2)} series'
        )


def check_versions_and_structure():
    """Keep shipped docs on the manifest version and stable navigation shape."""
    check_redact_dependency_description()
    manifest = tomllib.loads((ROOT / 'Cargo.toml').read_text())
    version = manifest['package']['version']
    major_minor = '.'.join(version.split('.')[:2])
    fixture_manifest = tomllib.loads((ROOT / 'tests/fixtures/documentation_examples/Cargo.toml').read_text())
    local_version = fixture_manifest['dependencies']['qubit-fs-local']['version']
    stale = re.compile(r'(?<![0-9])0\.6(?:\.0)?(?![0-9])')
    for path in markdown_documents():
        content = path.read_text()
        relative = path.relative_to(ROOT)
        assert not stale.search(content), f'{relative}: contains stale 0.6 version text'
        assert major_minor in content, f'{relative}: missing current package version {major_minor}'
    install_headings = {
        'README.md': 'Quick Start',
        'README.zh_CN.md': '快速开始',
        'doc/user_guide.md': 'Installation and Minimal Configuration',
        'doc/user_guide.zh_CN.md': '安装与最小配置',
    }
    for relative, heading in install_headings.items():
        content = (ROOT / relative).read_text()
        section = content.split(f'## {heading}\n', 1)[1].split('\n## ', 1)[0]
        match = re.search(r'```toml\n(.*?)\n```', section, re.S)
        assert match, f'{relative}: installation dependency block is missing'
        block = match.group(1)
        assert block.count(f'qubit-fs = "{major_minor}"') == 1, f'{relative}: core version is stale'
        assert block.count(f'qubit-fs-local = "{local_version}"') == 1, f'{relative}: local version is stale'
    for relative, heading in (
        ('doc/provider_guide.md', '## 15. Further reading'),
        ('doc/provider_guide.zh_CN.md', '## 15. 延伸阅读'),
        ('doc/user_guide.md', '## Further reading'),
        ('doc/user_guide.zh_CN.md', '## 延伸阅读'),
    ):
        headings = [line for line in (ROOT / relative).read_text().splitlines() if line.startswith('## ')]
        assert headings and headings[-1] == heading, f'{relative}: further-reading section must be last'


def check_examples():
    """Compare exact code text, allowing only the final file newline."""
    for relative, expected in DOCUMENTS.items():
        content = (ROOT / relative).read_text()
        matches = PATTERN.findall(content)
        assert {name for name, _ in matches} == expected, f'{relative}: missing or unexpected example IDs'
        assert len(matches) == len(expected), f'{relative}: duplicated example IDs'
        for name, code in matches:
            source = RUST_FILE_HEADER.sub('', SOURCES[name].read_text(), count=1)
            assert code + '\n' == source, f'{relative}: {name} differs from compiled fixture'


def markdown_anchors(text: str) -> set[str]:
    """Collect heading slugs and explicit HTML id/name anchors from Markdown."""
    anchors = set()
    automatic = set()
    duplicate_counts = {}
    lines = text.splitlines()
    in_fence = False
    fence_char = ''
    fence_size = 0

    def add_heading(value):
        value = re.sub(r'<[^>]*>', '', value)
        value = re.sub(r'!?\[([^]]*)\]\([^)]*\)', r'\1', value)
        value = re.sub(r'!?\[([^]]*)\]\[[^]]*\]', r'\1', value)
        value = re.sub(r'[`*_~]', '', value)
        slug_chars = []
        for char in value.lower().strip():
            category = unicodedata.category(char)
            if char.isspace():
                slug_chars.append('-')
            elif category[0] in ('L', 'N'):
                slug_chars.append(char)
        base = ''.join(slug_chars)
        if not base:
            return
        suffix = duplicate_counts.get(base, 0)
        slug = base if suffix == 0 else f'{base}-{suffix}'
        while slug in automatic:
            suffix += 1
            slug = f'{base}-{suffix}'
        duplicate_counts[base] = suffix + 1
        automatic.add(slug)
        anchors.add(slug)

    previous = None
    for line in lines:
        fence = re.match(r'^ {0,3}(`{3,}|~{3,})', line)
        if fence:
            marker = fence.group(1)
            if not in_fence:
                in_fence = True
                fence_char, fence_size = marker[0], len(marker)
            elif marker[0] == fence_char and len(marker) >= fence_size and not line.strip().strip(marker[0]).strip():
                in_fence = False
            previous = None
            continue
        if in_fence:
            continue

        for match in re.finditer(r'\bid\s*=\s*(["\'])(.*?)\1|\bname\s*=\s*(["\'])(.*?)\3', line, re.I):
            anchors.add(match.group(2) if match.group(2) is not None else match.group(4))

        atx = re.match(r'^ {0,3}#{1,6}(?:[ \t]+|$)(.*?)[ \t]*#*[ \t]*$', line)
        if atx:
            add_heading(atx.group(1))
            previous = None
            continue
        setext = re.match(r'^ {0,3}(=+|-+)[ \t]*$', line)
        if setext and previous is not None and previous.strip():
            add_heading(previous.strip())
            previous = None
            continue
        previous = line
    return anchors


def check_markdown_fragment(text: str, fragment: str, location: str) -> None:
    """Fail with link context when a Markdown fragment has no matching anchor."""
    if fragment and fragment not in markdown_anchors(text):
        raise AssertionError(f'{location}: missing Markdown fragment #{fragment}')


def check_package_links():
    """Require local destinations and their Markdown fragments in the package."""
    manifest_root = Path(os.environ['QUBIT_FS_SIBLING_ROOT']) / 'rs-fs' if 'QUBIT_FS_SIBLING_ROOT' in os.environ else ROOT
    result = subprocess.run(['cargo', 'package', '--list', '--allow-dirty', '--locked', '--manifest-path', str(manifest_root / 'Cargo.toml')], capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    files = set(result.stdout.splitlines())
    assert not any('target' in Path(name).parts for name in files), 'package includes build output'
    documents = [*DOCUMENTS, *[str(p.relative_to(ROOT)) for p in (ROOT / 'doc').glob('*.md')]]
    for relative in set(documents):
        assert relative in files, f'{relative}: missing from package'
        content = (ROOT / relative).read_text()
        for destination in re.findall(r'\]\(([^)\s]+)\)', content):
            if re.match(r'[a-zA-Z][a-zA-Z0-9+.-]*:', destination):
                continue
            target, separator, fragment = destination.partition('#')
            fragment = unquote(fragment) if separator else ''
            if not target:
                target = relative
            resolved = (ROOT / relative).parent.joinpath(target).resolve()
            assert resolved.is_relative_to(ROOT), f'{relative}: link escapes package: {target}'
            package_path = str(resolved.relative_to(ROOT))
            assert package_path in files, f'{relative}: link not packaged: {target}'
            if separator and fragment and resolved.suffix.lower() == '.md':
                target_content = resolved.read_text(encoding='utf-8')
                check_markdown_fragment(target_content, fragment, f'{relative}: {destination} -> {package_path}')


if __name__ == '__main__':
    check_versions_and_structure()
    check_examples()
    check_package_links()
    print('Bilingual examples match compiled sources; package links resolve.')
