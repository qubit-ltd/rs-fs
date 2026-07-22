// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::{
    FileSystemCapability,
    FsError,
    FsErrorKind,
    FsOperation,
    FsPath,
};
use qubit_spi::ProviderId;

use std::io;

const SECRET_SOURCE_TEXT: &str =
    "https://storage.example/object?x-amz-signature=secret-signature";

struct SecretSourceError;

impl std::fmt::Debug for SecretSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(SECRET_SOURCE_TEXT)
    }
}

impl std::fmt::Display for SecretSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(SECRET_SOURCE_TEXT)
    }
}

impl std::error::Error for SecretSourceError {}

#[test]
fn test_fs_error_carries_context_and_source() {
    let path = FsPath::parse("/source").expect("path should parse");
    let target = FsPath::parse("/target").expect("path should parse");
    let provider = ProviderId::new("mock").expect("provider id should parse");
    let error = FsError::with_source(
        FsErrorKind::Io,
        FsOperation::Copy,
        "copy failed",
        io::Error::other("source"),
    )
    .with_path(path.clone())
    .with_target(target.clone())
    .with_provider(provider.clone())
    .with_required_capability(FileSystemCapability::ServerSideCopy);

    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(FsOperation::Copy, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some(&target), error.target());
    assert_eq!(Some(&provider), error.provider());
    assert!(error.to_string().contains("copy failed"));
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(
        Some(FileSystemCapability::ServerSideCopy),
        error.required_capability(),
    );
}

#[test]
fn fs_error_without_optional_context_reports_none() {
    let error = FsError::new(FsErrorKind::Other, FsOperation::Other, "plain");

    assert_eq!(None, error.path());
    assert_eq!(None, error.target());
    assert_eq!(None, error.provider());
    assert_eq!(None, error.required_capability());
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn test_invalid_path_sets_invalid_path_kind() {
    assert_eq!(
        FsErrorKind::InvalidPath,
        FsError::invalid_path(FsOperation::ParsePath, "bad").kind(),
    );
}

#[test]
fn fs_error_round_trips_through_io_error_as_source() {
    let io_error = FsError::new(
        FsErrorKind::PermissionDenied,
        FsOperation::OpenReader,
        "denied",
    )
    .into_io_error();

    assert_eq!(std::io::ErrorKind::PermissionDenied, io_error.kind());
    assert!(
        io_error
            .get_ref()
            .and_then(|source| source.downcast_ref::<FsError>())
            .is_some()
    );
}

#[test]
fn fs_error_maps_every_specific_kind_to_io_error() {
    let cases = [
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
        (FsErrorKind::ResourceLimitExceeded, io::ErrorKind::Other),
        (FsErrorKind::DataCorruption, io::ErrorKind::InvalidData),
        (FsErrorKind::Other, io::ErrorKind::Other),
    ];

    for (fs_kind, io_kind) in cases {
        let error = FsError::new(fs_kind, FsOperation::Other, "mapping")
            .into_io_error();
        assert_eq!(io_kind, error.kind());
    }
}

#[test]
fn fs_error_maps_io_error_kinds_and_preserves_the_source() {
    let cases = [
        (io::ErrorKind::NotFound, FsErrorKind::NotFound),
        (io::ErrorKind::AlreadyExists, FsErrorKind::AlreadyExists),
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
    ];

    for (io_kind, fs_kind) in cases {
        let error = FsError::from_io(
            io::Error::new(io_kind, "stream source"),
            FsOperation::OpenReader,
        );
        assert_eq!(fs_kind, error.kind());
        assert_eq!(FsOperation::OpenReader, error.operation());
        assert!(std::error::Error::source(&error).is_some());
    }
}

#[test]
fn fs_error_debug_and_display_do_not_expand_secret_source_errors() {
    let error = FsError::with_source(
        FsErrorKind::Io,
        FsOperation::OpenReader,
        "provider request failed",
        SecretSourceError,
    )
    .with_path(FsPath::parse("/object").unwrap());

    let display = error.to_string();
    let debug = format!("{error:?}");
    assert!(!display.contains(SECRET_SOURCE_TEXT));
    assert!(!debug.contains(SECRET_SOURCE_TEXT));
    assert!(debug.contains("source_present"));
    assert!(debug.contains("provider request failed"));
    assert_eq!(
        SECRET_SOURCE_TEXT,
        std::error::Error::source(&error)
            .expect("source should remain available explicitly")
            .to_string(),
    );
}
