// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! External pending, failure, and rename coverage for the async facade.

#![cfg(feature = "async")]

#[path = "common/async_recording_spi.rs"]
mod async_recording_spi;
#[path = "common/poll_support.rs"]
mod poll_support;

use crate::async_recording_spi::{
    AsyncCopyStage,
    AsyncRecordingConfig,
    async_recording_file_system,
};
use crate::poll_support::{
    assert_pending,
    ready,
};
use qubit_fs::spi::{
    AsyncFileSystemSpi,
    AsyncFileWriteSession,
    CopyAttempt,
    CopyDeclineReason,
    CopyRequest,
    CreateDirectoryRequest,
    CreateTempDirectoryRequest,
    CreateTempFileRequest,
    DeleteDirectoryRequest,
    DeleteFileRequest,
    ListRequest,
    OpenReaderRequest,
    OpenWriterRequest,
    OpenedAsyncDirectoryStream,
    OpenedAsyncReader,
    OpenedAsyncTempDirectory,
    OpenedAsyncTempFile,
    OpenedAsyncWriter,
    RenameRequest,
    SpiCopyFailure,
    SpiFuture,
    SpiRenameFailure,
    StatRequest,
    StatResponse,
};
use qubit_fs::{
    AchievedAtomicity,
    AsyncFileSystem,
    AtomicityRequirement,
    CopyConflictPolicy,
    CopyFailureState,
    CopyOptions,
    CreateDirectoryOptions,
    DeleteOptions,
    DurabilityRequirement,
    FileKind,
    FileMetadata,
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
    ListOptions,
    OpenedFileInfo,
    Path,
    PathConstraints,
    PathSemantics,
    PersistOptions,
    PublicationMethod,
    ReadOptions,
    RenameFailureState,
    RenameOptions,
    RenameOutcome,
    WriteFailure,
    WriteFailureState,
    WriteOptions,
    WriteOutcome,
};
use qubit_io::{
    AsyncInput,
    AsyncOutput,
};
use std::{
    io::Result as IoResult,
    pin::Pin,
    task::Poll,
};

/// Parses one stable test path.
fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}

/// Verifies the asynchronous whole-file convenience API mirrors writer
/// publication semantics and preserves the provider operation order.
#[test]
fn test_async_write_all_publishes_complete_bytes() {
    let config = AsyncRecordingConfig {
        writer_atomicity: Some(AchievedAtomicity::Atomic),
        ..AsyncRecordingConfig::default()
    };
    let (filesystem, probe) = async_recording_file_system(config);
    let outcome = ready(filesystem.write_all(
        &path("/async-write-all"),
        b"bytes",
        WriteOptions::default(),
    ))
    .expect("async write-all should commit");
    assert_eq!(AchievedAtomicity::Atomic, outcome.atomicity());
    assert_eq!(vec!["open_writer"], probe.calls(),);
}

/// Verifies a failed asynchronous commit retains the writer for recovery.
#[test]
fn test_async_write_all_commit_failure_retains_writer() {
    let config = AsyncRecordingConfig {
        writer_commit_failure: Some(WriteFailureState::NotPublished),
        ..AsyncRecordingConfig::default()
    };
    let (filesystem, _) = async_recording_file_system(config);
    let failure = ready(filesystem.write_all(
        &path("/async-write-all-failure"),
        b"bytes",
        WriteOptions::default(),
    ))
    .expect_err("injected commit failure should retain writer");
    assert_eq!(FsErrorKind::Io, failure.error().kind());
    assert!(failure.writer().is_some());
}

/// Includes provider context when the asynchronous write-all limit is hit.
#[test]
fn test_async_write_all_limit_error_includes_provider_context() {
    let config = AsyncRecordingConfig {
        maximum_write_bytes: Some(1),
        ..AsyncRecordingConfig::default()
    };
    let (filesystem, _) = async_recording_file_system(config);
    let failure = ready(filesystem.write_all(
        &path("/async-write-all-limit"),
        b"bytes",
        WriteOptions::default(),
    ))
    .expect_err("async write-all limit should reject oversized content");

    assert_eq!(FsErrorKind::ResourceLimitExceeded, failure.error().kind());
    assert_eq!(Some("async-recording"), failure.error().provider());
}

#[derive(Clone, Copy)]
struct StreamCopyFallbackSpi {
    commit_already_exists: bool,
    stat_error: Option<FsErrorKind>,
}

impl StreamCopyFallbackSpi {
    fn unavailable(operation: FsOperation) -> FsError {
        FsError::new(
            FsErrorKind::UnsupportedOperation,
            operation,
            "async test SPI does not implement this operation",
        )
    }
    fn new(
        commit_already_exists: bool,
        stat_error: Option<FsErrorKind>,
    ) -> Self {
        Self {
            commit_already_exists,
            stat_error,
        }
    }
    fn file_info(&self, path: &Path) -> OpenedFileInfo {
        OpenedFileInfo::new(
            FileSystemId::new("async-fallback-review")
                .expect("test provider id should be valid"),
            path.clone(),
        )
    }
    fn properties(&self) -> FileSystemProperties {
        let capabilities = FileSystemCapabilities::new()
            .with(FileSystemCapability::Copy)
            .with(FileSystemCapability::Read)
            .with(FileSystemCapability::Write);
        FileSystemProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("async-fallback-review")
                    .expect("test provider id should be valid"),
                "async-fallback-review",
                PathSemantics::Hierarchical,
            ),
            capabilities,
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
        )
        .expect("test properties should be valid")
    }
}

impl AsyncFileSystemSpi for StreamCopyFallbackSpi {
    fn properties(&self) -> FileSystemProperties {
        self.properties()
    }
    fn stat<'a>(
        &'a self,
        request: StatRequest<'a>,
    ) -> SpiFuture<'a, FsResult<StatResponse>> {
        if let Some(kind) = self.stat_error {
            return Box::pin(async move {
                Err(FsError::new(
                    kind,
                    FsOperation::Stat,
                    "injected stat failure",
                ))
            });
        }
        let path = request.path().clone();
        let mut metadata = FileMetadata::new(FileKind::File);
        metadata = metadata.with_len(Some(5));
        Box::pin(async move { Ok(StatResponse::new(path, metadata)) })
    }

    fn list<'a>(
        &'a self,
        _: ListRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncDirectoryStream>> {
        Box::pin(async { Err(Self::unavailable(FsOperation::List)) })
    }
    fn open_reader<'a>(
        &'a self,
        request: OpenReaderRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
        let _ = request.options();
        let info = self.file_info(request.path());
        Box::pin(async move {
            Ok(OpenedAsyncReader::new(info, Box::new(ZeroReader)))
        })
    }
    fn open_writer<'a>(
        &'a self,
        request: OpenWriterRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
        let _ = request.options();
        let info = self.file_info(request.path());
        let session = CommitWriter::new(self.commit_already_exists);
        Box::pin(
            async move { Ok(OpenedAsyncWriter::new(info, Box::new(session))) },
        )
    }
    fn create_directory<'a>(
        &'a self,
        _: CreateDirectoryRequest<'a>,
    ) -> SpiFuture<'a, FsResult<qubit_fs::CreateDirectoryOutcome>> {
        Box::pin(async { Err(Self::unavailable(FsOperation::CreateDir)) })
    }
    fn delete_file<'a>(
        &'a self,
        _: DeleteFileRequest<'a>,
    ) -> SpiFuture<'a, FsResult<qubit_fs::DeleteOutcome>> {
        Box::pin(async { Err(Self::unavailable(FsOperation::Delete)) })
    }
    fn delete_directory<'a>(
        &'a self,
        _: DeleteDirectoryRequest<'a>,
    ) -> SpiFuture<'a, FsResult<qubit_fs::DeleteOutcome>> {
        Box::pin(async { Err(Self::unavailable(FsOperation::Delete)) })
    }
    fn try_copy<'a>(
        &'a self,
        _: CopyRequest<'a>,
    ) -> SpiFuture<'a, Result<CopyAttempt, SpiCopyFailure>> {
        Box::pin(async {
            Ok(CopyAttempt::Declined(CopyDeclineReason::NotApplicable))
        })
    }
    fn rename<'a>(
        &'a self,
        _: RenameRequest<'a>,
    ) -> SpiFuture<'a, Result<RenameOutcome, SpiRenameFailure>> {
        Box::pin(async {
            Err(SpiRenameFailure::new(
                Self::unavailable(FsOperation::Rename),
                RenameFailureState::Indeterminate,
            ))
        })
    }
    fn create_temp_file<'a>(
        &'a self,
        _: CreateTempFileRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempFile>> {
        Box::pin(async { Err(Self::unavailable(FsOperation::CreateTemp)) })
    }
    fn create_temp_directory<'a>(
        &'a self,
        _: CreateTempDirectoryRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempDirectory>> {
        Box::pin(async { Err(Self::unavailable(FsOperation::CreateTemp)) })
    }
}

struct ZeroReader;
impl AsyncInput for ZeroReader {
    type Item = u8;

    unsafe fn poll_read_unchecked(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &mut [u8],
        _: usize,
        _: usize,
    ) -> Poll<IoResult<usize>> {
        let _ = self;
        Poll::Ready(Ok(0))
    }
}

struct CommitWriter {
    commit_already_exists: bool,
}

impl CommitWriter {
    fn new(commit_already_exists: bool) -> Self {
        Self {
            commit_already_exists,
        }
    }
}

impl AsyncOutput for CommitWriter {
    type Item = u8;

    unsafe fn poll_write_unchecked(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &[u8],
        _: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        let _ = self;
        Poll::Ready(Ok(count))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> Poll<IoResult<()>> {
        let _ = self;
        Poll::Ready(Ok(()))
    }
}

impl AsyncFileWriteSession for CommitWriter {
    fn commit_async<'a>(
        self: Pin<&'a mut Self>,
    ) -> SpiFuture<'a, Result<WriteOutcome, WriteFailure>> {
        let commit_already_exists = self.commit_already_exists;
        Box::pin(async move {
            if commit_already_exists {
                return Err(WriteFailure::new(
                    FsError::new(
                        FsErrorKind::AlreadyExists,
                        FsOperation::CommitWriter,
                        "injected commit refusal",
                    ),
                    WriteFailureState::NotPublished,
                ));
            }
            Ok(WriteOutcome::new(
                AchievedAtomicity::NonAtomic,
                PublicationMethod::StreamCopy,
            ))
        })
    }

    fn abort_async<'a>(
        self: Pin<&'a mut Self>,
    ) -> SpiFuture<'a, FsResult<qubit_fs::WriteAbortOutcome>> {
        let _ = self;
        Box::pin(async { Ok(qubit_fs::WriteAbortOutcome::NotPublished) })
    }
}

/// Covers `exists` mapping for non-`NotFound` provider failures.
#[test]
fn test_async_facade_exists_contextualizes_other_errors() {
    let file_system = AsyncFileSystem::from_spi(StreamCopyFallbackSpi::new(
        false,
        Some(FsErrorKind::Io),
    ))
    .expect("test SPI should construct");
    let error = ready(file_system.exists(&path("/io-error")))
        .expect_err("exists should contextualize non-`NotFound` failures");
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(FsOperation::Exists, error.operation());
}

/// Covers the stream-copy commit `AlreadyExists` + `Skip` fallback-to-skip
/// path.
#[test]
fn test_async_facade_stream_copy_commit_already_exists_with_skip_marks_skipped()
{
    let file_system =
        AsyncFileSystem::from_spi(StreamCopyFallbackSpi::new(true, None))
            .expect("test SPI should construct");
    let mut operation = file_system
        .begin_copy(
            path("/source"),
            path("/target"),
            CopyOptions::default().with_conflict(CopyConflictPolicy::Skip),
        )
        .expect("copy preflight should succeed");
    let outcome = ready(operation.execute())
        .expect("commit refusal after stream write should skip");
    assert_eq!(1, outcome.stats().skipped);
}

/// Covers provider pending and failure propagation for facade I/O entry points.
#[test]
fn test_async_facade_stat_and_open_pending_and_error() {
    for stage in [
        AsyncCopyStage::Stat,
        AsyncCopyStage::OpenReader,
        AsyncCopyStage::OpenWriter,
    ] {
        let (fs, _) = async_recording_file_system(AsyncRecordingConfig {
            pending_stage: Some(stage),
            ..AsyncRecordingConfig::default()
        });
        match stage {
            AsyncCopyStage::Stat => {
                assert_pending(Box::pin(fs.stat(&path("/file"))).as_mut())
            }
            AsyncCopyStage::OpenReader => assert_pending(
                Box::pin(
                    fs.open_reader(&path("/file"), ReadOptions::default()),
                )
                .as_mut(),
            ),
            AsyncCopyStage::OpenWriter => assert_pending(
                Box::pin(
                    fs.open_writer(&path("/file"), WriteOptions::default()),
                )
                .as_mut(),
            ),
            _ => unreachable!(),
        }
        let (fs, _) = async_recording_file_system(AsyncRecordingConfig {
            failing_stage: Some(stage),
            ..AsyncRecordingConfig::default()
        });
        let error = match stage {
            AsyncCopyStage::Stat => ready(fs.stat(&path("/file")))
                .expect_err("provider failure expected"),
            AsyncCopyStage::OpenReader => {
                let Err(error) = ready(
                    fs.open_reader(&path("/file"), ReadOptions::default()),
                ) else {
                    panic!("provider failure expected");
                };
                error
            }
            AsyncCopyStage::OpenWriter => {
                let Err(error) = ready(
                    fs.open_writer(&path("/file"), WriteOptions::default()),
                ) else {
                    panic!("provider failure expected");
                };
                error
            }
            _ => unreachable!(),
        };
        assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    }
}

/// Covers rename preflight and result identity binding through the async
/// facade.
#[test]
fn test_async_rename_preflight_and_result_identity() {
    let (fs, probe) = async_recording_file_system(AsyncRecordingConfig {
        rename_atomicity: Some(AchievedAtomicity::Atomic),
        ..AsyncRecordingConfig::default()
    });
    let source = path("/source");
    let target = path("/target");
    let outcome = ready(fs.rename(&source, &target, RenameOptions::default()))
        .expect("rename should succeed");
    assert_eq!(&source, outcome.source());
    assert_eq!(&target, outcome.target());
    assert_eq!(vec!["rename"], probe.calls());
    let failure = ready(fs.rename(&source, &source, RenameOptions::default()))
        .expect_err("same path must fail locally");
    assert_eq!(FsErrorKind::InvalidOptions, failure.error().kind());
    assert_eq!(vec!["rename"], probe.calls());
}

/// Covers convenience reads, existence mapping, and response identity checks.
#[test]
fn test_async_facade_convenience_operations_enforce_contracts() {
    let (file_system, _) =
        async_recording_file_system(AsyncRecordingConfig::default());
    assert!(
        ready(file_system.exists(&path("/file")))
            .expect("existing path should be reported")
    );
    assert!(
        !ready(file_system.exists(&path("/missing")))
            .expect("missing path should be mapped to false")
    );
    assert_eq!(
        b"bytes",
        ready(file_system.read_all(&path("/file"), ReadOptions::default(), 5,))
            .expect("reader bytes should be collected")
            .as_slice()
    );
    let limit_error =
        ready(file_system.read_all(&path("/file"), ReadOptions::default(), 4))
            .expect_err("the byte cap should be enforced");
    assert_eq!(FsErrorKind::ResourceLimitExceeded, limit_error.kind());

    let read_error_file_system =
        async_recording_file_system(AsyncRecordingConfig {
            failing_stage: Some(AsyncCopyStage::ReaderRead),
            ..AsyncRecordingConfig::default()
        })
        .0;
    let read_error = ready(read_error_file_system.read_all(
        &path("/file"),
        ReadOptions::default(),
        5,
    ))
    .expect_err("reader failures should be contextualized");
    assert_eq!(FsErrorKind::Io, read_error.kind());

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        rename_atomicity: Some(AchievedAtomicity::Atomic),
        ..AsyncRecordingConfig::default()
    });
    let rename_error = ready(file_system.rename(
        &path("/source"),
        &path("/wrong-rename-target"),
        RenameOptions::default(),
    ))
    .expect_err("mismatched rename identities should be rejected");
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        rename_error.error().kind()
    );
}

/// Covers asynchronous facade contract failures and declined-copy boundary
/// failures that must remain typed and contextualized.
#[test]
fn test_async_facade_rejects_contract_and_fallback_boundary_failures() {
    let source = path("/source");
    let target = path("/target");

    for config in [
        AsyncRecordingConfig {
            rename_atomicity: Some(AchievedAtomicity::NonAtomic),
            ..AsyncRecordingConfig::default()
        },
        AsyncRecordingConfig {
            rename_atomicity: Some(AchievedAtomicity::Atomic),
            rename_copy_then_delete: true,
            ..AsyncRecordingConfig::default()
        },
    ] {
        let (file_system, _) = async_recording_file_system(config);
        let options = if file_system
            .properties()
            .capabilities()
            .contains(qubit_fs::FileSystemCapability::AtomicRename)
        {
            RenameOptions::default()
                .with_atomicity(AtomicityRequirement::Required)
        } else {
            RenameOptions::default()
        };
        let error = ready(file_system.rename(&source, &target, options))
            .expect_err("invalid provider rename outcome must be rejected");
        assert_eq!(
            FsErrorKind::ProviderContractViolation,
            error.error().kind()
        );
    }

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        invalid_temp_identity: true,
        temp_cleanup_failure: true,
        ..AsyncRecordingConfig::default()
    });
    let Err(file) = ready(
        file_system.create_temp_file(qubit_fs::TempFileOptions::default()),
    ) else {
        panic!("invalid temporary file identity must be rejected");
    };
    let Err(directory) = ready(
        file_system
            .create_temp_directory(qubit_fs::TempDirectoryOptions::default()),
    ) else {
        panic!("invalid temporary directory identity must be rejected");
    };
    for error in [file, directory] {
        assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    }

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        invalid_temp_path: true,
        ..AsyncRecordingConfig::default()
    });
    let Err(invalid_path) = ready(
        file_system.create_temp_file(qubit_fs::TempFileOptions::default()),
    ) else {
        panic!("relative temporary path must be rejected");
    };
    assert_eq!(FsErrorKind::ProviderContractViolation, invalid_path.kind());

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        completed_copy: Some(AchievedAtomicity::Atomic),
        ..AsyncRecordingConfig::default()
    });
    let mut operation = file_system
        .begin_copy(
            source.clone(),
            target.clone(),
            CopyOptions::default()
                .with_durability(qubit_fs::DurabilityRequirement::Required),
        )
        .expect("durable-copy capability should satisfy preflight");
    let durability = ready(operation.execute())
        .expect_err("non-durable provider outcome must be rejected");
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        durability.error().kind()
    );

    let (file_system, _) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let Err(same) = file_system.begin_copy(
        source.clone(),
        source.clone(),
        CopyOptions::default(),
    ) else {
        panic!("copy source and target must differ");
    };
    assert_eq!(FsErrorKind::InvalidOptions, same.error().kind());

    for config in [
        AsyncRecordingConfig {
            failing_stage: Some(AsyncCopyStage::Stat),
            ..AsyncRecordingConfig::default()
        },
        AsyncRecordingConfig {
            failing_stage: Some(AsyncCopyStage::OpenReader),
            ..AsyncRecordingConfig::default()
        },
        AsyncRecordingConfig {
            failing_stage: Some(AsyncCopyStage::OpenWriter),
            ..AsyncRecordingConfig::default()
        },
        AsyncRecordingConfig {
            stat_kind: Some(FileKind::Directory),
            ..AsyncRecordingConfig::default()
        },
    ] {
        let (file_system, _) = async_recording_file_system(config);
        let mut operation = file_system
            .begin_copy(source.clone(), target.clone(), CopyOptions::default())
            .expect("copy preflight should succeed");
        let failure = ready(operation.execute())
            .expect_err("declined fallback boundary failure must propagate");
        assert_eq!(CopyFailureState::Unchanged, failure.state());
    }

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        writer_open_error: Some(FsErrorKind::AlreadyExists),
        ..AsyncRecordingConfig::default()
    });
    let mut operation = file_system
        .begin_copy(
            source,
            target,
            CopyOptions::default().with_conflict(CopyConflictPolicy::Skip),
        )
        .expect("skip fallback preflight should succeed");
    let outcome =
        ready(operation.execute()).expect("existing target should be skipped");
    assert_eq!(1, outcome.stats().skipped);
}

/// Covers capability gates and path-semantics checks before provider I/O.
#[test]
fn test_async_facade_rejects_unsupported_capabilities_and_path_semantics() {
    let source = path("/source");
    let target = path("/target");

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::Read),
        ..AsyncRecordingConfig::default()
    });
    let error = ready(file_system.open_reader(&source, ReadOptions::default()))
        .expect_err("read capability must be required");
    assert_eq!(FsErrorKind::UnsupportedCapability, error.kind());

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::Write),
        ..AsyncRecordingConfig::default()
    });
    let error =
        ready(file_system.open_writer(&target, WriteOptions::default()))
            .expect_err("write capability must be required");
    assert_eq!(FsErrorKind::UnsupportedCapability, error.kind());

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::CreateDirectory),
        ..AsyncRecordingConfig::default()
    });
    let error = ready(
        file_system
            .create_directory(&target, CreateDirectoryOptions::default()),
    )
    .expect_err("directory-creation capability must be required");
    assert_eq!(FsErrorKind::UnsupportedCapability, error.kind());

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::TempFile),
        ..AsyncRecordingConfig::default()
    });
    let Err(error) = ready(
        file_system.create_temp_file(qubit_fs::TempFileOptions::default()),
    ) else {
        panic!("temporary-file capability must be required");
    };
    assert_eq!(FsErrorKind::UnsupportedCapability, error.kind());

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::TempDirectory),
        ..AsyncRecordingConfig::default()
    });
    let Err(error) = ready(
        file_system
            .create_temp_directory(qubit_fs::TempDirectoryOptions::default()),
    ) else {
        panic!("temporary-directory capability must be required");
    };
    assert_eq!(FsErrorKind::UnsupportedCapability, error.kind());

    let (file_system, _) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let error =
        ready(file_system.rename(&source, &target, RenameOptions::default()))
            .expect_err("rename capability must be required");
    assert_eq!(FsErrorKind::UnsupportedCapability, error.error().kind());

    let object_key = Path::parse_with_semantics(
        "/provider-literal",
        PathSemantics::ObjectKey,
    )
    .expect("object key should parse");
    let error = ready(file_system.stat(&object_key))
        .expect_err("foreign path semantics must be rejected");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(FsOperation::Stat, error.operation());
    assert_eq!(Some(&object_key), error.path());
    assert_eq!(Some("async-recording"), error.provider());
}

/// Covers successful stream fallback completion after all provider boundaries.
#[test]
fn test_async_facade_stream_fallback_returns_completed_outcome() {
    let (file_system, _) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let mut operation = file_system
        .begin_copy(path("/source"), path("/target"), CopyOptions::default())
        .expect("copy preflight should succeed");
    let outcome = ready(operation.execute())
        .expect("stream fallback should return its completed outcome");
    assert_eq!(1, outcome.stats().files);
}

/// Covers facade preflight error exits that must remain free of provider I/O.
#[test]
fn test_async_facade_preflight_rejects_invalid_paths_options_and_capabilities()
{
    let source = path("/source");
    let target = path("/target");
    let relative = path("relative");
    let (file_system, _) =
        async_recording_file_system(AsyncRecordingConfig::default());

    ready(file_system.list(&source, ListOptions::default()))
        .expect("listing should dispatch with the advertised capability");
    assert!(
        ready(file_system.list(&relative, ListOptions::default())).is_err()
    );
    let (without_list, _) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::List),
        ..AsyncRecordingConfig::default()
    });
    assert!(ready(without_list.list(&source, ListOptions::default())).is_err());
    assert!(
        ready(file_system.open_reader(&relative, ReadOptions::default()))
            .is_err()
    );
    assert!(
        ready(
            file_system.open_reader(
                &source,
                ReadOptions::default().with_length(Some(1)),
            ),
        )
        .is_err()
    );
    let (read_limited, _) = async_recording_file_system(AsyncRecordingConfig {
        range_read: true,
        maximum_read_range_bytes: Some(1),
        ..AsyncRecordingConfig::default()
    });
    assert!(
        ready(
            read_limited.open_reader(
                &source,
                ReadOptions::default().with_length(Some(2)),
            ),
        )
        .is_err()
    );
    assert!(
        ready(file_system.open_writer(&relative, WriteOptions::default()))
            .is_err()
    );
    assert!(
        ready(
            file_system.open_writer(
                &target,
                WriteOptions::default()
                    .with_atomicity(AtomicityRequirement::Required),
            ),
        )
        .is_err()
    );
    assert!(
        ready(
            file_system
                .create_directory(&relative, CreateDirectoryOptions::default()),
        )
        .is_err()
    );

    assert!(
        file_system
            .begin_copy(
                relative.clone(),
                target.clone(),
                CopyOptions::default()
            )
            .is_err()
    );
    assert!(
        file_system
            .begin_copy(
                source.clone(),
                relative.clone(),
                CopyOptions::default()
            )
            .is_err()
    );
    assert!(
        file_system
            .begin_copy(
                source.clone(),
                target.clone(),
                CopyOptions::default()
                    .with_durability(DurabilityRequirement::Required),
            )
            .is_err()
    );
    let (without_copy, _) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::Copy),
        omit_read_and_write: true,
        ..AsyncRecordingConfig::default()
    });
    let mut operation = without_copy
        .begin_copy(source.clone(), target.clone(), CopyOptions::default())
        .expect("copy preflight should allow fallback selection");
    let failure = ready(operation.execute()).expect_err("fallback should require read and write");
    assert_eq!(FsErrorKind::UnsupportedCapability, failure.error().kind());
    assert_eq!(CopyFailureState::Unchanged, failure.state());

    assert!(
        ready(file_system.rename(&relative, &target, RenameOptions::default()))
            .is_err()
    );
    assert!(
        ready(file_system.rename(&source, &relative, RenameOptions::default()))
            .is_err()
    );
    assert!(
        ready(file_system.rename(
            &source,
            &target,
            RenameOptions::default()
                .with_atomicity(AtomicityRequirement::Required),
        ),)
        .is_err()
    );

    assert!(
        ready(file_system.delete_file(&relative, DeleteOptions::default()))
            .is_err()
    );
    assert!(
        ready(file_system.delete_file(
            &target,
            DeleteOptions::default().with_recursive(true),
        ),)
        .is_err()
    );
    let (without_delete, _) =
        async_recording_file_system(AsyncRecordingConfig {
            omitted_capability: Some(FileSystemCapability::Delete),
            ..AsyncRecordingConfig::default()
        });
    assert!(
        ready(without_delete.delete_file(&target, DeleteOptions::default()),)
            .is_err()
    );

    let (file_system, _) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let mut temporary = ready(
        file_system.create_temp_file(qubit_fs::TempFileOptions::default()),
    )
    .expect("temporary file should be created");
    assert!(
        ready(temporary.persist(&relative, PersistOptions::default())).is_err()
    );
}

/// Covers a completed native-copy outcome that satisfies the requested policy.
#[test]
fn test_async_facade_returns_valid_completed_copy_outcome() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        completed_copy: Some(AchievedAtomicity::Atomic),
        ..AsyncRecordingConfig::default()
    });
    let mut operation = file_system
        .begin_copy(path("/source"), path("/target"), CopyOptions::default())
        .expect("copy preflight should succeed");
    let outcome = ready(operation.execute())
        .expect("valid completed copy should succeed");
    assert_eq!(AchievedAtomicity::Atomic, outcome.atomicity());
}
