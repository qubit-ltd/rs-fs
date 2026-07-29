// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

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

/// Verifies a valid component remains an opaque lexical value for display and
/// generic string consumers.
#[test]
fn test_path_component_preserves_valid_text() {
    let component =
        PathComponent::parse("report.csv").expect("component should parse");
    assert_eq!("report.csv", component.as_str());
    assert_eq!("report.csv", component.to_string());
}
