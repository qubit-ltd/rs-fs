#!/usr/bin/env python3
"""Verify bilingual examples and package-relative Markdown links."""
from pathlib import Path
import re
import os
import subprocess

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/fixtures/documentation_examples/src'
SOURCES = {
    'quick-start': FIXTURE / 'bin/quick_start.rs',
    'sync-recovery': FIXTURE / 'sync_recovery.rs',
    'async-recovery': FIXTURE / 'async_recovery.rs',
    'provider-minimal': FIXTURE / 'provider_minimal.rs',
}
DOCUMENTS = {
    'README.md': {'quick-start'},
    'README.zh_CN.md': {'quick-start'},
    'doc/user_guide.md': {'quick-start', 'sync-recovery', 'async-recovery'},
    'doc/user_guide.zh_CN.md': {'quick-start', 'sync-recovery', 'async-recovery'},
    'doc/provider_guide.md': {'provider-minimal'},
    'doc/provider_guide.zh_CN.md': {'provider-minimal'},
}
PATTERN = re.compile(r'<!-- example: ([a-z-]+) -->\n```rust\n(.*?)\n```', re.S)


def check_examples():
    """Compare exact code text, allowing only the final file newline."""
    for relative, expected in DOCUMENTS.items():
        content = (ROOT / relative).read_text()
        matches = PATTERN.findall(content)
        assert {name for name, _ in matches} == expected, f'{relative}: missing or unexpected example IDs'
        assert len(matches) == len(expected), f'{relative}: duplicated example IDs'
        for name, code in matches:
            assert code + '\n' == SOURCES[name].read_text(), f'{relative}: {name} differs from compiled fixture'


def check_package_links():
    """Require every local Markdown destination to exist in the source package."""
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
            if re.match(r'[a-zA-Z][a-zA-Z0-9+.-]*:', destination) or destination.startswith('#'):
                continue
            target = destination.split('#', 1)[0]
            resolved = (ROOT / relative).parent.joinpath(target).resolve()
            assert resolved.is_relative_to(ROOT), f'{relative}: link escapes package: {target}'
            package_path = str(resolved.relative_to(ROOT))
            assert package_path in files, f'{relative}: link not packaged: {target}'


if __name__ == '__main__':
    check_examples()
    check_package_links()
    print('Bilingual examples match compiled sources; package links resolve.')
