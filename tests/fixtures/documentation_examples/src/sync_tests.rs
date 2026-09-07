//! Exercises the guide's synchronous recovery carrier through the public
//! facade.

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
use qubit_io::Output;

use super::sync_recovery::copy_report;

struct PublishedThenCleanupFailure;
impl FileSystemSpi for PublishedThenCleanupFailure {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(FileSystemId::new("docs").unwrap(), "docs", PathSemantics::Hierarchical),
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
        .unwrap()
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(Some(5)),
        ))
    }
    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Ok(OpenedReader::new(
            OpenedFileInfo::new(self.properties().info().id().clone(), request.path().clone()),
            Box::new(Cursor::new(b"bytes")),
        ))
    }
    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        Ok(OpenedWriter::new(
            OpenedFileInfo::new(self.properties().info().id().clone(), request.path().clone()),
            Box::new(FailedWriter),
        ))
    }
}

struct FailedWriter;
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
        Err(FsError::new(
            FsErrorKind::Io,
            FsOperation::AbortWriter,
            "cleanup failed",
        ))
    }
}

#[test]
fn synchronous_example_retains_publication_and_both_errors() {
    let filesystem = FileSystem::from_spi(PublishedThenCleanupFailure).unwrap();
    let recovery = copy_report(
        &filesystem,
        &Path::parse("/source").unwrap(),
        &Path::parse("/target").unwrap(),
    )
    .unwrap_err();
    assert_eq!(CopyFailureState::Published, recovery.failure.state());
    assert!(
        recovery
            .failure
            .error()
            .to_string()
            .contains("published but finalization failed")
    );
    assert_eq!(5, recovery.failure.partial_stats().bytes);
    assert!(recovery.failure.has_writer());
    assert_eq!(FsOperation::AbortWriter, recovery.cleanup_error.unwrap().operation());
}
