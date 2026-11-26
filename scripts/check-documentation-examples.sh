#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$project_root"

python3 scripts/check-documentation.py
cargo test --locked --manifest-path tests/fixtures/documentation_examples/Cargo.toml
cargo run --locked --manifest-path tests/fixtures/documentation_examples/Cargo.toml --bin quick_start

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/rust-common/report-app/src"
ln -s "$project_root" "$scratch/rust-common/rs-fs"
for sibling in rs-fs-local rs-fs-registry rs-fs-testkit rs-local-files; do
    if [[ -d "$project_root/../$sibling" ]]; then
        ln -s "$project_root/../$sibling" "$scratch/rust-common/$sibling"
    fi
done
python3 - "$project_root" "$scratch/rust-common/report-app" <<'PY'
from pathlib import Path
import sys
sys.path.insert(0, str(Path(sys.argv[1]) / 'scripts'))
import importlib.util
spec = importlib.util.spec_from_file_location('checker', Path(sys.argv[1]) / 'scripts/check-documentation.py')
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)
root = Path(sys.argv[1])
app = Path(sys.argv[2])
readme = (root / 'README.md').read_text(encoding='utf-8')
dependencies = checker.marked_fence(readme, checker.QUICK_START_DEPENDENCIES, 'toml', 'README.md')
(app / 'Cargo.toml').write_text('[package]\nname = \"qubit-fs-quick-start-consumer\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n\n' + dependencies + '\n', encoding='utf-8')
match = checker.PATTERN.search(readme)
assert match and match.group(1) == 'quick-start'
source = checker.RUST_FILE_HEADER.sub('', checker.SOURCES['quick-start'].read_text(encoding='utf-8'), count=1)
(app / 'src/main.rs').write_text(source, encoding='utf-8')
(app / 'Cargo.lock').write_bytes((root / 'tests/fixtures/documentation_examples/sync-consumer/Cargo.lock').read_bytes())
PY
metadata=$(cargo metadata --locked --format-version 1 --manifest-path "$scratch/rust-common/report-app/Cargo.toml")
python3 - "$metadata" "$project_root" <<'PYMETA'
import json
import sys
from pathlib import Path
data = json.loads(sys.argv[1])
root = Path(sys.argv[2]).resolve()
packages = [package for package in data['packages'] if package['name'] == 'qubit-fs']
assert len(packages) == 1, packages
assert Path(packages[0]['manifest_path']).resolve() == root / 'Cargo.toml', packages[0]['manifest_path']
node = next(node for node in data['resolve']['nodes'] if node['id'] == packages[0]['id'])
assert 'async' not in node['features'], node['features']
print('locked sync metadata: one qubit-fs package at the selected source path, async disabled')
PYMETA
output=$(cargo run --locked --manifest-path "$scratch/rust-common/report-app/Cargo.toml")
grep -F 'report ready' <<<"$output"
