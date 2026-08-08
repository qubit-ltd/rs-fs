// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Recording-provider coverage for copy dispatch and fallback.

use std::io::{
    Cursor,
    Result as IoResult,
};
use std::sync::{
    Arc,
    Mutex,
};

use qubit_fs::spi::{
    CopyAttempt,
    CopyDeclineReason,
    CopyRequest,
    CreateDirectoryRequest,
    CreateTempDirectoryRequest,
    CreateTempFileRequest,
    DeleteDirectoryRequest,
    DeleteFileRequest,
    FileSystemSpi,
    FileWriterSpi,
    ListRequest,
    OpenReaderRequest,
    OpenWriterRequest,
    OpenedDirectoryStream,
    OpenedReader,
    OpenedTempDirectory,
    OpenedTempFile,
    OpenedWriter,
    RenameRequest,
    SpiCopyFailure,
    SpiRenameFailure,
    StatRequest,
    StatResponse,
};
use qubit_fs::{
    AchievedAtomicity,
    AtomicityRequirement,
    CopyFailureState,
    CopyMethod,
    CopyOptions,
    CopyOutcome,
    CopyStats,
    CreateDirectoryOutcome,
    DeleteOutcome,
    FileKind,
    FileMetadata,
    FileSystem,
    FileSystemCapabilities,
    FileSystemCapability,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimits,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    MetadataPreservePolicy,
    OpenedFileInfo,
    Path,
    PathConstraints,
    RenameFailureState,
    RenameOutcome,
    ServerSidePreference,
    SymlinkPolicy,
    WriteOutcome,
};
use qubit_io::{
    Input,
    Output,
};

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
    maximum_write_bytes: Option<u64>,
    calls: Arc<Mutex<Vec<&'static str>>>,
    bytes: Arc<Mutex<Vec<u8>>>,
}

type RecordingHandles = (
    FileSystem,
    Arc<Mutex<Vec<&'static str>>>,
    Arc<Mutex<Vec<u8>>>,
);

/// Constructs a recording filesystem with a selected fast-path response.
fn recording_filesystem(response: CopyResponse) -> RecordingHandles {
    recording_filesystem_with_options(response, true, None)
}
/// Constructs a recording filesystem with no advertised copy capability.
fn recording_filesystem_without_copy(
    response: CopyResponse,
) -> RecordingHandles {
    recording_filesystem_with_options(response, false, None)
}
/// Constructs a recording filesystem with a finite write-session limit.
fn recording_filesystem_with_write_limit(
    response: CopyResponse,
    maximum_write_bytes: u64,
) -> RecordingHandles {
    recording_filesystem_with_options(response, true, Some(maximum_write_bytes))
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
    maximum_write_bytes: Option<u64>,
) -> FileSystemProperties {
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
        capabilities =
            capabilities.with_guaranteed(FileSystemCapability::Write);
    }
    if matches!(
        response,
        CopyResponse::CompletedAtomicDowngrade
            | CopyResponse::DeclinedSkipAtomic
    ) {
        capabilities =
            capabilities.with_guaranteed(FileSystemCapability::AtomicFileCopy);
    }
    if matches!(response, CopyResponse::CompletedDurabilityDowngrade) {
        capabilities =
            capabilities.with_guaranteed(FileSystemCapability::DurableFileCopy);
    }
    if matches!(
        response,
        CopyResponse::CompletedServerSideRequiredButNative
            | CopyResponse::CompletedServerSideWhenDisabled
    ) {
        capabilities =
            capabilities.with_guaranteed(FileSystemCapability::ServerSideCopy);
    }
    FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("recording").expect("id should be valid"),
            "recording",
            qubit_fs::PathSemantics::Hierarchical,
        ),
        capabilities,
        maximum_write_bytes.map_or_else(FileSystemLimits::unknown, |maximum| {
            FileSystemLimits::unknown().with_max_write_bytes(
                qubit_fs::FileSystemLimit::Maximum(maximum),
            )
        }),
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
    fn properties(&self) -> FileSystemProperties {
        properties(
            &self.response,
            self.advertise_copy,
            self.maximum_write_bytes,
        )
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        self.calls
            .lock()
            .expect("calls lock should succeed")
            .push("stat");
        if matches!(self.response, CopyResponse::DeclinedStatFailure) {
            return Err(FsError::new(
                FsErrorKind::Io,
                FsOperation::Stat,
                "injected stat failure",
            ));
        }
        let mut metadata = FileMetadata::new(FileKind::File);
        if matches!(self.response, CopyResponse::DeclinedUnsupportedSourceKind)
        {
            metadata = metadata.with_kind(FileKind::Directory);
        }
        metadata = metadata.with_len(Some(5));
        Ok(StatResponse::new(request.path().clone(), metadata))
    }
    fn list(&self, _: ListRequest<'_>) -> FsResult<OpenedDirectoryStream> {
        Err(unused())
    }
    fn open_reader(
        &self,
        request: OpenReaderRequest<'_>,
    ) -> FsResult<OpenedReader> {
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
        let reader: Box<dyn Input<Item = u8> + Send> =
            if matches!(self.response, CopyResponse::DeclinedReadFailure) {
                Box::new(FailingReader)
            } else {
                Box::new(Cursor::new(b"bytes".to_vec()))
            };
        Ok(OpenedReader::new(info(request.path()), reader))
    }
    fn open_writer(
        &self,
        request: OpenWriterRequest<'_>,
    ) -> FsResult<OpenedWriter> {
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
        let opened_path = if matches!(
            self.response,
            CopyResponse::DeclinedWriterInvalidIdentity
        ) {
            path("/wrong")
        } else {
            request.path().clone()
        };
        Ok(OpenedWriter::new(
            info(&opened_path),
            Box::new(RecordingWriter {
                bytes: Arc::clone(&self.bytes),
                fail_flush: matches!(
                    self.response,
                    CopyResponse::DeclinedFlushFailure
                ),
                fail_write: matches!(
                    self.response,
                    CopyResponse::DeclinedWriteFailure
                ),
                commit_failure_state: match self.response {
                    CopyResponse::DeclinedCommitFailure => {
                        Some(qubit_fs::WriteFailureState::NotPublished)
                    }
                    CopyResponse::DeclinedCommitRetryable => {
                        Some(qubit_fs::WriteFailureState::RetryableNotPublished)
                    }
                    CopyResponse::DeclinedCommitPublished => {
                        Some(qubit_fs::WriteFailureState::Published)
                    }
                    CopyResponse::DeclinedCommitIndeterminate => {
                        Some(qubit_fs::WriteFailureState::Indeterminate)
                    }
                    _ => None,
                },
            }),
        ))
    }
    fn create_directory(
        &self,
        _: CreateDirectoryRequest<'_>,
    ) -> FsResult<CreateDirectoryOutcome> {
        Err(unused())
    }
    fn delete_file(&self, _: DeleteFileRequest<'_>) -> FsResult<DeleteOutcome> {
        Err(unused())
    }
    fn delete_directory(
        &self,
        _: DeleteDirectoryRequest<'_>,
    ) -> FsResult<DeleteOutcome> {
        Err(unused())
    }
    fn try_copy(
        &self,
        _: CopyRequest<'_>,
    ) -> Result<CopyAttempt, SpiCopyFailure> {
        self.calls
            .lock()
            .expect("calls lock should succeed")
            .push("try_copy");
        match self.response {
            CopyResponse::Completed => {
                Ok(CopyAttempt::Completed(CopyOutcome::new(
                    CopyStats::default(),
                    CopyMethod::Native,
                    AchievedAtomicity::Atomic,
                )))
            }
            CopyResponse::CompletedAtomicDowngrade => {
                Ok(CopyAttempt::Completed(CopyOutcome::new(
                    CopyStats::default(),
                    CopyMethod::Native,
                    AchievedAtomicity::NonAtomic,
                )))
            }
            CopyResponse::CompletedDurabilityDowngrade => {
                Ok(CopyAttempt::Completed(CopyOutcome::new(
                    CopyStats::default(),
                    CopyMethod::Native,
                    AchievedAtomicity::Atomic,
                )))
            }
            CopyResponse::CompletedServerSideRequiredButNative
            | CopyResponse::CompletedMetadataDowngrade => {
                Ok(CopyAttempt::Completed(CopyOutcome::new(
                    CopyStats::default(),
                    CopyMethod::Native,
                    AchievedAtomicity::Atomic,
                )))
            }
            CopyResponse::CompletedServerSideWhenDisabled => {
                Ok(CopyAttempt::Completed(CopyOutcome::new(
                    CopyStats::default(),
                    CopyMethod::ServerSide,
                    AchievedAtomicity::Atomic,
                )))
            }
            CopyResponse::CompletedInvalidSkippedStats => {
                Ok(CopyAttempt::Completed(CopyOutcome::new(
                    CopyStats {
                        skipped: 1,
                        ..CopyStats::default()
                    },
                    CopyMethod::Native,
                    AchievedAtomicity::Atomic,
                )))
            }
            CopyResponse::CompletedInvalidFailedStats => {
                Ok(CopyAttempt::Completed(CopyOutcome::new(
                    CopyStats {
                        failed: 1,
                        ..CopyStats::default()
                    },
                    CopyMethod::Native,
                    AchievedAtomicity::Atomic,
                )))
            }
            CopyResponse::CompletedInvalidOverwrittenStats => {
                Ok(CopyAttempt::Completed(CopyOutcome::new(
                    CopyStats {
                        overwritten: 1,
                        ..CopyStats::default()
                    },
                    CopyMethod::Native,
                    AchievedAtomicity::Atomic,
                )))
            }
            CopyResponse::CompletedStreamedOutcome => {
                Ok(CopyAttempt::Completed(CopyOutcome::new(
                    CopyStats::default(),
                    CopyMethod::Streamed,
                    AchievedAtomicity::Atomic,
                )))
            }
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
            | CopyResponse::DeclinedCommitIndeterminate => {
                Ok(CopyAttempt::Declined(CopyDeclineReason::NotApplicable))
            }
            CopyResponse::Failed => Err(SpiCopyFailure::new(
                FsError::new(
                    FsErrorKind::Io,
                    FsOperation::BeginCopy,
                    "injected",
                ),
                CopyFailureState::Indeterminate,
                CopyStats::default(),
            )),
        }
    }
    fn rename(
        &self,
        _: RenameRequest<'_>,
    ) -> Result<RenameOutcome, SpiRenameFailure> {
        Err(SpiRenameFailure::new(
            unused(),
            RenameFailureState::Unchanged,
        ))
    }
    fn create_temp_file(
        &self,
        _: CreateTempFileRequest,
    ) -> FsResult<OpenedTempFile> {
        Err(unused())
    }
    fn create_temp_directory(
        &self,
        _: CreateTempDirectoryRequest,
    ) -> FsResult<OpenedTempDirectory> {
        Err(unused())
    }
}
/// Returns a provider identity bound to `path`.
fn info(path: &Path) -> OpenedFileInfo {
    OpenedFileInfo::new(
        FileSystemId::new("recording").expect("id should be valid"),
        path.clone(),
    )
}
/// Returns an unused-operation provider error.
fn unused() -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedOperation,
        FsOperation::Other,
        "unused",
    )
}
/// Captures fallback bytes through the writer SPI.
struct RecordingWriter {
    bytes: Arc<Mutex<Vec<u8>>>,
    fail_flush: bool,
    fail_write: bool,
    commit_failure_state: Option<qubit_fs::WriteFailureState>,
}

/// Reports a deterministic failure from the fallback reader after its writer
/// has been opened.
struct FailingReader;

impl Input for FailingReader {
    type Item = u8;

    unsafe fn read_unchecked(
        &mut self,
        _: &mut [u8],
        _: usize,
        _: usize,
    ) -> IoResult<usize> {
        Err(std::io::Error::other("injected read failure"))
    }
}
/// Implements byte output for the recording writer.
impl Output for RecordingWriter {
    type Item = u8;
    unsafe fn write_unchecked(
        &mut self,
        buffer: &[u8],
        _: usize,
        count: usize,
    ) -> IoResult<usize> {
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
    fn commit(
        &mut self,
    ) -> Result<WriteOutcome, qubit_fs::spi::SpiWriteFailure> {
        if let Some(state) = self.commit_failure_state {
            return Err(qubit_fs::spi::SpiWriteFailure::new(
                FsError::new(
                    FsErrorKind::Io,
                    FsOperation::CommitWriter,
                    "injected commit failure",
                ),
                state,
            ));
        }
        Ok(WriteOutcome::new(
            AchievedAtomicity::NonAtomic,
            qubit_fs::PublicationMethod::StreamCopy,
        ))
    }
    fn abort(&mut self) -> FsResult<qubit_fs::WriteAbortOutcome> {
        Ok(qubit_fs::WriteAbortOutcome::NotPublished)
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
    let (filesystem, calls, bytes) =
        recording_filesystem(CopyResponse::Declined);
    let outcome = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect("safe fallback should succeed");
    assert_eq!(CopyMethod::Streamed, outcome.method());
    assert!(outcome.used_fallback());
    assert_eq!(
        b"bytes",
        bytes.lock().expect("bytes lock should succeed").as_slice()
    );
    assert_eq!(
        ["try_copy", "stat", "open_reader", "open_writer"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Uses the facade stream fallback when the provider does not advertise copy.
#[test]
fn test_copy_fallback_does_not_require_copy_capability() {
    let (filesystem, calls, _) =
        recording_filesystem_without_copy(CopyResponse::Declined);
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
            CopyOptions::default()
                .with_server_side(ServerSidePreference::Prefer),
        )
        .expect("preferred server-side copy may fall back");

    assert_eq!(CopyMethod::Streamed, outcome.method());
    assert!(outcome.used_fallback());
    assert_eq!(
        b"bytes",
        bytes.lock().expect("bytes lock should succeed").as_slice()
    );
}

/// Rejects copy options the synchronous stream fallback cannot faithfully
/// implement, without opening source or destination handles.
#[test]
fn test_copy_declined_rejects_incompatible_fallback_options() {
    let options = [
        CopyOptions::default().with_continue_on_error(true),
        CopyOptions::default()
            .with_preserve_metadata(MetadataPreservePolicy::Portable),
        CopyOptions::default().with_create_parent(true),
        CopyOptions::default()
            .with_conflict(qubit_fs::CopyConflictPolicy::Overwrite),
    ];
    for options in options {
        let (filesystem, calls, _) =
            recording_filesystem(CopyResponse::Declined);
        let failure = filesystem
            .copy(&path("/source"), &path("/target"), options)
            .expect_err(
                "declined copy must reject an incompatible fallback option",
            );
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
    let (reader_filesystem, _, _) =
        recording_filesystem(CopyResponse::DeclinedReaderFailure);
    let reader = reader_filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("reader-open failure should stop before publication");
    assert_eq!(CopyFailureState::Unchanged, reader.state());

    let (filesystem, _, _) =
        recording_filesystem(CopyResponse::DeclinedWriterAlreadyExists);
    let skipped = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default()
                .with_conflict(qubit_fs::CopyConflictPolicy::Skip),
        )
        .expect("an existing target should be skipped when explicitly allowed");
    assert_eq!(1, skipped.stats().skipped);

    for (response, expected, writer_state) in [
        (
            CopyResponse::DeclinedReadFailure,
            CopyFailureState::Unchanged,
            qubit_fs::WriterState::Open,
        ),
        (
            CopyResponse::DeclinedWriteFailure,
            CopyFailureState::Indeterminate,
            qubit_fs::WriterState::Indeterminate,
        ),
        (
            CopyResponse::DeclinedFlushFailure,
            CopyFailureState::Indeterminate,
            qubit_fs::WriterState::Indeterminate,
        ),
        (
            CopyResponse::DeclinedCommitFailure,
            CopyFailureState::Unchanged,
            qubit_fs::WriterState::NotPublished,
        ),
        (
            CopyResponse::DeclinedCommitRetryable,
            CopyFailureState::Unchanged,
            qubit_fs::WriterState::Open,
        ),
        (
            CopyResponse::DeclinedCommitPublished,
            CopyFailureState::Published,
            qubit_fs::WriterState::Published,
        ),
        (
            CopyResponse::DeclinedCommitIndeterminate,
            CopyFailureState::Indeterminate,
            qubit_fs::WriterState::Indeterminate,
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
            writer
                .expect("post-open fallback failure retains its writer")
                .state()
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
    let (filesystem, _, _) =
        recording_filesystem(CopyResponse::DeclinedReadFailure);
    let source = path("/source");
    let error = filesystem
        .read_all(&source, qubit_fs::ReadOptions::default(), 16)
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
        .read_all(&path("/source"), qubit_fs::ReadOptions::default(), 2)
        .expect_err("read-all limit should reject oversized content");

    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(Some("recording"), error.provider());
}

/// Retains the public write-all failure type when the writer cannot be opened.
#[test]
fn test_write_all_wraps_writer_open_failure() {
    let (filesystem, _, _) =
        recording_filesystem(CopyResponse::DeclinedWriterFailure);
    let target = path("/target");
    let failure = filesystem
        .write_all(&target, b"bytes", qubit_fs::WriteOptions::default())
        .expect_err(
            "writer-open failure should use the write-all failure type",
        );
    assert_eq!(FsErrorKind::Io, failure.error().kind());
    assert!(failure.writer().is_none());
}

/// Includes provider context when the synchronous write-all limit is hit.
#[test]
fn test_write_all_limit_error_includes_provider_context() {
    let (filesystem, _, _) =
        recording_filesystem_with_write_limit(CopyResponse::Declined, 1);
    let failure = filesystem
        .write_all(
            &path("/target"),
            b"bytes",
            qubit_fs::WriteOptions::default(),
        )
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
            CopyOptions::default()
                .with_atomicity(AtomicityRequirement::Required),
        )
        .expect_err("missing atomic guarantee must fail locally");
    assert_eq!(CopyFailureState::Unchanged, failure.state());
    assert_eq!(
        Some(qubit_fs::FileSystemCapability::AtomicFileCopy),
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
            CopyOptions::default()
                .with_durability(qubit_fs::DurabilityRequirement::Required),
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
    let (filesystem, calls, _) =
        recording_filesystem(CopyResponse::CompletedAtomicDowngrade);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default()
                .with_atomicity(AtomicityRequirement::Required),
        )
        .expect_err("downgrade must fail");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert_eq!(
        ["try_copy"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies declared durability cannot mask a completed non-durable provider
/// outcome.
#[test]
fn test_copy_completed_durability_downgrade_is_contract_failure() {
    let (filesystem, calls, _) =
        recording_filesystem(CopyResponse::CompletedDurabilityDowngrade);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default()
                .with_durability(qubit_fs::DurabilityRequirement::Required),
        )
        .expect_err("durability downgrade must fail");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert_eq!(
        ["try_copy"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies a completed result cannot claim to satisfy required server-side
/// copy when its reported method is not server-side.
#[test]
fn test_copy_completed_non_server_side_method_violates_required_server_side() {
    let (filesystem, calls, _) = recording_filesystem(
        CopyResponse::CompletedServerSideRequiredButNative,
    );
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default()
                .with_server_side(ServerSidePreference::Require),
        )
        .expect_err("native result cannot satisfy required server-side copy");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert_eq!(
        ["try_copy"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies a provider cannot ignore an explicit request to avoid server-side
/// copy while still reporting a server-side completed outcome.
#[test]
fn test_copy_completed_server_side_method_violates_disabled_preference() {
    let (filesystem, calls, _) =
        recording_filesystem(CopyResponse::CompletedServerSideWhenDisabled);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default()
                .with_server_side(ServerSidePreference::Disable),
        )
        .expect_err("server-side outcome must honor the disabled preference");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert_eq!(
        ["try_copy"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies a provider-completed result reports the requested metadata
/// preservation fact rather than silently returning its default none value.
#[test]
fn test_copy_completed_missing_metadata_preservation_is_contract_failure() {
    let (filesystem, calls, _) =
        recording_filesystem(CopyResponse::CompletedMetadataDowngrade);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default()
                .with_preserve_metadata(MetadataPreservePolicy::Portable),
        )
        .expect_err("missing metadata preservation must be rejected");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert_eq!(
        ["try_copy"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies a successful copy cannot report skipped entries under a
/// fail-on-conflict request.
#[test]
fn test_copy_completed_skipped_stats_violate_fail_conflict_policy() {
    let (filesystem, _, _) =
        recording_filesystem(CopyResponse::CompletedInvalidSkippedStats);
    let failure = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("skipped stats must match the conflict policy");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
}

/// Verifies a native provider result cannot report failed entries unless the
/// caller explicitly accepted continued errors.
#[test]
fn test_copy_completed_failed_stats_violate_stop_on_error_policy() {
    let (filesystem, _, _) =
        recording_filesystem(CopyResponse::CompletedInvalidFailedStats);
    let failure = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("failed stats must match the continue-on-error policy");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
}

/// Verifies a native provider result cannot report overwritten entries unless
/// the request selected overwrite conflict handling.
#[test]
fn test_copy_completed_overwritten_stats_violate_fail_conflict_policy() {
    let (filesystem, _, _) =
        recording_filesystem(CopyResponse::CompletedInvalidOverwrittenStats);
    let failure = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("overwritten stats must match the conflict policy");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
}

/// Verifies only the facade may return a streamed fallback outcome; providers
/// must not present it as a native fast-path completion.
#[test]
fn test_copy_completed_streamed_outcome_violates_native_contract() {
    let (filesystem, _, _) =
        recording_filesystem(CopyResponse::CompletedStreamedOutcome);
    let failure = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("providers must not return facade fallback outcomes");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
}

/// Verifies a declined atomic skip request opens neither fallback stream
/// handle.
#[test]
fn test_copy_declined_skip_required_atomicity_rejects_without_opening_handles()
{
    let (filesystem, calls, _) =
        recording_filesystem(CopyResponse::DeclinedSkipAtomic);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default()
                .with_conflict(qubit_fs::CopyConflictPolicy::Skip)
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
    let (filesystem, _, _) =
        recording_filesystem(CopyResponse::DeclinedFlushFailure);
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
            std::error::Error::source(&failure)
                .expect("failure should expose its source")
        )
    );
    assert_eq!(
        qubit_fs::WriterState::Indeterminate,
        failure
            .writer()
            .expect("post-open fallback failure retains its writer")
            .state()
    );
    assert_eq!(
        qubit_fs::WriterState::Indeterminate,
        failure
            .writer_mut()
            .expect("fallback writer should still be recoverable")
            .state()
    );
    assert_eq!(
        qubit_fs::WriterState::Indeterminate,
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
