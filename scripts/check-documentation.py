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
    'managed-read': FIXTURE / 'managed_read.rs',
    'managed-read-async': FIXTURE / 'managed_read_async.rs',
}
DOCUMENTS = {
    'README.md': {'quick-start'},
    'README.zh_CN.md': {'quick-start'},
    'doc/user_guide.md': {'quick-start', 'sync-recovery', 'async-recovery', 'random-access-sampling', 'random-access-sampling-async', 'managed-read', 'managed-read-async'},
    'doc/user_guide.zh_CN.md': {'quick-start', 'sync-recovery', 'async-recovery', 'random-access-sampling', 'random-access-sampling-async', 'managed-read', 'managed-read-async'},
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
QUICK_START_DEPENDENCIES = '<!-- dependencies: quick-start -->'
QUICK_START_ASYNC_DEPENDENCIES = '<!-- dependencies: quick-start-async -->'
QUICK_START_RELEASE_STATUS = '<!-- release-status: quick-start -->'
STALE_CORE_VERSION = re.compile(r'qubit-fs\s*(?:=\s*(?:\{\s*version\s*=\s*)?)?["\']?\s*0\.6(?:\.0)?(?![0-9])')


def marked_fence(content, marker, language, location):
    """Return the one closed fence immediately following a unique marker."""
    count = content.count(marker)
    assert count == 1, f'{location}: expected exactly one {marker!r} marker, found {count}'
    lines = content.splitlines()
    marker_index = lines.index(marker)
    opening_index = marker_index + 1
    while opening_index < len(lines) and not lines[opening_index].strip():
        opening_index += 1
    opening = re.fullmatch(r'```([^`]*)', lines[opening_index].strip()) if opening_index < len(lines) else None
    assert opening, f'{location}: marker must be followed by a closed fenced block'
    assert opening.group(1).strip() == language, f'{location}: marker block must use a ```{language} fence'
    for closing_index in range(opening_index + 1, len(lines)):
        line = lines[closing_index].strip()
        if line == '```':
            return '\n'.join(lines[opening_index + 1:closing_index]).strip()
        if line.startswith('```'):
            break
    raise AssertionError(f'{location}: marker must be followed by a closed fenced block')


def check_marked_quick_start_dependencies(root=ROOT):
    """Keep source installation examples and their release status bilingual."""
    blocks = {}
    for relative in ('README.md', 'README.zh_CN.md', 'doc/user_guide.md', 'doc/user_guide.zh_CN.md'):
        content = (root / relative).read_text(encoding='utf-8')
        blocks[relative] = marked_fence(content, QUICK_START_DEPENDENCIES, 'toml', relative)
    baseline = blocks['README.md']
    for relative, block in blocks.items():
        assert block == baseline, f'{relative}: Quick Start dependency block differs from README.md'
    dependencies = tomllib.loads(baseline).get('dependencies', {})
    expected = {
        'qubit-fs': {'path': '../rs-fs'},
        'qubit-fs-local': {'path': '../rs-fs-local'},
        'tempfile': '3',
    }
    assert dependencies == expected, 'Quick Start dependency marker: use the required sibling source paths and tempfile'

    async_blocks = {}
    statuses = {}
    for relative in blocks:
        content = (root / relative).read_text(encoding='utf-8')
        async_blocks[relative] = marked_fence(content, QUICK_START_ASYNC_DEPENDENCIES, 'toml', relative)
        statuses[relative] = marked_fence(content, QUICK_START_RELEASE_STATUS, 'toml', relative)
    async_baseline = async_blocks['README.md']
    for relative, block in async_blocks.items():
        assert block == async_baseline, f'{relative}: async dependency block differs from README.md'
    async_dependencies = tomllib.loads(async_baseline).get('dependencies', {})
    assert async_dependencies == {
        'qubit-fs': {'path': '../rs-fs', 'features': ['async']},
    }, 'Quick Start async dependency marker: enable async on the same core source path'
    status_baseline = statuses['README.md']
    for relative, block in statuses.items():
        assert block == status_baseline, f'{relative}: release status differs from README.md'
    status = tomllib.loads(status_baseline).get('release', {})
    assert status == {'qubit-fs': 'not-published', 'checked': '2026-11-24'}, (
        'Quick Start release status marker: update the public-index status and check date'
    )
    for relative in blocks:
        content = (root / relative).read_text(encoding='utf-8').lower()
        assert 'src/main.rs' in content, f'{relative}: installation must name src/main.rs'
        assert 'cargo run --manifest-path ../report-app/cargo.toml' in content, (
            f'{relative}: installation must give the standalone caller run command'
        )
        assert 'report ready' in content, f'{relative}: installation must state the expected output'
    return baseline


def required_sibling_path_dependencies(root=ROOT):
    """Return sibling repositories required by core and documentation manifests."""
    required = set()
    pending = [root / 'Cargo.toml', root / 'tests/fixtures/documentation_examples/Cargo.toml']
    visited = set()
    while pending:
        manifest_path = pending.pop().resolve()
        if manifest_path in visited or not manifest_path.is_file():
            continue
        visited.add(manifest_path)
        manifest = tomllib.loads(manifest_path.read_text(encoding='utf-8'))
        for section in ('dependencies', 'dev-dependencies', 'build-dependencies'):
            for dependency in manifest.get(section, {}).values():
                if isinstance(dependency, dict) and 'path' in dependency:
                    dependency_path = (manifest_path.parent / dependency['path']).resolve()
                    if dependency_path == root.resolve():
                        continue
                    if dependency_path.parent == root.resolve().parent and dependency_path != root.resolve():
                        required.add(dependency_path.name)
                    dependency_manifest = dependency_path / 'Cargo.toml'
                    if dependency_manifest.is_file() and dependency_path.parent == root.resolve().parent:
                        pending.append(dependency_manifest)
        for patch_table in manifest.get('patch', {}).values():
            for dependency in patch_table.values():
                if isinstance(dependency, dict) and 'path' in dependency:
                    dependency_path = (manifest_path.parent / dependency['path']).resolve()
                    if dependency_path == root.resolve():
                        continue
                    if dependency_path.parent == root.resolve().parent:
                        required.add(dependency_path.name)
                        dependency_manifest = dependency_path / 'Cargo.toml'
                        if dependency_manifest.is_file():
                            pending.append(dependency_manifest)
    return required


def check_source_layout_dependencies(root=ROOT):
    """Keep documented sibling checkouts and CI checkouts aligned with path dependencies."""
    required = required_sibling_path_dependencies(root)
    for relative in ('README.md', 'README.zh_CN.md'):
        content = (root / relative).read_text(encoding='utf-8')
        match = re.search(r'<!-- source-layout -->\n(.*?)(?=\n```)', content, re.S)
        assert match, f'{relative}: missing marked source-layout tree'
        documented = set(re.findall(r'├── ([A-Za-z0-9_-]+)/', match.group(1)))
        documented.update(re.findall(r'└── ([A-Za-z0-9_-]+)/', match.group(1)))
        missing = required - documented
        assert not missing, f'{relative}: source layout is missing sibling repositories: {", ".join(sorted(missing))}'
    workflow = (root / '.github/workflows/documentation-examples.yml').read_text(encoding='utf-8')
    checked_out = set(re.findall(r'path:\s*rust-common/([A-Za-z0-9_-]+)', workflow))
    missing = required - checked_out
    assert not missing, f'.github/workflows/documentation-examples.yml: missing sibling checkouts: {", ".join(sorted(missing))}'


def markdown_documents():
    """Return every Markdown document shipped by this crate."""
    return sorted(path for path in (ROOT / 'doc').glob('*.md') if not path.name.startswith(('design-', 'implementation-plan-'))) + [ROOT / 'README.md', ROOT / 'README.zh_CN.md']


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
    for path in markdown_documents():
        content = path.read_text()
        relative = path.relative_to(ROOT)
        assert not STALE_CORE_VERSION.search(content), f'{relative}: contains stale qubit-fs 0.6 version text'
        assert major_minor in content, f'{relative}: missing current package version {major_minor}'
    quick_start_dependencies = check_marked_quick_start_dependencies(ROOT)
    parsed_quick_start_dependencies = tomllib.loads(quick_start_dependencies)
    quick_start_table = parsed_quick_start_dependencies.get('dependencies', {})
    assert quick_start_table.get('qubit-fs', {}).get('path') == '../rs-fs', 'Quick Start dependency marker: core source path is incorrect'
    assert quick_start_table.get('qubit-fs-local', {}).get('path') == '../rs-fs-local', 'Quick Start dependency marker: local source path is incorrect'
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


def check_package_scope(files):
    """Reject internal planning and design records in the published package."""
    excluded = [
        name for name in files
        if Path(name).match('doc/design-*.md')
        or Path(name).match('doc/implementation-plan-*.md')
    ]
    assert not excluded, f'internal documents included in package: {", ".join(sorted(excluded))}'


def check_package_links():
    """Require local destinations and their Markdown fragments in the package."""
    manifest_root = Path(os.environ['QUBIT_FS_SIBLING_ROOT']) / 'rs-fs' if 'QUBIT_FS_SIBLING_ROOT' in os.environ else ROOT
    result = subprocess.run(['cargo', 'package', '--list', '--allow-dirty', '--locked', '--manifest-path', str(manifest_root / 'Cargo.toml')], capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    files = set(result.stdout.splitlines())
    check_package_scope(files)
    assert not any('target' in Path(name).parts for name in files), 'package includes build output'
    documents = [str(path.relative_to(ROOT)) for path in markdown_documents()]
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
    check_source_layout_dependencies()
    check_examples()
    check_package_links()
    print('Bilingual examples match compiled sources; package links resolve.')
