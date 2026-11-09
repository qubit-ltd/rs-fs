#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$project_root"

cargo test --locked --manifest-path tests/fixtures/documentation_examples/Cargo.toml
cargo run --locked --manifest-path tests/fixtures/documentation_examples/Cargo.toml --bin quick_start
