// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::io::Cursor;
use std::io::Result as IoResult;

use qubit_fs::FileSystem;
use qubit_fs::Path;
use qubit_fs::copy::CopyFailureState;
use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::error::FsResult;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::metadata::WriteOutcome;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::FileWriterSpi;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::OpenedWriter;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::SpiWriteFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_fs::write::WriteAbortOutcome;
use qubit_fs::write::WriteFailureState;
use qubit_fs_documentation_examples::sync_recovery::copy_report;
use qubit_io::Output;

struct PublishedThenCleanupFailure {
    abort_outcome: Result<WriteAbortOutcome, FsErrorKind>,
}
impl FileSystemSpi for PublishedThenCleanupFailure {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("docs")
                    .expect("documentation filesystem identity should be valid"),
                "docs",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::OpenReader)
                .with(ProviderOperation::OpenWriter),
            FileSystemCapabilities::new()
                .with_guaranteed(FileSystemCapability::Read)
                .with_guaranteed(FileSystemCapability::Write),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("documentation filesystem properties should be valid")
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(Some(5)),
        ))
    }
    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Ok(OpenedReader::new(
            OpenedFileInfo::new(
                self.properties().info().id().clone(),
                request.path().clone(),
            ),
            Box::new(Cursor::new(b"bytes")),
        ))
    }
    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        Ok(OpenedWriter::new(
            OpenedFileInfo::new(
                self.properties().info().id().clone(),
                request.path().clone(),
            ),
            Box::new(FailedWriter {
                abort_outcome: self.abort_outcome.clone(),
            }),
        ))
    }
}

struct FailedWriter {
    abort_outcome: Result<WriteAbortOutcome, FsErrorKind>,
}
impl Output for FailedWriter {
    type Item = u8;
    unsafe fn write_unchecked(&mut self, _: &[u8], _: usize, count: usize) -> IoResult<usize> {
        Ok(count)
    }
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}
impl FileWriterSpi for FailedWriter {
    fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure> {
        Err(SpiWriteFailure::new(
            FsError::new(
                FsErrorKind::Io,
                FsOperation::CommitWriter,
                "published but finalization failed",
            ),
            WriteFailureState::Published,
        ))
    }
    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        self.abort_outcome
            .clone()
            .map_err(|kind| FsError::new(kind, FsOperation::AbortWriter, "cleanup failed"))
    }
}

#[test]
fn test_synchronous_example_retains_publication_and_both_errors() {
    let filesystem = FileSystem::from_spi(PublishedThenCleanupFailure {
        abort_outcome: Err(FsErrorKind::Io),
    })
        .expect("the fixture provider should produce a valid filesystem");
    let recovery = copy_report(
        &filesystem,
        &Path::parse("/source").expect("the source path should be valid"),
        &Path::parse("/target").expect("the target path should be valid"),
    )
    .expect_err("the fixture should preserve the publication and cleanup failure");
    assert_eq!(CopyFailureState::Published, recovery.failure.state());
    assert!(
        recovery
            .failure
            .error()
            .to_string()
            .contains("published but finalization failed")
    );
    assert_eq!(5, recovery.failure.partial_stats().bytes);
    assert!(recovery.failure.has_recovery());
    assert_eq!(
        FsOperation::AbortWriter,
        recovery
            .cleanup_error
            .expect("the fixture writer should report its cleanup failure")
            .operation()
    );
    assert_eq!(None, recovery.cleanup_outcome);
}

#[test]
fn test_synchronous_example_preserves_each_successful_cleanup_outcome() {
    for outcome in [
        WriteAbortOutcome::NotPublished,
        WriteAbortOutcome::Published,
        WriteAbortOutcome::Indeterminate,
    ] {
        let filesystem = FileSystem::from_spi(PublishedThenCleanupFailure {
            abort_outcome: Ok(outcome),
        })
        .expect("the fixture provider should produce a valid filesystem");
        let recovery = copy_report(
            &filesystem,
            &Path::parse("/source").expect("the source path should be valid"),
            &Path::parse("/target").expect("the target path should be valid"),
        )
        .expect_err("the fixture should preserve the primary publication failure");
        assert_eq!(Some(outcome), recovery.cleanup_outcome);
        assert!(recovery.cleanup_error.is_none());
        assert!(recovery.failure.has_recovery());
    }
}
