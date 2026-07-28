//! Tests for individual logical path components.

use qubit_fs::PathComponent;

/// Verifies components reject hierarchy and traversal syntax.
#[test]
fn test_path_component_rejects_hierarchy_and_traversal() {
    for invalid in ["", "/", ".", "..", "a/b", "nul\0byte"] {
        assert!(
            PathComponent::parse(invalid).is_err(),
            "{invalid:?} must fail"
        );
    }
}
