// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Tests for logical filesystem paths.

use qubit_fs::Path;
use qubit_fs::PathComponent;
use qubit_fs::PathSemantics;
use qubit_fs::RelativePath;

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

/// Verifies hierarchical parsing normalizes dots and separators, and exposes
/// its resulting spelling, semantics, and display form consistently.
#[test]
fn test_path_normalizes_hierarchical_text_and_exposes_attributes() {
    let path = Path::parse("/bucket//./folder/../object")
        .expect("hierarchical path should normalize");
    assert_eq!("/bucket/object", path.as_str());
    assert_eq!("/bucket/object", path.to_string());
    assert_eq!("/bucket/object", path.as_ref());
    assert!(path.is_absolute());
    assert_eq!(PathSemantics::Hierarchical, path.semantics());
    assert_eq!(
        vec!["bucket", "object"],
        path.components().collect::<Vec<_>>()
    );
}

/// Verifies a canonical hierarchical spelling can be parsed again without
/// changing path identity.
#[test]
fn test_path_canonical_text_round_trips() {
    let path = Path::parse("/bucket//./folder/../object")
        .expect("hierarchical path should normalize");
    let reparsed =
        Path::parse(path.as_str()).expect("canonical path should reparse");
    assert_eq!(path, reparsed);
}

/// Verifies provider-specific path semantics preserve lexical text without
/// treating separators or dots as hierarchy.
#[test]
fn test_path_provider_specific_semantics_preserves_literal_text() {
    let path = Path::parse_with_semantics(
        "provider//./key",
        PathSemantics::ProviderSpecific,
    )
    .expect("provider-specific path should parse");
    assert_eq!("provider//./key", path.as_str());
    assert_eq!(PathSemantics::ProviderSpecific, path.semantics());
    assert!(!path.is_absolute());
}

/// Verifies invalid hierarchical input cannot represent an empty path, NUL,
/// or a traversal above the provider root.
#[test]
fn test_path_rejects_empty_nul_and_root_escape() {
    for invalid in ["", "nul\0byte", "../outside", "/../../outside", "."] {
        assert!(Path::parse(invalid).is_err(), "{invalid:?} must fail");
    }
}

/// Verifies file names distinguish roots, literal trailing separators, and
/// ordinary final components.
#[test]
fn test_path_file_name_handles_root_and_trailing_separator() {
    assert_eq!(None, Path::root().file_name());
    assert_eq!(
        Some("object"),
        Path::parse("/bucket/object")
            .expect("hierarchical path should parse")
            .file_name()
    );
    assert_eq!(
        Some("object"),
        Path::parse_literal("/bucket/object")
            .expect("literal path should parse")
            .file_name()
    );
    assert_eq!(
        None,
        Path::parse_literal("/bucket/object/")
            .expect("literal path should parse")
            .file_name()
    );
}
