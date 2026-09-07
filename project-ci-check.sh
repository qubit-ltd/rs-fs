#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
documentation_root="$project_root"
if [[ -n "${QUBIT_FS_SIBLING_ROOT:-}" ]]; then
    documentation_root="$QUBIT_FS_SIBLING_ROOT/rs-fs"
fi

python3 "$project_root/scripts/check-documentation.py"
fixture_manifest="$documentation_root/tests/fixtures/documentation_examples/Cargo.toml"
fixture_target="$project_root/target/documentation-examples"
cargo test --locked --manifest-path "$fixture_manifest" --target-dir "$fixture_target"
cargo run --locked --quiet --manifest-path "$fixture_manifest" --target-dir "$fixture_target" --bin quick_start
