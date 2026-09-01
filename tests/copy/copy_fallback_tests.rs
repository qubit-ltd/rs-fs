// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Recording-provider coverage for copy dispatch and fallback.

use std::io::Cursor;
use std::io::Result as IoResult;
use std::sync::Arc;
use std::sync::Mutex;

use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::copy::CopyConflictPolicy;
use qubit_fs::copy::CopyFailureState;
use qubit_fs::copy::CopyMethod;
use qubit_fs::copy::CopyOptions;
use qubit_fs::copy::CopyOutcome;
use qubit_fs::copy::CopyStats;
use qubit_fs::copy::MetadataPreservePolicy;
use qubit_fs::copy::ServerSidePreference;
use qubit_fs::directory::CreateDirectoryOutcome;
use qubit_fs::directory::DeleteOutcome;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::AchievedAtomicity;
use qubit_fs::metadata::AtomicityRequirement;
use qubit_fs::metadata::DurabilityRequirement;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimit;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::PublicationMethod;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::metadata::WriteOutcome;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::read::ReadOptions;
use qubit_fs::rename::RenameFailureState;
use qubit_fs::rename::RenameOutcome;
use qubit_fs::spi::CopyAttempt;
use qubit_fs::spi::CopyDeclineReason;
use qubit_fs::spi::CopyRequest;
use qubit_fs::spi::CreateDirectoryRequest;
use qubit_fs::spi::CreateTempDirectoryRequest;
use qubit_fs::spi::CreateTempFileRequest;
use qubit_fs::spi::DeleteDirectoryRequest;
use qubit_fs::spi::DeleteFileRequest;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::FileWriterSpi;
use qubit_fs::spi::ListRequest;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::OpenedDirectoryStream;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::OpenedTempDirectory;
use qubit_fs::spi::OpenedTempFile;
use qubit_fs::spi::OpenedWriter;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::RenameRequest;
use qubit_fs::spi::SpiCopyFailure;
use qubit_fs::spi::SpiRenameFailure;
use qubit_fs::spi::SpiWriteFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_fs::write::WriteAbortOutcome;
use qubit_fs::write::WriteFailureState;
use qubit_fs::write::WriteOptions;
use qubit_fs::write::WriterState;
use qubit_io::Input;
use qubit_io::Output;

/// Selects the provider's response to the copy fast path.
enum CopyResponse {
    Completed,
    CompletedAtomicDowngrade,
    CompletedDurabilityDowngrade,
    CompletedServerSideRequiredButNative,
    CompletedServerSideWhenDisabled,
    CompletedMetadataDowngrade,
    CompletedInvalidSkippedStats,
    CompletedInvalidFailedStats,
    CompletedInvalidOverwrittenStats,
    CompletedStreamedOutcome,
    Declined,
    DeclinedSkipAtomic,
    Failed,
    DeclinedFlushFailure,
    DeclinedWithoutRead,
    DeclinedWithoutWrite,
    DeclinedStatFailure,
    DeclinedUnsupportedSourceKind,
    DeclinedReaderFailure,
    DeclinedReadFailure,
    DeclinedWriterAlreadyExists,
    DeclinedWriterFailure,
    DeclinedWriterInvalidIdentity,
    DeclinedWriteFailure,
    DeclinedCommitFailure,
    DeclinedCommitRetryable,
    DeclinedCommitPublished,
    DeclinedCommitIndeterminate,
}
/// Records facade-to-provider calls and stream-fallback bytes.
struct RecordingSpi {
    response: CopyResponse,
    advertise_copy: bool,
    maximum_read_range_bytes: Option<u64>,
    maximum_write_bytes: Option<u64>,
    calls: Arc<Mutex<Vec<&'static str>>>,
    bytes: Arc<Mutex<Vec<u8>>>,
}

type RecordingHandles = (FileSystem, Arc<Mutex<Vec<&'static str>>>, Arc<Mutex<Vec<u8>>>);

/// Constructs a recording filesystem with a selected fast-path response.
fn recording_filesystem(response: CopyResponse) -> RecordingHandles {
    recording_filesystem_with_options(response, true, None)
}
/// Constructs a recording filesystem with no advertised copy capability.
fn recording_filesystem_without_copy(response: CopyResponse) -> RecordingHandles {
    recording_filesystem_with_options(response, false, None)
}
/// Constructs a recording filesystem with a finite write-session limit.
fn recording_filesystem_with_write_limit(response: CopyResponse, maximum_write_bytes: u64) -> RecordingHandles {
    recording_filesystem_with_options(response, true, Some(maximum_write_bytes))
}
fn recording_filesystem_with_range_limit(response: CopyResponse, maximum_read_range_bytes: u64) -> RecordingHandles {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let filesystem = FileSystem::from_spi(RecordingSpi {
        response,
        advertise_copy: true,
        maximum_read_range_bytes: Some(maximum_read_range_bytes),
        maximum_write_bytes: None,
        calls: Arc::clone(&calls),
        bytes: Arc::clone(&bytes),
    })
    .expect("recording facade should construct");
    (filesystem, calls, bytes)
}
/// Constructs a recording filesystem with explicit capability and limit flags.
fn recording_filesystem_with_options(
    response: CopyResponse,
    advertise_copy: bool,
    maximum_write_bytes: Option<u64>,
) -> RecordingHandles {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let filesystem = FileSystem::from_spi(RecordingSpi {
        response,
        advertise_copy,
        maximum_read_range_bytes: None,
        maximum_write_bytes,
        calls: Arc::clone(&calls),
        bytes: Arc::clone(&bytes),
    })
    .expect("recording facade should construct");
    (filesystem, calls, bytes)
}
/// Builds properties sufficient for native copy, stream fallback, and rename
/// tests.
fn properties(
    response: &CopyResponse,
    advertise_copy: bool,
    maximum_read_range_bytes: Option<u64>,
    maximum_write_bytes: Option<u64>,
) -> ProviderProperties {
    let mut capabilities = FileSystemCapabilities::new()
        .with_guaranteed(FileSystemCapability::Rename)
        .with_guaranteed(FileSystemCapability::AtomicRename);
    if advertise_copy {
        capabilities = capabilities.with_guaranteed(FileSystemCapability::Copy);
    }
    if !matches!(response, CopyResponse::DeclinedWithoutRead) {
        capabilities = capabilities.with_guaranteed(FileSystemCapability::Read);
    }
    if !matches!(response, CopyResponse::DeclinedWithoutWrite) {
        capabilities = capabilities.with_guaranteed(FileSystemCapability::Write);
    }
    if matches!(
        response,
        CopyResponse::CompletedAtomicDowngrade | CopyResponse::DeclinedSkipAtomic
    ) {
        capabilities = capabilities.with_guaranteed(FileSystemCapability::AtomicFileCopy);
    }
    if matches!(response, CopyResponse::CompletedDurabilityDowngrade) {
        capabilities = capabilities.with_guaranteed(FileSystemCapability::DurableFileCopy);
    }
    if matches!(
        response,
        CopyResponse::CompletedServerSideRequiredButNative | CopyResponse::CompletedServerSideWhenDisabled
    ) {
        capabilities = capabilities.with_guaranteed(FileSystemCapability::ServerSideCopy);
    }
    let mut operations = ProviderOperations::new()
        .with(ProviderOperation::Stat)
        .with(ProviderOperation::OpenReader)
        .with(ProviderOperation::OpenWriter)
        .with(ProviderOperation::Rename);
    if advertise_copy {
        operations = operations.with(ProviderOperation::TryCopy);
    }
    ProviderProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("recording").expect("id should be valid"),
            "recording",
            PathSemantics::Hierarchical,
        ),
        operations,
        capabilities,
        FileSystemLimits::unknown()
            .with_max_read_range_bytes(
                maximum_read_range_bytes.map_or(FileSystemLimit::Unknown, FileSystemLimit::Maximum),
            )
            .with_max_write_bytes(maximum_write_bytes.map_or(FileSystemLimit::Unknown, FileSystemLimit::Maximum)),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("properties should be valid")
}
/// Returns a stable absolute test path.
fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}
/// Implements the provider contract while recording every potentially relevant
/// call.
impl FileSystemSpi for RecordingSpi {
    fn properties(&self) -> ProviderProperties {
        properties(
            &self.response,
            self.advertise_copy,
            self.maximum_read_range_bytes,
            self.maximum_write_bytes,
        )
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        self.calls.lock().expect("calls lock should succeed").push("stat");
        if matches!(self.response, CopyResponse::DeclinedStatFailure) {
            return Err(FsError::new(
                FsErrorKind::Io,
                FsOperation::Stat,
                "injected stat failure",
            ));
        }
        let mut metadata = FileMetadata::new(FileKind::File);
        if matches!(self.response, CopyResponse::DeclinedUnsupportedSourceKind) {
            metadata = metadata.with_kind(FileKind::Directory);
        }
        metadata = metadata.with_len(Some(5));
        Ok(StatResponse::new(request.path().clone(), metadata))
    }
    fn list(&self, _: ListRequest<'_>) -> FsResult<OpenedDirectoryStream> {
        Err(unused())
    }
    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        self.calls
            .lock()
            .expect("calls lock should succeed")
            .push("open_reader");
        if matches!(self.response, CopyResponse::DeclinedReaderFailure) {
            return Err(FsError::new(
                FsErrorKind::Io,
                FsOperation::OpenReader,
                "injected reader failure",
            ));
        }
        let reader: Box<dyn Input<Item = u8> + Send> = if matches!(self.response, CopyResponse::DeclinedReadFailure) {
            Box::new(FailingReader)
        } else {
            Box::new(Cursor::new(b"bytes".to_vec()))
        };
        Ok(OpenedReader::new(info(request.path()), reader))
    }
    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        self.calls
            .lock()
            .expect("calls lock should succeed")
            .push("open_writer");
        if matches!(self.response, CopyResponse::DeclinedWriterAlreadyExists) {
            return Err(FsError::new(
                FsErrorKind::AlreadyExists,
                FsOperation::OpenWriter,
                "injected existing target",
            ));
        }
        if matches!(self.response, CopyResponse::DeclinedWriterFailure) {
            return Err(FsError::new(
                FsErrorKind::Io,
                FsOperation::OpenWriter,
                "injected writer failure",
            ));
        }
        let opened_path = if matches!(self.response, CopyResponse::DeclinedWriterInvalidIdentity) {
            path("/wrong")
        } else {
            request.path().clone()
        };
        Ok(OpenedWriter::new(
            info(&opened_path),
            Box::new(RecordingWriter {
                bytes: Arc::clone(&self.bytes),
                fail_flush: matches!(self.response, CopyResponse::DeclinedFlushFailure),
                fail_write: matches!(self.response, CopyResponse::DeclinedWriteFailure),
                commit_failure_state: match self.response {
                    CopyResponse::DeclinedCommitFailure => Some(WriteFailureState::NotPublished),
                    CopyResponse::DeclinedCommitRetryable => Some(WriteFailureState::RetryableNotPublished),
                    CopyResponse::DeclinedCommitPublished => Some(WriteFailureState::Published),
                    CopyResponse::DeclinedCommitIndeterminate => Some(WriteFailureState::Indeterminate),
                    _ => None,
                },
            }),
        ))
    }
    fn create_directory(&self, _: CreateDirectoryRequest<'_>) -> FsResult<CreateDirectoryOutcome> {
        Err(unused())
    }
    fn delete_file(&self, _: DeleteFileRequest<'_>) -> FsResult<DeleteOutcome> {
        Err(unused())
    }
    fn delete_directory(&self, _: DeleteDirectoryRequest<'_>) -> FsResult<DeleteOutcome> {
        Err(unused())
    }
    fn try_copy(&self, _: CopyRequest<'_>) -> Result<CopyAttempt, SpiCopyFailure> {
        self.calls.lock().expect("calls lock should succeed").push("try_copy");
        match self.response {
            CopyResponse::Completed => Ok(CopyAttempt::Completed(CopyOutcome::new(
                CopyStats::default(),
                CopyMethod::Native,
                AchievedAtomicity::Atomic,
            ))),
            CopyResponse::CompletedAtomicDowngrade => Ok(CopyAttempt::Completed(CopyOutcome::new(
                CopyStats::default(),
                CopyMethod::Native,
                AchievedAtomicity::NonAtomic,
            ))),
            CopyResponse::CompletedDurabilityDowngrade => Ok(CopyAttempt::Completed(CopyOutcome::new(
                CopyStats::default(),
                CopyMethod::Native,
                AchievedAtomicity::Atomic,
            ))),
            CopyResponse::CompletedServerSideRequiredButNative | CopyResponse::CompletedMetadataDowngrade => {
                Ok(CopyAttempt::Completed(CopyOutcome::new(
                    CopyStats::default(),
                    CopyMethod::Native,
                    AchievedAtomicity::Atomic,
                )))
            }
            CopyResponse::CompletedServerSideWhenDisabled => Ok(CopyAttempt::Completed(CopyOutcome::new(
                CopyStats::default(),
                CopyMethod::ServerSide,
                AchievedAtomicity::Atomic,
            ))),
            CopyResponse::CompletedInvalidSkippedStats => Ok(CopyAttempt::Completed(CopyOutcome::new(
                CopyStats {
                    skipped: 1,
                    ..CopyStats::default()
                },
                CopyMethod::Native,
                AchievedAtomicity::Atomic,
            ))),
            CopyResponse::CompletedInvalidFailedStats => Ok(CopyAttempt::Completed(CopyOutcome::new(
                CopyStats {
                    failed: 1,
                    ..CopyStats::default()
                },
                CopyMethod::Native,
                AchievedAtomicity::Atomic,
            ))),
            CopyResponse::CompletedInvalidOverwrittenStats => Ok(CopyAttempt::Completed(CopyOutcome::new(
                CopyStats {
                    overwritten: 1,
                    ..CopyStats::default()
                },
                CopyMethod::Native,
                AchievedAtomicity::Atomic,
            ))),
            CopyResponse::CompletedStreamedOutcome => Ok(CopyAttempt::Completed(CopyOutcome::new(
                CopyStats::default(),
                CopyMethod::Streamed,
                AchievedAtomicity::Atomic,
            ))),
            CopyResponse::Declined
            | CopyResponse::DeclinedSkipAtomic
            | CopyResponse::DeclinedFlushFailure
            | CopyResponse::DeclinedWithoutRead
            | CopyResponse::DeclinedWithoutWrite
            | CopyResponse::DeclinedStatFailure
            | CopyResponse::DeclinedUnsupportedSourceKind
            | CopyResponse::DeclinedReaderFailure
            | CopyResponse::DeclinedReadFailure
            | CopyResponse::DeclinedWriterAlreadyExists
            | CopyResponse::DeclinedWriterFailure
            | CopyResponse::DeclinedWriterInvalidIdentity
            | CopyResponse::DeclinedWriteFailure
            | CopyResponse::DeclinedCommitFailure
            | CopyResponse::DeclinedCommitRetryable
            | CopyResponse::DeclinedCommitPublished
            | CopyResponse::DeclinedCommitIndeterminate => Ok(CopyAttempt::Declined(CopyDeclineReason::NotApplicable)),
            CopyResponse::Failed => Err(SpiCopyFailure::new(
                FsError::new(FsErrorKind::Io, FsOperation::BeginCopy, "injected"),
                CopyFailureState::Indeterminate,
                CopyStats::default(),
            )),
        }
    }
    fn rename(&self, _: RenameRequest<'_>) -> Result<RenameOutcome, SpiRenameFailure> {
        Err(SpiRenameFailure::new(unused(), RenameFailureState::Unchanged))
    }
    fn create_temp_file(&self, _: CreateTempFileRequest) -> FsResult<OpenedTempFile> {
        Err(unused())
    }
    fn create_temp_directory(&self, _: CreateTempDirectoryRequest) -> FsResult<OpenedTempDirectory> {
        Err(unused())
    }
}
/// Returns a provider identity bound to `path`.
fn info(path: &Path) -> OpenedFileInfo {
    OpenedFileInfo::new(
        FileSystemId::new("recording").expect("id should be valid"),
        path.clone(),
    )
    .with_metadata(FileMetadata::new(FileKind::File).with_len(Some(5)))
}
/// Returns an unused-operation provider error.
fn unused() -> FsError {
    FsError::new(FsErrorKind::UnsupportedOperation, FsOperation::Other, "unused")
}
/// Captures fallback bytes through the writer SPI.
struct RecordingWriter {
    bytes: Arc<Mutex<Vec<u8>>>,
    fail_flush: bool,
    fail_write: bool,
    commit_failure_state: Option<WriteFailureState>,
}

/// Reports a deterministic failure from the fallback reader after its writer
/// has been opened.
struct FailingReader;

impl Input for FailingReader {
    type Item = u8;

    unsafe fn read_unchecked(&mut self, _: &mut [u8], _: usize, _: usize) -> IoResult<usize> {
        Err(std::io::Error::other("injected read failure"))
    }
}
/// Implements byte output for the recording writer.
impl Output for RecordingWriter {
    type Item = u8;
    unsafe fn write_unchecked(&mut self, buffer: &[u8], _: usize, count: usize) -> IoResult<usize> {
        if self.fail_write {
            return Err(std::io::Error::other("injected write failure"));
        }
        self.bytes
            .lock()
            .expect("bytes lock should succeed")
            .extend_from_slice(&buffer[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> IoResult<()> {
        if self.fail_flush {
            Err(std::io::Error::other("injected flush failure"))
        } else {
            Ok(())
        }
    }
}
/// Publishes recording-writer data without additional effects.
impl FileWriterSpi for RecordingWriter {
    fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure> {
        if let Some(state) = self.commit_failure_state {
            return Err(SpiWriteFailure::new(
                FsError::new(FsErrorKind::Io, FsOperation::CommitWriter, "injected commit failure"),
                state,
            ));
        }
        Ok(WriteOutcome::new(
            AchievedAtomicity::NonAtomic,
            PublicationMethod::StreamCopy,
        ))
    }
    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        Ok(WriteAbortOutcome::NotPublished)
    }
}

/// Requires the facade-owned copy template method instead of a provider-only
/// primitive.
#[test]
fn test_copy_completed_does_not_open_fallback_handles() {
    let (filesystem, calls, _) = recording_filesystem(CopyResponse::Completed);
    let outcome = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect("native copy should succeed");
    assert_eq!(CopyMethod::Native, outcome.method());
    assert_eq!(
        ["try_copy"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}
/// Verifies that a provider decline alone reaches the explicit stream fallback.
#[test]
fn test_copy_declined_uses_allowlisted_stream_fallback() {
    let (filesystem, calls, bytes) = recording_filesystem(CopyResponse::Declined);
    let outcome = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect("safe fallback should succeed");
    assert_eq!(CopyMethod::Streamed, outcome.method());
    assert!(outcome.used_fallback());
    assert_eq!(b"bytes", bytes.lock().expect("bytes lock should succeed").as_slice());
    assert_eq!(
        ["try_copy", "stat", "open_reader", "open_writer"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}
#[test]
fn test_copy_stream_fallback_ignores_range_read_limit() {
    let (filesystem, calls, bytes) = recording_filesystem_with_range_limit(CopyResponse::Declined, 4);
    let outcome = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect("sequential fallback should not use the range-read limit");
    assert_eq!(CopyMethod::Streamed, outcome.method());
    assert_eq!(5, outcome.stats().bytes);
    assert_eq!(b"bytes", bytes.lock().expect("bytes lock should succeed").as_slice());
    assert_eq!(
        ["try_copy", "stat", "open_reader", "open_writer"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Uses the facade stream fallback when the provider does not advertise copy.
#[test]
fn test_copy_fallback_does_not_require_copy_capability() {
    let (filesystem, calls, _) = recording_filesystem_without_copy(CopyResponse::Declined);
    let outcome = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect("read and write capabilities should enable fallback");

    assert!(outcome.used_fallback());
    assert_eq!(
        ["stat", "open_reader", "open_writer"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies a preferred server-side attempt may use the safe stream fallback.
#[test]
fn test_copy_declined_with_preferred_server_side_uses_stream_fallback() {
    let (filesystem, _, bytes) = recording_filesystem(CopyResponse::Declined);
    let outcome = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default().with_server_side(ServerSidePreference::Prefer),
        )
        .expect("preferred server-side copy may fall back");

    assert_eq!(CopyMethod::Streamed, outcome.method());
    assert!(outcome.used_fallback());
    assert_eq!(b"bytes", bytes.lock().expect("bytes lock should succeed").as_slice());
}

/// Rejects copy options the synchronous stream fallback cannot faithfully
/// implement, without opening source or destination handles.
#[test]
fn test_copy_declined_rejects_incompatible_fallback_options() {
    let options = [
        CopyOptions::default().with_continue_on_error(true),
        CopyOptions::default().with_preserve_metadata(MetadataPreservePolicy::Portable),
        CopyOptions::default().with_create_parent(true),
        CopyOptions::default().with_conflict(CopyConflictPolicy::Overwrite),
    ];
    for options in options {
        let (filesystem, calls, _) = recording_filesystem(CopyResponse::Declined);
        let failure = filesystem
            .copy(&path("/source"), &path("/target"), options)
            .expect_err("declined copy must reject an incompatible fallback option");
        assert_eq!(CopyFailureState::Unchanged, failure.state());
        assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
        assert_eq!(
            ["try_copy"],
            calls.lock().expect("calls lock should succeed").as_slice()
        );
    }
}

/// Covers fallback failures before either stream is opened, preserving their
/// unchanged recovery state and provider context.
#[test]
fn test_copy_declined_propagates_pre_stream_failures() {
    for response in [
        CopyResponse::DeclinedWithoutRead,
        CopyResponse::DeclinedWithoutWrite,
        CopyResponse::DeclinedStatFailure,
        CopyResponse::DeclinedUnsupportedSourceKind,
    ] {
        let (filesystem, _, _) = recording_filesystem(response);
        let failure = filesystem
            .copy(&path("/source"), &path("/target"), CopyOptions::default())
            .expect_err("pre-stream fallback failure should be reported");
        assert_eq!(CopyFailureState::Unchanged, failure.state());
        assert_eq!(Some("recording"), failure.error().provider());
    }
}

/// Covers reader and writer failures that occur while preparing or transferring
/// a declined copy fallback.
#[test]
fn test_copy_declined_preserves_stream_and_writer_recovery_states() {
    let (reader_filesystem, _, _) = recording_filesystem(CopyResponse::DeclinedReaderFailure);
    let reader = reader_filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("reader-open failure should stop before publication");
    assert_eq!(CopyFailureState::Unchanged, reader.state());

    let (filesystem, _, _) = recording_filesystem(CopyResponse::DeclinedWriterAlreadyExists);
    let skipped = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default().with_conflict(CopyConflictPolicy::Skip),
        )
        .expect("an existing target should be skipped when explicitly allowed");
    assert_eq!(1, skipped.stats().skipped);

    for (response, expected, writer_state) in [
        (
            CopyResponse::DeclinedReadFailure,
            CopyFailureState::Unchanged,
            WriterState::Open,
        ),
        (
            CopyResponse::DeclinedWriteFailure,
            CopyFailureState::Indeterminate,
            WriterState::Indeterminate,
        ),
        (
            CopyResponse::DeclinedFlushFailure,
            CopyFailureState::Indeterminate,
            WriterState::Indeterminate,
        ),
        (
            CopyResponse::DeclinedCommitFailure,
            CopyFailureState::Unchanged,
            WriterState::NotPublished,
        ),
        (
            CopyResponse::DeclinedCommitRetryable,
            CopyFailureState::Unchanged,
            WriterState::Open,
        ),
        (
            CopyResponse::DeclinedCommitPublished,
            CopyFailureState::Published,
            WriterState::Published,
        ),
        (
            CopyResponse::DeclinedCommitIndeterminate,
            CopyFailureState::Indeterminate,
            WriterState::Indeterminate,
        ),
    ] {
        let (filesystem, _, _) = recording_filesystem(response);
        let failure = filesystem
            .copy(&path("/source"), &path("/target"), CopyOptions::default())
            .expect_err("fallback writer failure should preserve recovery");
        assert_eq!(expected, failure.state());
        let (_, _, _, writer) = failure.into_parts();
        assert_eq!(
            writer_state,
            writer.expect("post-open fallback failure retains its writer").state()
        );
    }

    for response in [
        CopyResponse::DeclinedWriterFailure,
        CopyResponse::DeclinedWriterInvalidIdentity,
    ] {
        let (filesystem, _, _) = recording_filesystem(response);
        let failure = filesystem
            .copy(&path("/source"), &path("/target"), CopyOptions::default())
            .expect_err("fallback writer failure should preserve recovery");
        assert_eq!(CopyFailureState::Unchanged, failure.state());
        assert!(!failure.has_writer());
    }
}

/// Maps fallback reader I/O failures through the public whole-file convenience
/// operation without exposing the provider's raw error.
#[test]
fn test_read_all_contextualizes_reader_failure() {
    let (filesystem, _, _) = recording_filesystem(CopyResponse::DeclinedReadFailure);
    let source = path("/source");
    let error = filesystem
        .read_all(&source, ReadOptions::default(), 16)
        .expect_err("reader failure should be contextualized");
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&source), error.path());
}

/// Includes provider context when the synchronous read-all byte cap is hit.
#[test]
fn test_read_all_limit_error_includes_provider_context() {
    let (filesystem, _, _) = recording_filesystem(CopyResponse::Declined);
    let error = filesystem
        .read_all(&path("/source"), ReadOptions::default(), 2)
        .expect_err("read-all limit should reject oversized content");

    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(Some("recording"), error.provider());
    assert!(std::error::Error::source(&error).is_some());
}

/// Retains the public write-all failure type when the writer cannot be opened.
#[test]
fn test_write_all_wraps_writer_open_failure() {
    let (filesystem, _, _) = recording_filesystem(CopyResponse::DeclinedWriterFailure);
    let target = path("/target");
    let failure = filesystem
        .write_all(&target, b"bytes", WriteOptions::default())
        .expect_err("writer-open failure should use the write-all failure type");
    assert_eq!(FsErrorKind::Io, failure.error().kind());
    assert!(failure.writer().is_none());
}

/// Includes provider context when the synchronous write-all limit is hit.
#[test]
fn test_write_all_limit_error_includes_provider_context() {
    let (filesystem, _, _) = recording_filesystem_with_write_limit(CopyResponse::Declined, 1);
    let failure = filesystem
        .write_all(&path("/target"), b"bytes", WriteOptions::default())
        .expect_err("write-all limit should reject oversized content");

    assert_eq!(FsErrorKind::ResourceLimitExceeded, failure.error().kind());
    assert_eq!(Some("recording"), failure.error().provider());
}
/// Verifies that a typed provider failure terminates without a fallback retry.
#[test]
fn test_copy_spi_failure_never_retries() {
    let (filesystem, calls, _) = recording_filesystem(CopyResponse::Failed);
    let failure = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("native failure should terminate");
    assert_eq!(CopyFailureState::Indeterminate, failure.state());
    assert_eq!(Some(&path("/source")), failure.error().path());
    assert_eq!(Some(&path("/target")), failure.error().target());
    assert_eq!(Some("recording"), failure.error().provider());
    assert_eq!(
        ["try_copy"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies unmet required semantics fail before any provider operation.
#[test]
fn test_copy_required_atomicity_preflight_has_zero_spi_calls() {
    let (filesystem, calls, _) = recording_filesystem(CopyResponse::Completed);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default().with_atomicity(AtomicityRequirement::Required),
        )
        .expect_err("missing atomic guarantee must fail locally");
    assert_eq!(CopyFailureState::Unchanged, failure.state());
    assert_eq!(
        Some(FileSystemCapability::AtomicFileCopy),
        failure.error().required_capability()
    );
    assert!(calls.lock().expect("calls lock should succeed").is_empty());
}

/// Verifies required durability also rejects before any provider call.
#[test]
fn test_copy_required_durability_preflight_has_zero_spi_calls() {
    let (filesystem, calls, _) = recording_filesystem(CopyResponse::Completed);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default().with_durability(DurabilityRequirement::Required),
        )
        .expect_err("missing durability guarantee must fail locally");
    assert_eq!(CopyFailureState::Unchanged, failure.state());
    assert_eq!(
        Some(FileSystemCapability::DurableFileCopy),
        failure.error().required_capability()
    );
    assert!(calls.lock().expect("calls lock should succeed").is_empty());
}

/// Verifies a completed non-atomic result cannot satisfy an atomic requirement.
#[test]
fn test_copy_completed_atomicity_downgrade_is_contract_failure() {
    let (filesystem, calls, _) = recording_filesystem(CopyResponse::CompletedAtomicDowngrade);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default().with_atomicity(AtomicityRequirement::Required),
        )
        .expect_err("downgrade must fail");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
    assert_eq!(
        ["try_copy"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies declared durability cannot mask a completed non-durable provider
/// outcome.
#[test]
fn test_copy_completed_durability_downgrade_is_contract_failure() {
    let (filesystem, calls, _) = recording_filesystem(CopyResponse::CompletedDurabilityDowngrade);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default().with_durability(DurabilityRequirement::Required),
        )
        .expect_err("durability downgrade must fail");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
    assert_eq!(
        ["try_copy"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies a completed result cannot claim to satisfy required server-side
/// copy when its reported method is not server-side.
#[test]
fn test_copy_completed_non_server_side_method_violates_required_server_side() {
    let (filesystem, calls, _) = recording_filesystem(CopyResponse::CompletedServerSideRequiredButNative);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default().with_server_side(ServerSidePreference::Require),
        )
        .expect_err("native result cannot satisfy required server-side copy");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
    assert_eq!(
        ["try_copy"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies a provider cannot ignore an explicit request to avoid server-side
/// copy while still reporting a server-side completed outcome.
#[test]
fn test_copy_completed_server_side_method_violates_disabled_preference() {
    let (filesystem, calls, _) = recording_filesystem(CopyResponse::CompletedServerSideWhenDisabled);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default().with_server_side(ServerSidePreference::Disable),
        )
        .expect_err("server-side outcome must honor the disabled preference");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
    assert_eq!(
        ["try_copy"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies a provider-completed result reports the requested metadata
/// preservation fact rather than silently returning its default none value.
#[test]
fn test_copy_completed_missing_metadata_preservation_is_contract_failure() {
    let (filesystem, calls, _) = recording_filesystem(CopyResponse::CompletedMetadataDowngrade);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default().with_preserve_metadata(MetadataPreservePolicy::Portable),
        )
        .expect_err("missing metadata preservation must be rejected");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
    assert_eq!(
        ["try_copy"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies a successful copy cannot report skipped entries under a
/// fail-on-conflict request.
#[test]
fn test_copy_completed_skipped_stats_violate_fail_conflict_policy() {
    let (filesystem, _, _) = recording_filesystem(CopyResponse::CompletedInvalidSkippedStats);
    let failure = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("skipped stats must match the conflict policy");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
}

/// Verifies a native provider result cannot report failed entries unless the
/// caller explicitly accepted continued errors.
#[test]
fn test_copy_completed_failed_stats_violate_stop_on_error_policy() {
    let (filesystem, _, _) = recording_filesystem(CopyResponse::CompletedInvalidFailedStats);
    let failure = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("failed stats must match the continue-on-error policy");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
}

/// Verifies a native provider result cannot report overwritten entries unless
/// the request selected overwrite conflict handling.
#[test]
fn test_copy_completed_overwritten_stats_violate_fail_conflict_policy() {
    let (filesystem, _, _) = recording_filesystem(CopyResponse::CompletedInvalidOverwrittenStats);
    let failure = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("overwritten stats must match the conflict policy");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
}

/// Verifies only the facade may return a streamed fallback outcome; providers
/// must not present it as a native fast-path completion.
#[test]
fn test_copy_completed_streamed_outcome_violates_native_contract() {
    let (filesystem, _, _) = recording_filesystem(CopyResponse::CompletedStreamedOutcome);
    let failure = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("providers must not return facade fallback outcomes");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
}

/// Verifies a declined atomic skip request opens neither fallback stream
/// handle.
#[test]
fn test_copy_declined_skip_required_atomicity_rejects_without_opening_handles() {
    let (filesystem, calls, _) = recording_filesystem(CopyResponse::DeclinedSkipAtomic);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default()
                .with_conflict(CopyConflictPolicy::Skip)
                .with_atomicity(AtomicityRequirement::Required),
        )
        .expect_err("skip cannot satisfy atomic publication");
    assert_eq!(CopyFailureState::Unchanged, failure.state());
    assert_eq!(
        ["try_copy"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies post-writer fallback failure retains recovery and transfer facts.
#[test]
fn test_copy_fallback_flush_failure_is_indeterminate_with_stats_and_writer() {
    let (filesystem, _, _) = recording_filesystem(CopyResponse::DeclinedFlushFailure);
    let mut failure = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("flush failure should be recoverable");
    assert_eq!(CopyFailureState::Indeterminate, failure.state());
    assert_eq!(5, failure.partial_stats().bytes);
    assert!(failure.has_writer());
    assert!(!format!("{failure}").is_empty());
    assert!(format!("{failure:?}").contains("CopyFailure"));
    assert_eq!(
        format!("{}", failure.error()),
        format!(
            "{}",
            std::error::Error::source(&failure).expect("failure should expose its source")
        )
    );
    assert_eq!(
        WriterState::Indeterminate,
        failure
            .writer()
            .expect("post-open fallback failure retains its writer")
            .state()
    );
    assert_eq!(
        WriterState::Indeterminate,
        failure
            .writer_mut()
            .expect("fallback writer should still be recoverable")
            .state()
    );
    assert_eq!(
        WriterState::Indeterminate,
        failure
            .take_writer()
            .expect("take_writer should move the recovery writer")
            .state()
    );
    assert!(!failure.has_writer());

    let (error, state, stats, writer) = failure.into_parts();
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(CopyFailureState::Indeterminate, state);
    assert_eq!(5, stats.bytes);
    assert!(writer.is_none());
}
