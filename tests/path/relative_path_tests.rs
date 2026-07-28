//! Tests for safe relative paths.

use qubit_fs::RelativePath;

/// Verifies relative paths cannot escape their logical base.
#[test]
fn test_relative_path_rejects_escape() {
    assert!(RelativePath::parse("../secret").is_err());
}
