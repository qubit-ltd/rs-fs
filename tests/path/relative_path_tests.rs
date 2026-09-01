// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Tests for safe relative paths.

use qubit_fs::path::RelativePath;

/// Verifies relative paths cannot escape their logical base.
#[test]
fn test_relative_path_rejects_escape() {
    assert!(RelativePath::parse("../secret").is_err());
}

/// Verifies relative paths normalize lexical dot and separator components
/// while retaining only descendants below their supplied base.
#[test]
fn test_relative_path_normalizes_and_formats_descendants() {
    let path = RelativePath::parse("folder//./nested/../report.csv").expect("relative descendant should parse");
    assert_eq!("folder/report.csv", path.as_str());
    assert_eq!("folder/report.csv", path.to_string());
}

/// Verifies all invalid relative boundaries are rejected before a path can be
/// joined to a provider resource.
#[test]
fn test_relative_path_rejects_empty_absolute_nul_and_dot_only_forms() {
    for invalid in ["", "/absolute", "nul\0byte", ".", "folder/../../outside"] {
        assert!(RelativePath::parse(invalid).is_err(), "{invalid:?} must fail");
    }
}
