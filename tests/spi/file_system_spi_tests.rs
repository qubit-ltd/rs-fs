// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! SPI failure state tests.

use qubit_fs::CopyFailureState;
use qubit_fs::CopyStats;
use qubit_fs::FileKind;
use qubit_fs::FileMetadata;
use qubit_fs::FsError;
use qubit_fs::FsErrorKind;
use qubit_fs::FsOperation;
use qubit_fs::Path;
use qubit_fs::PersistFailureState;
use qubit_fs::RenameFailureState;
use qubit_fs::WriteFailureState;
use qubit_fs::spi::SpiCopyFailure;
use qubit_fs::spi::SpiPersistFailure;
use qubit_fs::spi::SpiRenameFailure;
use qubit_fs::spi::SpiWriteFailure;

/// Verifies provider copy failures retain typed recovery state and statistics.
#[test]
fn test_spi_copy_failure_preserves_typed_state() {
    let failure = SpiCopyFailure::new(
        FsError::new(
            FsErrorKind::Indeterminate,
            FsOperation::Copy,
            "test failure",
        ),
        CopyFailureState::Indeterminate,
        CopyStats::default(),
    );
    assert_eq!(CopyFailureState::Indeterminate, failure.state());
    assert_eq!(FsErrorKind::Indeterminate, failure.error().kind());
    let (_, state, stats) = failure.into_parts();
    assert_eq!(CopyFailureState::Indeterminate, state);
    assert_eq!(CopyStats::default(), stats);
}

/// Verifies provider rename failures retain typed recovery state.
#[test]
fn test_spi_rename_failure_preserves_typed_state() {
    let failure = SpiRenameFailure::new(
        FsError::new(
            FsErrorKind::Indeterminate,
            FsOperation::Rename,
            "test failure",
        ),
        RenameFailureState::Indeterminate,
    );
    assert_eq!(RenameFailureState::Indeterminate, failure.state());
    assert_eq!(FsErrorKind::Indeterminate, failure.error().kind());
    let (_, state) = failure.into_parts();
    assert_eq!(RenameFailureState::Indeterminate, state);
}

/// Exposes the metadata getter on a provider response before ownership is
/// transferred into the validating facade.
#[test]
fn test_stat_response_exposes_path_and_metadata_snapshot() {
    let path = Path::parse("/file").expect("test path should parse");
    let response = qubit_fs::spi::StatResponse::new(
        path.clone(),
        FileMetadata::new(FileKind::File),
    );
    assert_eq!(&path, response.path());
    assert_eq!(&FileKind::File, response.metadata().kind());
}

/// Verifies provider write failures preserve their error and recovery state.
#[test]
fn test_spi_write_failure_preserves_typed_state() {
    let failure = SpiWriteFailure::new(
        FsError::new(
            FsErrorKind::Io,
            FsOperation::CommitWriter,
            "test failure",
        ),
        WriteFailureState::RetryableNotPublished,
    );
    assert_eq!(FsErrorKind::Io, failure.error().kind());
    assert_eq!(WriteFailureState::RetryableNotPublished, failure.state());
    let (error, state) = failure.into_parts();
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(WriteFailureState::RetryableNotPublished, state);
}

/// Verifies provider temporary-persist failures retain partial-progress facts.
#[test]
fn test_spi_persist_failure_preserves_typed_state() {
    let failure = SpiPersistFailure::new(
        FsError::new(FsErrorKind::Io, FsOperation::PersistTemp, "test failure"),
        PersistFailureState::PublishedSourceRetained,
    );
    assert_eq!(FsErrorKind::Io, failure.error().kind());
    assert_eq!(
        PersistFailureState::PublishedSourceRetained,
        failure.state()
    );
    let (error, state) = failure.into_parts();
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(PersistFailureState::PublishedSourceRetained, state);
}
