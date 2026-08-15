// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error as _;
use std::io;

use qubit_fs::FsError;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemCapability;

#[test]
fn test_fs_error_keeps_request_and_failure_paths_separate() {
    let request = Path::parse("/source").expect("request path should parse");
    let request_target =
        Path::parse("/target").expect("request target should parse");
    let failed =
        Path::parse("/source/nested/second").expect("failed path should parse");
    let failed_target = Path::parse("/target/nested/second")
        .expect("failed target should parse");
    let error = FsError::new(FsErrorKind::Io, FsOperation::Copy, "copy failed")
        .with_path(request.clone())
        .with_target(request_target.clone())
        .with_failure_path(failed.clone())
        .with_failure_target(failed_target.clone());

    assert_eq!(Some(&request), error.path());
    assert_eq!(Some(&request_target), error.target());
    assert_eq!(Some(&failed), error.failure_path());
    assert_eq!(Some(&failed_target), error.failure_target());
}

/// Verifies structured context builders preserve every public error fact while
/// normal formatting deliberately omits the source diagnostic text.
#[test]
fn test_fs_error_preserves_context_and_redacts_source_formatting() {
    let path = Path::parse("/source").expect("path should parse");
    let target = Path::parse("/target").expect("path should parse");
    let error = FsError::with_source(
        FsErrorKind::PermissionDenied,
        FsOperation::Read,
        "safe failure message",
        io::Error::other("secret transport detail"),
    )
    .with_path(path.clone())
    .with_target(target.clone())
    .with_provider("test-provider")
    .with_required_capability(FileSystemCapability::Read)
    .with_operation(FsOperation::OpenReader);

    assert_eq!(FsErrorKind::PermissionDenied, error.kind());
    assert_eq!(FsOperation::OpenReader, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some(&target), error.target());
    assert_eq!(Some("test-provider"), error.provider());
    assert_eq!(
        Some(FileSystemCapability::Read),
        error.required_capability()
    );
    assert!(error.source().is_some());
    assert!(error.to_string().contains("safe failure message"));
    assert!(!error.to_string().contains("secret transport detail"));
    assert!(!format!("{error:?}").contains("secret transport detail"));
}

/// Verifies standard I/O errors are mapped to their typed filesystem
/// categories while retaining the original error as a source.
#[test]
fn test_fs_error_from_io_maps_standard_categories() {
    for (io_kind, expected) in [
        (io::ErrorKind::NotFound, FsErrorKind::NotFound),
        (io::ErrorKind::AlreadyExists, FsErrorKind::AlreadyExists),
        (io::ErrorKind::DirectoryNotEmpty, FsErrorKind::Conflict),
        (io::ErrorKind::NotADirectory, FsErrorKind::NotDirectory),
        (io::ErrorKind::IsADirectory, FsErrorKind::IsDirectory),
        (
            io::ErrorKind::PermissionDenied,
            FsErrorKind::PermissionDenied,
        ),
        (io::ErrorKind::InvalidInput, FsErrorKind::InvalidOptions),
        (
            io::ErrorKind::Unsupported,
            FsErrorKind::UnsupportedOperation,
        ),
        (io::ErrorKind::TimedOut, FsErrorKind::Timeout),
        (io::ErrorKind::Interrupted, FsErrorKind::Interrupted),
        (io::ErrorKind::StorageFull, FsErrorKind::QuotaExceeded),
        (io::ErrorKind::InvalidData, FsErrorKind::DataCorruption),
        (io::ErrorKind::Other, FsErrorKind::Io),
    ] {
        let error =
            FsError::from_io(io::Error::from(io_kind), FsOperation::Read);
        assert_eq!(expected, error.kind(), "{io_kind:?} should map");
        assert_eq!(FsOperation::Read, error.operation());
        assert!(error.source().is_some());
    }
}

/// Verifies every filesystem category uses the intended I/O bridge category
/// and preserves the complete typed error as that bridge's source.
#[test]
fn test_fs_error_into_io_error_maps_categories() {
    for (kind, expected_io_kind) in [
        (FsErrorKind::NotFound, io::ErrorKind::NotFound),
        (FsErrorKind::AlreadyExists, io::ErrorKind::AlreadyExists),
        (FsErrorKind::NotDirectory, io::ErrorKind::NotADirectory),
        (FsErrorKind::IsDirectory, io::ErrorKind::IsADirectory),
        (
            FsErrorKind::PermissionDenied,
            io::ErrorKind::PermissionDenied,
        ),
        (
            FsErrorKind::AuthenticationFailed,
            io::ErrorKind::PermissionDenied,
        ),
        (FsErrorKind::InvalidPath, io::ErrorKind::InvalidInput),
        (FsErrorKind::InvalidUri, io::ErrorKind::InvalidInput),
        (FsErrorKind::InvalidOptions, io::ErrorKind::InvalidInput),
        (FsErrorKind::InvalidState, io::ErrorKind::InvalidInput),
        (
            FsErrorKind::UnsupportedOperation,
            io::ErrorKind::Unsupported,
        ),
        (
            FsErrorKind::UnsupportedCapability,
            io::ErrorKind::Unsupported,
        ),
        (FsErrorKind::Timeout, io::ErrorKind::TimedOut),
        (FsErrorKind::Interrupted, io::ErrorKind::Interrupted),
        (FsErrorKind::Cancelled, io::ErrorKind::Interrupted),
        (FsErrorKind::QuotaExceeded, io::ErrorKind::StorageFull),
        (FsErrorKind::DataCorruption, io::ErrorKind::InvalidData),
        (FsErrorKind::Conflict, io::ErrorKind::Other),
        (FsErrorKind::ProviderContractViolation, io::ErrorKind::Other),
    ] {
        let error = FsError::new(kind, FsOperation::Write, "safe message")
            .into_io_error();
        assert_eq!(expected_io_kind, error.kind(), "{kind:?} should map");
        let source = error
            .get_ref()
            .expect("typed filesystem source should remain");
        assert!(source.downcast_ref::<FsError>().is_some());
    }
}

/// Verifies the dedicated invalid-path constructor supplies the parse-path
/// operation expected by path validation callers.
#[test]
fn test_fs_error_invalid_path_constructor_uses_supplied_operation() {
    let error = FsError::invalid_path(FsOperation::ParsePath, "invalid path");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(FsOperation::ParsePath, error.operation());
}
