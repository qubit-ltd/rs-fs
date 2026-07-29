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
    DeclinedCommitPublished,
    DeclinedCommitIndeterminate,
}
/// Records facade-to-provider calls and stream-fallback bytes.
struct RecordingSpi {
    response: CopyResponse,
    calls: Arc<Mutex<Vec<&'static str>>>,
    bytes: Arc<Mutex<Vec<u8>>>,
}
/// Constructs a recording filesystem with a selected fast-path response.
#[allow(clippy::type_complexity)]
fn recording_filesystem(
    response: CopyResponse,
) -> (
    FileSystem,
    Arc<Mutex<Vec<&'static str>>>,
    Arc<Mutex<Vec<u8>>>,
) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let filesystem = FileSystem::from_spi(RecordingSpi {
        response,
        calls: Arc::clone(&calls),
        bytes: Arc::clone(&bytes),
    })
    .expect("recording facade should construct");
    (filesystem, calls, bytes)
}
/// Builds properties sufficient for native copy, stream fallback, and rename
/// tests.
fn properties(response: &CopyResponse) -> FileSystemProperties {
    let mut capabilities = FileSystemCapabilities::new()
        .with(FileSystemCapability::Copy)
        .with(FileSystemCapability::Rename)
        .with(FileSystemCapability::AtomicRename);
    if !matches!(response, CopyResponse::DeclinedWithoutRead) {
        capabilities = capabilities.with(FileSystemCapability::Read);
    }
    if !matches!(response, CopyResponse::DeclinedWithoutWrite) {
        capabilities = capabilities.with(FileSystemCapability::Write);
    }
    if matches!(
        response,
        CopyResponse::CompletedAtomicDowngrade
            | CopyResponse::DeclinedSkipAtomic
    ) {
        capabilities = capabilities.with(FileSystemCapability::AtomicReplace);
    }
    if matches!(response, CopyResponse::CompletedDurabilityDowngrade) {
        capabilities = capabilities.with(FileSystemCapability::DurableCopy);
    }
    if matches!(response, CopyResponse::CompletedServerSideRequiredButNative) {
        capabilities = capabilities.with(FileSystemCapability::ServerSideCopy);
    }
    FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("recording").expect("id should be valid"),
            "recording",
            qubit_fs::PathSemantics::Hierarchical,
        ),
        capabilities,
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
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
        properties(&self.response)
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
            metadata.kind = FileKind::Directory;
        }
        metadata.len = Some(5);
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
    fn abort(&mut self) -> FsResult<()> {
        Ok(())
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

/// Verifies a preferred server-side attempt may use the safe stream fallback.
#[test]
fn test_copy_declined_with_preferred_server_side_uses_stream_fallback() {
    let (filesystem, _, bytes) = recording_filesystem(CopyResponse::Declined);
    let outcome = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions {
                server_side: ServerSidePreference::Prefer,
                ..CopyOptions::default()
            },
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
        CopyOptions {
            continue_on_error: true,
            ..CopyOptions::default()
        },
        CopyOptions {
            preserve_metadata: MetadataPreservePolicy::Portable,
            ..CopyOptions::default()
        },
        CopyOptions {
            create_parent: true,
            ..CopyOptions::default()
        },
        CopyOptions {
            conflict: qubit_fs::CopyConflictPolicy::Overwrite,
            ..CopyOptions::default()
        },
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
            CopyOptions {
                conflict: qubit_fs::CopyConflictPolicy::Skip,
                ..CopyOptions::default()
            },
        )
        .expect("an existing target should be skipped when explicitly allowed");
    assert_eq!(1, skipped.stats().skipped);

    for response in [
        CopyResponse::DeclinedWriterFailure,
        CopyResponse::DeclinedWriterInvalidIdentity,
        CopyResponse::DeclinedReadFailure,
        CopyResponse::DeclinedWriteFailure,
        CopyResponse::DeclinedCommitFailure,
        CopyResponse::DeclinedCommitPublished,
        CopyResponse::DeclinedCommitIndeterminate,
    ] {
        let (filesystem, _, _) = recording_filesystem(response);
        let failure = filesystem
            .copy(&path("/source"), &path("/target"), CopyOptions::default())
            .expect_err("fallback writer failure should preserve recovery");
        assert!(
            matches!(
                failure.state(),
                CopyFailureState::Unchanged
                    | CopyFailureState::PartiallyPublished
                    | CopyFailureState::Published
                    | CopyFailureState::Indeterminate
            ),
            "failure state must reflect whether a writer was opened"
        );
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
            CopyOptions {
                atomicity: AtomicityRequirement::Required,
                ..CopyOptions::default()
            },
        )
        .expect_err("missing atomic guarantee must fail locally");
    assert_eq!(CopyFailureState::Unchanged, failure.state());
    assert_eq!(
        Some(qubit_fs::FileSystemCapability::AtomicReplace),
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
            CopyOptions {
                durability: qubit_fs::DurabilityRequirement::Required,
                ..CopyOptions::default()
            },
        )
        .expect_err("missing durability guarantee must fail locally");
    assert_eq!(CopyFailureState::Unchanged, failure.state());
    assert_eq!(
        Some(FileSystemCapability::DurableCopy),
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
            CopyOptions {
                atomicity: AtomicityRequirement::Required,
                ..CopyOptions::default()
            },
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
            CopyOptions {
                durability: qubit_fs::DurabilityRequirement::Required,
                ..CopyOptions::default()
            },
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
            CopyOptions {
                server_side: ServerSidePreference::Require,
                ..CopyOptions::default()
            },
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
            CopyOptions {
                preserve_metadata: MetadataPreservePolicy::Portable,
                ..CopyOptions::default()
            },
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
            CopyOptions {
                conflict: qubit_fs::CopyConflictPolicy::Skip,
                atomicity: AtomicityRequirement::Required,
                ..CopyOptions::default()
            },
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
fn test_copy_fallback_flush_failure_is_partially_published_with_stats_and_writer()
 {
    let (filesystem, _, _) =
        recording_filesystem(CopyResponse::DeclinedFlushFailure);
    let failure = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("flush failure should be recoverable");
    assert_eq!(CopyFailureState::PartiallyPublished, failure.state());
    assert_eq!(5, failure.partial_stats().bytes);
    assert!(failure.has_writer());
    assert!(format!("{failure:?}").contains("CopyFailure"));

    let (error, state, stats, writer) = failure.into_parts();
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(CopyFailureState::PartiallyPublished, state);
    assert_eq!(5, stats.bytes);
    assert!(writer.is_some());
}
