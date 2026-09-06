// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Regression tests for the shared, I/O-free path contract validation.

use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimit;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::FileSystemProperties;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;

fn properties(semantics: PathSemantics, constraints: PathConstraints) -> FileSystemProperties {
    FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("path-contract").expect("test id should be valid"),
            "path-contract-provider",
            semantics,
        ),
        FileSystemCapabilities::new(),
        FileSystemLimits::unknown(),
        constraints,
        SymlinkPolicy::Reject,
    )
    .expect("test properties should be valid")
}

#[test]
fn validates_semantics_and_path_form_without_io() {
    let hierarchical = properties(PathSemantics::Hierarchical, PathConstraints::absolute());
    let absolute = Path::parse("/a").expect("absolute path should parse");
    assert!(hierarchical.validate_path(&absolute, FsOperation::Stat).is_ok());

    let relative = Path::parse("a").expect("relative path should parse");
    let error = hierarchical
        .validate_path(&relative, FsOperation::Stat)
        .expect_err("absolute-only filesystem must reject relative paths");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(FsOperation::Stat, error.operation());
    assert_eq!(Some(&relative), error.path());
    assert_eq!(Some("path-contract-provider"), error.provider());

    let object_key = properties(PathSemantics::ObjectKey, PathConstraints::relative());
    let literal = Path::parse_literal("a").expect("literal path should parse");
    assert!(object_key.validate_path(&literal, FsOperation::Stat).is_ok());
    let hierarchical_path = Path::parse("a").expect("hierarchical path should parse");
    let error = object_key
        .validate_path(&hierarchical_path, FsOperation::Stat)
        .expect_err("foreign path semantics must be rejected");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(Some(&hierarchical_path), error.path());
}

#[test]
fn validates_path_limits_and_preserves_operation_context() {
    let limits = FileSystemLimits::unknown().with_max_path_text_bytes(FileSystemLimit::Maximum(3));
    let properties = FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("path-limit").expect("test id should be valid"),
            "path-limit-provider",
            PathSemantics::Hierarchical,
        ),
        FileSystemCapabilities::new(),
        limits,
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("test properties should be valid");
    let path = Path::parse("/long").expect("path should parse");
    let error = properties
        .validate_path(&path, FsOperation::OpenReader)
        .expect_err("path over the provider limit must be rejected");
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(FsOperation::OpenReader, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some("path-limit-provider"), error.provider());
}
