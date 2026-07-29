// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Tests for logical filesystem paths.

use qubit_fs::{
    Path,
    PathComponent,
    PathSemantics,
    RelativePath,
};

/// Verifies normalized parsing and joins use only validated logical values.
#[test]
fn test_path_normalized_parse_and_safe_join() {
    let root = Path::parse("/bucket").expect("absolute path should parse");
    let child = PathComponent::parse("object").expect("component should parse");
    let relative =
        RelativePath::parse("dir/file").expect("relative path should parse");
    assert_eq!(root.child(&child).as_str(), "/bucket/object");
    assert_eq!(root.join(&relative).as_str(), "/bucket/dir/file");
}

/// Verifies literal provider keys preserve lexical separators and dot text.
#[test]
fn test_path_literal_parse_preserves_repeated_separator_and_dot_text() {
    let path =
        Path::parse_with_semantics("key//./value", PathSemantics::ObjectKey)
            .expect("literal provider key should parse");
    assert_eq!(path.as_str(), "key//./value");
}

/// Verifies a hierarchical root is represented separately from components.
#[test]
fn test_path_components_do_not_emit_a_root_placeholder() {
    let root = Path::root();
    assert!(root.is_absolute());
    assert_eq!(root.components().collect::<Vec<_>>(), Vec::<&str>::new());
}

/// Verifies literal paths retain a leading separator as a lexical boundary.
#[test]
fn test_path_components_preserve_literal_leading_separator_boundary() {
    let path =
        Path::parse_literal("/bucket/key").expect("literal path should parse");
    assert_eq!(
        path.components().collect::<Vec<_>>(),
        vec!["", "bucket", "key"]
    );
}
