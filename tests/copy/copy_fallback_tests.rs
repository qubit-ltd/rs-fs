// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Recording-provider coverage for copy dispatch and fallback.

use std::io::{Cursor, Result as IoResult};
use std::sync::{Arc, Mutex};

use qubit_fs::spi::{
    CopyAttempt, CopyDeclineReason, CopyRequest, CreateDirectoryRequest,
    CreateTempDirectoryRequest, CreateTempFileRequest, DeleteDirectoryRequest, DeleteFileRequest,
    FileSystemSpi, FileWriterSpi, ListRequest, OpenReaderRequest, OpenWriterRequest,
    OpenedDirectoryStream, OpenedReader, OpenedTempDirectory, OpenedTempFile, OpenedWriter,
    RenameRequest, SpiCopyFailure, SpiRenameFailure, StatRequest, StatResponse,
};
use qubit_fs::{
    AchievedAtomicity, AtomicityRequirement, CopyFailureState, CopyMethod, CopyOptions,
    CopyOutcome, CopyStats, CreateDirectoryOutcome, DeleteOutcome, FileKind, FileMetadata,
    FileSystem, FileSystemCapabilities, FileSystemCapability, FileSystemId, FileSystemInfo,
    FileSystemLimits, FileSystemProperties, FsError, FsErrorKind, FsOperation, FsResult,
    MetadataPreservePolicy, OpenedFileInfo, Path, PathConstraints, RenameFailureState,
    RenameOutcome, ServerSidePreference, WriteOutcome,
};
use qubit_io::Output;

/// Selects the provider's response to the copy fast path.
enum CopyResponse {
    Completed,
    CompletedAtomicDowngrade,
    CompletedDurabilityDowngrade,
    CompletedServerSideRequiredButNative,
    CompletedMetadataDowngrade,
    CompletedInvalidSkippedStats,
    Declined,
    DeclinedSkipAtomic,
    Failed,
    DeclinedFlushFailure,
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
        .with(FileSystemCapability::Read)
        .with(FileSystemCapability::Write)
        .with(FileSystemCapability::Rename)
        .with(FileSystemCapability::AtomicRename);
    if matches!(
        response,
        CopyResponse::CompletedAtomicDowngrade | CopyResponse::DeclinedSkipAtomic
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
        let mut metadata = FileMetadata::new(FileKind::File);
        metadata.len = Some(5);
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
        Ok(OpenedReader::new(
            info(request.path()),
            Box::new(Cursor::new(b"bytes".to_vec())),
        ))
    }
    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        self.calls
            .lock()
            .expect("calls lock should succeed")
            .push("open_writer");
        Ok(OpenedWriter::new(
            info(request.path()),
            Box::new(RecordingWriter {
                bytes: Arc::clone(&self.bytes),
                fail_flush: matches!(self.response, CopyResponse::DeclinedFlushFailure),
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
        self.calls
            .lock()
            .expect("calls lock should succeed")
            .push("try_copy");
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
            CopyResponse::Declined
            | CopyResponse::DeclinedSkipAtomic
            | CopyResponse::DeclinedFlushFailure => {
                Ok(CopyAttempt::Declined(CopyDeclineReason::NotApplicable))
            }
            CopyResponse::Failed => Err(SpiCopyFailure::new(
                FsError::new(FsErrorKind::Io, FsOperation::BeginCopy, "injected"),
                CopyFailureState::Indeterminate,
                CopyStats::default(),
            )),
        }
    }
    fn rename(&self, _: RenameRequest<'_>) -> Result<RenameOutcome, SpiRenameFailure> {
        Err(SpiRenameFailure::new(
            unused(),
            RenameFailureState::Unchanged,
        ))
    }
    fn create_temp_file(&self, _: CreateTempFileRequest) -> FsResult<OpenedTempFile> {
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
}
/// Implements byte output for the recording writer.
impl Output for RecordingWriter {
    type Item = u8;
    unsafe fn write_unchecked(&mut self, buffer: &[u8], _: usize, count: usize) -> IoResult<usize> {
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
    fn commit(&mut self) -> Result<WriteOutcome, qubit_fs::spi::SpiWriteFailure> {
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
    let (filesystem, calls, bytes) = recording_filesystem(CopyResponse::Declined);
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
    let (filesystem, calls, _) = recording_filesystem(CopyResponse::CompletedAtomicDowngrade);
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
    let (filesystem, calls, _) = recording_filesystem(CopyResponse::CompletedDurabilityDowngrade);
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
    let (filesystem, calls, _) =
        recording_filesystem(CopyResponse::CompletedServerSideRequiredButNative);
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
    let (filesystem, calls, _) = recording_filesystem(CopyResponse::CompletedMetadataDowngrade);
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
    let (filesystem, _, _) = recording_filesystem(CopyResponse::CompletedInvalidSkippedStats);
    let failure = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("skipped stats must match the conflict policy");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
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
fn test_copy_fallback_flush_failure_is_partially_published_with_stats_and_writer() {
    let (filesystem, _, _) = recording_filesystem(CopyResponse::DeclinedFlushFailure);
    let failure = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .expect_err("flush failure should be recoverable");
    assert_eq!(CopyFailureState::PartiallyPublished, failure.state());
    assert_eq!(5, failure.partial_stats().bytes);
    assert!(failure.has_writer());
}
