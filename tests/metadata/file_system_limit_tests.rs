// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    FileSystemLimit,
    FileSystemLimits,
    FsErrorKind,
    FsOperation,
    FsPath,
    PathSemantics,
};

#[test]
fn finite_limit_uses_an_inclusive_maximum() {
    let limit = FileSystemLimit::Maximum(8);

    assert_eq!(Some(8), limit.maximum());
    assert!(!limit.is_exceeded_by(8));
    assert!(limit.is_exceeded_by(9));
}

#[test]
fn non_finite_limit_states_do_not_reject_values() {
    for limit in [
        FileSystemLimit::Unknown,
        FileSystemLimit::NotApplicable,
        FileSystemLimit::Unbounded,
    ] {
        assert_eq!(None, limit.maximum());
        assert!(!limit.is_exceeded_by(u64::MAX));
    }
}

#[test]
fn zero_is_a_valid_finite_maximum() {
    let limit = FileSystemLimit::Maximum(0);

    assert!(!limit.is_exceeded_by(0));
    assert!(limit.is_exceeded_by(1));
}

#[test]
fn filesystem_limits_have_explicit_states_and_units() {
    let limits = FileSystemLimits::unknown()
        .with_max_path_text_bytes(FileSystemLimit::Maximum(4096))
        .with_max_component_text_bytes(FileSystemLimit::Maximum(255))
        .with_max_read_range_bytes(FileSystemLimit::Unbounded)
        .with_max_write_bytes(FileSystemLimit::Maximum(1_048_576))
        .with_max_list_page_entries(FileSystemLimit::NotApplicable);

    assert_eq!(FileSystemLimit::Maximum(4096), limits.max_path_text_bytes(),);
    assert_eq!(
        FileSystemLimit::Maximum(255),
        limits.max_component_text_bytes(),
    );
    assert_eq!(FileSystemLimit::Unbounded, limits.max_read_range_bytes());
    assert_eq!(
        FileSystemLimit::Maximum(1_048_576),
        limits.max_write_bytes(),
    );
    assert_eq!(
        FileSystemLimit::NotApplicable,
        limits.max_list_page_entries(),
    );
}

#[test]
fn unknown_filesystem_limits_are_explicit() {
    let limits = FileSystemLimits::unknown();

    assert_eq!(FileSystemLimit::Unknown, limits.max_path_text_bytes());
    assert_eq!(FileSystemLimit::Unknown, limits.max_component_text_bytes(),);
    assert_eq!(FileSystemLimit::Unknown, limits.max_read_range_bytes());
    assert_eq!(FileSystemLimit::Unknown, limits.max_write_bytes());
    assert_eq!(FileSystemLimit::Unknown, limits.max_list_page_entries(),);
}

#[test]
fn path_limits_validate_canonical_text_and_hierarchical_components() {
    let path = FsPath::parse_normalized("/abc/de").unwrap();
    let accepted = FileSystemLimits::unknown()
        .with_max_path_text_bytes(FileSystemLimit::Maximum(7))
        .with_max_component_text_bytes(FileSystemLimit::Maximum(3));

    accepted
        .validate_path(&path, PathSemantics::Hierarchical, FsOperation::Stat)
        .unwrap();

    let path_error = accepted
        .with_max_path_text_bytes(FileSystemLimit::Maximum(6))
        .validate_path(&path, PathSemantics::Hierarchical, FsOperation::Stat)
        .unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, path_error.kind());
    assert_eq!(FsOperation::Stat, path_error.operation());
    assert_eq!(Some(&path), path_error.path());

    let component_error = accepted
        .with_max_component_text_bytes(FileSystemLimit::Maximum(2))
        .validate_path(&path, PathSemantics::Hierarchical, FsOperation::List)
        .unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, component_error.kind());
    assert_eq!(FsOperation::List, component_error.operation());
}

#[test]
fn component_limit_does_not_apply_to_object_keys() {
    let path = FsPath::parse_literal("abc/long-key").unwrap();
    let limits = FileSystemLimits::unknown()
        .with_max_component_text_bytes(FileSystemLimit::Maximum(1));

    limits
        .validate_path(&path, PathSemantics::ObjectKey, FsOperation::Stat)
        .unwrap();
}

#[test]
fn operation_limits_validate_ranges_and_write_sessions() {
    let path = FsPath::parse_normalized("/file").unwrap();
    let limits = FileSystemLimits::unknown()
        .with_max_read_range_bytes(FileSystemLimit::Maximum(8))
        .with_max_write_bytes(FileSystemLimit::Maximum(8));

    limits.validate_read_range(&path, Some(8)).unwrap();
    limits.validate_write_size(&path, 8).unwrap();

    let read_error = limits.validate_read_range(&path, Some(9)).unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, read_error.kind());
    assert_eq!(FsOperation::OpenReader, read_error.operation());

    let write_error = limits.validate_write_size(&path, 9).unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, write_error.kind());
    assert_eq!(FsOperation::Write, write_error.operation());
}
