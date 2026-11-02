// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression coverage for copy route preflight and fallback dispatch.

#[cfg(feature = "async")]
use std::future::Future;
use std::io::Result as IoResult;
#[cfg(feature = "async")]
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
#[cfg(feature = "async")]
use std::task::Context;
#[cfg(feature = "async")]
use std::task::Poll;

#[cfg(feature = "async")]
use qubit_fs::AsyncFileSystem;
use qubit_fs::FileSystem;
use qubit_fs::FsResult;
use qubit_fs::Path;
#[cfg(feature = "async")]
use qubit_fs::copy::AsyncCopyOperationState;
use qubit_fs::copy::CopyConflictPolicy;
use qubit_fs::copy::CopyFailureState;
use qubit_fs::copy::CopyMethod;
use qubit_fs::copy::CopyOptions;
use qubit_fs::copy::CopyOutcome;
use qubit_fs::copy::CopyStats;
use qubit_fs::copy::FallbackRejection;
use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::AchievedAtomicity;
use qubit_fs::metadata::AtomicityRequirement;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::PublicationMethod;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::metadata::WriteOutcome;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
#[cfg(feature = "async")]
use qubit_fs::spi::AsyncFileSystemSpi;
#[cfg(feature = "async")]
use qubit_fs::spi::AsyncFileWriteSession;
use qubit_fs::spi::CopyAttempt;
use qubit_fs::spi::CopyDeclineReason;
use qubit_fs::spi::CopyRequest;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::FileWriterSpi;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenWriterRequest;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenedAsyncReader;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenedAsyncWriter;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::OpenedWriter;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::SpiCopyFailure;
#[cfg(feature = "async")]
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_fs::write::WriteAbortOutcome;
use qubit_fs::write::WriteDisposition;
use qubit_fs::write::WriteOptions;
#[cfg(feature = "async")]
use qubit_io::AsyncInput;
#[cfg(feature = "async")]
use qubit_io::AsyncOutput;
use qubit_io::Output;

struct RouteProbe {
    calls: Arc<Mutex<Vec<&'static str>>>,
    include_stat: bool,
    include_try_copy: bool,
    include_read: bool,
    include_write: bool,
    native_reply: NativeReply,
}

#[derive(Clone, Copy)]
enum NativeReply {
    Declined,
    Completed,
    Failed,
}

impl RouteProbe {
    fn properties(&self) -> ProviderProperties {
        let mut operations = ProviderOperations::new()
            .with(ProviderOperation::OpenReader)
            .with(ProviderOperation::OpenWriter);
        if self.include_stat {
            operations = operations.with(ProviderOperation::Stat);
        }
        if self.include_try_copy {
            operations = operations.with(ProviderOperation::TryCopy);
        }
        let mut capabilities = FileSystemCapabilities::new();
        if self.include_read {
            capabilities = capabilities.with_guaranteed(FileSystemCapability::Read);
        }
        if self.include_write {
            capabilities = capabilities.with_guaranteed(FileSystemCapability::Write);
        }
        if self.include_try_copy {
            capabilities = capabilities.with_guaranteed(FileSystemCapability::Copy);
        }
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("copy-route-test").expect("test id"),
                "copy-route-test",
                PathSemantics::Hierarchical,
            ),
            operations,
            capabilities,
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("test provider properties")
    }
}

impl FileSystemSpi for RouteProbe {
    fn properties(&self) -> ProviderProperties {
        self.properties()
    }

    fn stat(&self, _: StatRequest<'_>) -> FsResult<StatResponse> {
        self.calls.lock().expect("calls lock").push("stat");
        Err(qubit_fs::FsError::new(
            FsErrorKind::UnsupportedOperation,
            qubit_fs::error::FsOperation::Stat,
            "unexpected stat dispatch",
        ))
    }

    fn try_copy(&self, request: CopyRequest<'_>) -> Result<CopyAttempt, SpiCopyFailure> {
        self.calls.lock().expect("calls lock").push("try_copy");
        match self.native_reply {
            NativeReply::Declined => Ok(CopyAttempt::Declined(CopyDeclineReason::NotApplicable)),
            NativeReply::Completed => Ok(CopyAttempt::Completed(CopyOutcome::new(
                CopyStats {
                    files: 1,
                    ..CopyStats::default()
                },
                CopyMethod::Native,
                AchievedAtomicity::Atomic,
            ))),
            NativeReply::Failed => Err(SpiCopyFailure::new(
                qubit_fs::FsError::new(
                    FsErrorKind::Io,
                    qubit_fs::error::FsOperation::Copy,
                    "native source marker",
                )
                .with_path(request.source().clone()),
                CopyFailureState::Indeterminate,
                CopyStats::default(),
            )),
        }
    }
}

fn path(value: &str) -> Path {
    Path::parse(value).expect("valid test path")
}

#[test]
fn test_sync_copy_rejects_missing_stat_without_provider_io() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let filesystem = FileSystem::from_spi(RouteProbe {
        calls: Arc::clone(&calls),
        include_stat: false,
        include_try_copy: false,
        include_read: true,
        include_write: true,
        native_reply: NativeReply::Declined,
    })
    .expect("valid facade");
    let source = path("/source");
    let target = path("/target");

    let assessment = filesystem
        .assess_copy(&source, &target, &CopyOptions::file())
        .expect_err("missing stat prevents stream-only copy");
    assert_eq!(FsErrorKind::RequirementNotMet, assessment.kind());

    let failure = filesystem
        .copy(&source, &target, CopyOptions::file())
        .expect_err("execution must reject before provider I/O");
    assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
    assert_eq!(CopyFailureState::Unchanged, failure.state());
    assert_eq!(0, failure.partial_stats().files);
    assert_eq!(0, failure.partial_stats().bytes);
    assert_eq!(0, failure.partial_stats().failed);
    assert!(failure.recovery().is_none());
    assert_eq!(Some(&source), failure.error().path());
    assert_eq!(Some(&target), failure.error().target());
    assert_eq!(Some("copy-route-test"), failure.error().provider());
    assert!(calls.lock().expect("calls lock").is_empty());
}

#[test]
fn test_sync_decline_does_not_dispatch_missing_stat_fallback_primitive() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let filesystem = FileSystem::from_spi(RouteProbe {
        calls: Arc::clone(&calls),
        include_stat: false,
        include_try_copy: true,
        include_read: true,
        include_write: true,
        native_reply: NativeReply::Declined,
    })
    .expect("valid facade");
    let source = path("/source");
    let target = path("/target");
    let assessment = filesystem
        .assess_copy(&source, &target, &CopyOptions::file())
        .expect("native route remains available");
    assert_eq!(Some(FallbackRejection::MissingStat), assessment.fallback_rejection());
    let failure = filesystem
        .copy(&source, &target, CopyOptions::file())
        .expect_err("declined native route cannot stream without stat");
    assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
    assert_eq!(vec!["try_copy"], *calls.lock().expect("calls lock"));
}

#[test]
fn test_sync_decline_does_not_dispatch_missing_read_or_write_fallback_primitives() {
    for (include_read, include_write, expected) in [
        (false, true, FallbackRejection::MissingRead),
        (true, false, FallbackRejection::MissingWrite),
    ] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let filesystem = FileSystem::from_spi(RouteProbe {
            calls: Arc::clone(&calls),
            include_stat: true,
            include_try_copy: true,
            include_read,
            include_write,
            native_reply: NativeReply::Declined,
        })
        .expect("valid facade");
        let source = path("/source");
        let target = path("/target");
        let assessment = filesystem
            .assess_copy(&source, &target, &CopyOptions::file())
            .expect("native route remains available");
        assert_eq!(Some(expected), assessment.fallback_rejection());
        let failure = filesystem
            .copy(&source, &target, CopyOptions::file())
            .expect_err("decline must not enter an incomplete fallback");
        assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
        assert_eq!(vec!["try_copy"], *calls.lock().expect("calls lock"));
    }
}

#[test]
fn test_sync_native_completed_does_not_require_stream_primitives() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let filesystem = FileSystem::from_spi(RouteProbe {
        calls: Arc::clone(&calls),
        include_stat: false,
        include_try_copy: true,
        include_read: false,
        include_write: false,
        native_reply: NativeReply::Completed,
    })
    .expect("valid native-only facade");
    let outcome = filesystem
        .copy(&path("/source"), &path("/target"), CopyOptions::file())
        .expect("native completion needs no stream primitives");
    assert_eq!(CopyMethod::Native, outcome.method());
    assert_eq!(1, outcome.stats().files);
    assert_eq!(vec!["try_copy"], *calls.lock().expect("calls lock"));
}

#[test]
fn test_sync_native_failure_preserves_source_and_does_not_fallback() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let filesystem = FileSystem::from_spi(RouteProbe {
        calls: Arc::clone(&calls),
        include_stat: false,
        include_try_copy: true,
        include_read: false,
        include_write: false,
        native_reply: NativeReply::Failed,
    })
    .expect("valid native-only facade");
    let source = path("/source");
    let failure = filesystem
        .copy(&source, &path("/target"), CopyOptions::file())
        .expect_err("native failure must remain terminal");
    assert_eq!(FsErrorKind::Io, failure.error().kind());
    assert_eq!(Some(&source), failure.error().path());
    assert_eq!(CopyFailureState::Indeterminate, failure.state());
    assert_eq!(vec!["try_copy"], *calls.lock().expect("calls lock"));
}

#[test]
fn test_sync_stat_runtime_error_keeps_its_category_and_source() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let filesystem = FileSystem::from_spi(RouteProbe {
        calls: Arc::clone(&calls),
        include_stat: true,
        include_try_copy: false,
        include_read: true,
        include_write: true,
        native_reply: NativeReply::Declined,
    })
    .expect("valid facade");
    let source = path("/source");
    let failure = filesystem
        .copy(&source, &path("/target"), CopyOptions::file())
        .expect_err("provider stat failure should be returned");
    assert_eq!(FsErrorKind::UnsupportedOperation, failure.error().kind());
    assert_eq!(Some(&source), failure.error().path());
    assert_eq!(vec!["stat"], *calls.lock().expect("calls lock"));
}

#[cfg(feature = "async")]
impl AsyncFileSystemSpi for RouteProbe {
    fn properties(&self) -> ProviderProperties {
        RouteProbe::properties(self)
    }

    fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        self.calls.lock().expect("calls lock").push("stat");
        Box::pin(async {
            Err(qubit_fs::FsError::new(
                FsErrorKind::UnsupportedOperation,
                qubit_fs::error::FsOperation::Stat,
                "unexpected stat dispatch",
            ))
        })
    }

    fn try_copy<'a>(&'a self, request: CopyRequest<'a>) -> SpiFuture<'a, Result<CopyAttempt, SpiCopyFailure>> {
        self.calls.lock().expect("calls lock").push("try_copy");
        let reply = match self.native_reply {
            NativeReply::Declined => Ok(CopyAttempt::Declined(CopyDeclineReason::NotApplicable)),
            NativeReply::Completed => Ok(CopyAttempt::Completed(CopyOutcome::new(
                CopyStats {
                    files: 1,
                    ..CopyStats::default()
                },
                CopyMethod::Native,
                AchievedAtomicity::Atomic,
            ))),
            NativeReply::Failed => Err(SpiCopyFailure::new(
                qubit_fs::FsError::new(
                    FsErrorKind::Io,
                    qubit_fs::error::FsOperation::Copy,
                    "native source marker",
                )
                .with_path(request.source().clone()),
                CopyFailureState::Indeterminate,
                CopyStats::default(),
            )),
        };
        Box::pin(async move { reply })
    }
}

#[cfg(feature = "async")]
#[test]
fn test_async_copy_rejects_missing_stat_without_provider_io() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let filesystem = AsyncFileSystem::from_spi(RouteProbe {
        calls: Arc::clone(&calls),
        include_stat: false,
        include_try_copy: false,
        include_read: true,
        include_write: true,
        native_reply: NativeReply::Declined,
    })
    .expect("valid facade");
    let source = path("/source");
    let target = path("/target");
    let mut operation = filesystem
        .begin_copy(source.clone(), target.clone(), CopyOptions::file())
        .expect("copy operation construction should not require stat");
    assert_eq!(AsyncCopyOperationState::Ready, operation.state());
    assert!(calls.lock().expect("calls lock").is_empty());
    let waker = std::task::Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = Box::pin(operation.execute());
    let Poll::Ready(Err(failure)) = future.as_mut().poll(&mut context) else {
        panic!("immediate provider attempt should finish in one poll");
    };
    assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
    assert_eq!(CopyFailureState::Unchanged, failure.state());
    assert_eq!(0, failure.partial_stats().files);
    assert_eq!(0, failure.partial_stats().bytes);
    assert_eq!(0, failure.partial_stats().failed);
    drop(future);
    assert!(!operation.has_recovery());
    assert_eq!(
        AsyncCopyOperationState::Failed(CopyFailureState::Unchanged),
        operation.state()
    );
    assert!(calls.lock().expect("calls lock").is_empty());
}

#[cfg(feature = "async")]
#[test]
fn test_async_decline_does_not_dispatch_missing_stat_fallback_primitive() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let filesystem = AsyncFileSystem::from_spi(RouteProbe {
        calls: Arc::clone(&calls),
        include_stat: false,
        include_try_copy: true,
        include_read: true,
        include_write: true,
        native_reply: NativeReply::Declined,
    })
    .expect("valid facade");
    let source = path("/source");
    let target = path("/target");
    let assessment = filesystem
        .assess_copy(&source, &target, &CopyOptions::file())
        .expect("native route remains available");
    assert_eq!(Some(FallbackRejection::MissingStat), assessment.fallback_rejection());

    let mut operation = filesystem
        .begin_copy(source, target, CopyOptions::file())
        .expect("copy preflight should succeed");
    let waker = std::task::Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = Box::pin(operation.execute());
    let Poll::Ready(Err(failure)) = future.as_mut().poll(&mut context) else {
        panic!("immediate provider attempt should finish in one poll");
    };
    assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
    assert_eq!(CopyFailureState::Unchanged, failure.state());
    drop(future);
    assert_eq!(
        AsyncCopyOperationState::Failed(CopyFailureState::Unchanged),
        operation.state()
    );
    assert_eq!(vec!["try_copy"], *calls.lock().expect("calls lock"));
}

#[cfg(feature = "async")]
#[test]
fn test_async_decline_does_not_dispatch_missing_read_or_write_fallback_primitives() {
    for (include_read, include_write, expected) in [
        (false, true, FallbackRejection::MissingRead),
        (true, false, FallbackRejection::MissingWrite),
    ] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let filesystem = AsyncFileSystem::from_spi(RouteProbe {
            calls: Arc::clone(&calls),
            include_stat: true,
            include_try_copy: true,
            include_read,
            include_write,
            native_reply: NativeReply::Declined,
        })
        .expect("valid facade");
        let source = path("/source");
        let target = path("/target");
        let assessment = filesystem
            .assess_copy(&source, &target, &CopyOptions::file())
            .expect("native route remains available");
        assert_eq!(Some(expected), assessment.fallback_rejection());

        let mut operation = filesystem
            .begin_copy(source, target, CopyOptions::file())
            .expect("copy preflight should succeed");
        let waker = std::task::Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut future = Box::pin(operation.execute());
        let Poll::Ready(Err(failure)) = future.as_mut().poll(&mut context) else {
            panic!("immediate provider attempt should finish in one poll");
        };
        assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
        assert_eq!(CopyFailureState::Unchanged, failure.state());
        drop(future);
        assert_eq!(
            AsyncCopyOperationState::Failed(CopyFailureState::Unchanged),
            operation.state()
        );
        assert_eq!(vec!["try_copy"], *calls.lock().expect("calls lock"));
    }
}

const ATOMIC_PAYLOAD: &[u8] = b"atomic-report-0123456789";

struct AtomicRouteProbe {
    calls: Arc<Mutex<Vec<&'static str>>>,
    payload: Arc<Mutex<Vec<u8>>>,
    writer_options: Arc<Mutex<Vec<WriteOptions>>>,
    commit_atomicity: AchievedAtomicity,
    with_copy_atomic_capability: bool,
    with_writer_atomic_capability: bool,
}

type AtomicRouteHandles = (
    FileSystem,
    Arc<Mutex<Vec<&'static str>>>,
    Arc<Mutex<Vec<u8>>>,
    Arc<Mutex<Vec<WriteOptions>>>,
);

impl AtomicRouteProbe {
    fn properties(&self) -> ProviderProperties {
        let mut capabilities = FileSystemCapabilities::new()
            .with_guaranteed(FileSystemCapability::Read)
            .with_guaranteed(FileSystemCapability::Write)
            .with_guaranteed(FileSystemCapability::Copy);
        if self.with_copy_atomic_capability {
            capabilities = capabilities.with_guaranteed(FileSystemCapability::AtomicFileCopy);
        }
        if self.with_writer_atomic_capability {
            capabilities = capabilities.with_guaranteed(FileSystemCapability::AtomicReplace);
        }
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("atomic-copy-route-test").expect("test id"),
                "atomic-copy-route-test",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::OpenReader)
                .with(ProviderOperation::OpenWriter)
                .with(ProviderOperation::TryCopy),
            capabilities,
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("valid atomic-copy provider properties")
    }
}

impl FileSystemSpi for AtomicRouteProbe {
    fn properties(&self) -> ProviderProperties {
        AtomicRouteProbe::properties(self)
    }

    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        self.calls.lock().expect("calls lock").push("stat");
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(Some(ATOMIC_PAYLOAD.len() as u64)),
        ))
    }

    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        self.calls.lock().expect("calls lock").push("open_reader");
        let info = OpenedFileInfo::new(
            FileSystemId::new("atomic-copy-route-test").expect("test id"),
            request.path().clone(),
        )
        .with_metadata(FileMetadata::new(FileKind::File).with_len(Some(ATOMIC_PAYLOAD.len() as u64)));
        Ok(OpenedReader::new(
            info,
            Box::new(std::io::Cursor::new(ATOMIC_PAYLOAD.to_vec())),
        ))
    }

    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        self.calls.lock().expect("calls lock").push("open_writer");
        self.writer_options
            .lock()
            .expect("writer options lock")
            .push(request.options().options().clone());
        let info = OpenedFileInfo::new(
            FileSystemId::new("atomic-copy-route-test").expect("test id"),
            request.path().clone(),
        )
        .with_metadata(FileMetadata::new(FileKind::File));
        Ok(OpenedWriter::new(
            info,
            Box::new(AtomicRouteWriter {
                calls: Arc::clone(&self.calls),
                payload: Arc::clone(&self.payload),
                commit_atomicity: self.commit_atomicity,
            }),
        ))
    }

    fn try_copy(&self, _: CopyRequest<'_>) -> Result<CopyAttempt, SpiCopyFailure> {
        self.calls.lock().expect("calls lock").push("try_copy");
        Ok(CopyAttempt::Declined(CopyDeclineReason::NotApplicable))
    }
}

struct AtomicRouteWriter {
    calls: Arc<Mutex<Vec<&'static str>>>,
    payload: Arc<Mutex<Vec<u8>>>,
    commit_atomicity: AchievedAtomicity,
}

impl Output for AtomicRouteWriter {
    type Item = u8;

    unsafe fn write_unchecked(&mut self, buffer: &[u8], _: usize, count: usize) -> IoResult<usize> {
        self.calls.lock().expect("calls lock").push("write");
        self.payload
            .lock()
            .expect("payload lock")
            .extend_from_slice(&buffer[..count]);
        Ok(count)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.calls.lock().expect("calls lock").push("flush");
        Ok(())
    }
}

impl FileWriterSpi for AtomicRouteWriter {
    fn commit(&mut self) -> Result<WriteOutcome, qubit_fs::spi::SpiWriteFailure> {
        self.calls.lock().expect("calls lock").push("commit");
        Ok(WriteOutcome::new(self.commit_atomicity, PublicationMethod::StreamCopy))
    }

    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        self.calls.lock().expect("calls lock").push("abort");
        Ok(WriteAbortOutcome::NotPublished)
    }
}

fn atomic_route_filesystem(
    commit_atomicity: AchievedAtomicity,
    with_copy_atomic_capability: bool,
    with_writer_atomic_capability: bool,
) -> AtomicRouteHandles {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let payload = Arc::new(Mutex::new(Vec::new()));
    let writer_options = Arc::new(Mutex::new(Vec::new()));
    let filesystem = FileSystem::from_spi(AtomicRouteProbe {
        calls: Arc::clone(&calls),
        payload: Arc::clone(&payload),
        writer_options: Arc::clone(&writer_options),
        commit_atomicity,
        with_copy_atomic_capability,
        with_writer_atomic_capability,
    })
    .expect("valid atomic-copy facade");
    (filesystem, calls, payload, writer_options)
}

#[test]
fn test_sync_atomic_required_copy_can_use_stream_fallback() {
    let (filesystem, calls, payload, writer_options) = atomic_route_filesystem(AchievedAtomicity::Atomic, true, true);
    let options = CopyOptions::file().with_atomicity(AtomicityRequirement::Required);
    let assessment = filesystem
        .assess_copy(&path("/source"), &path("/target"), &options)
        .expect("atomic file copy should pass preflight");
    assert_eq!(
        qubit_fs::copy::CopyExecutionRoute::ProviderThenStream,
        assessment.route()
    );
    assert_eq!(None, assessment.fallback_rejection());
    let outcome = filesystem
        .copy(&path("/source"), &path("/target"), options)
        .expect("atomic streamed fallback should succeed");
    assert_eq!(CopyMethod::Streamed, outcome.method());
    assert_eq!(AchievedAtomicity::Atomic, outcome.atomicity());
    assert!(outcome.used_fallback());
    assert_eq!(1, outcome.stats().files);
    assert_eq!(ATOMIC_PAYLOAD.len() as u64, outcome.stats().bytes);
    assert_eq!(ATOMIC_PAYLOAD, payload.lock().expect("payload lock").as_slice());
    let writer_options = writer_options.lock().expect("writer options lock");
    assert_eq!(1, writer_options.len());
    assert_eq!(WriteDisposition::CreateNew, writer_options[0].disposition());
    assert_eq!(AtomicityRequirement::Required, writer_options[0].atomicity());
    assert_eq!(
        1,
        calls
            .lock()
            .expect("calls lock")
            .iter()
            .filter(|call| **call == "try_copy")
            .count()
    );
    assert_eq!(
        1,
        calls
            .lock()
            .expect("calls lock")
            .iter()
            .filter(|call| **call == "commit")
            .count()
    );
}

#[test]
fn test_sync_atomic_required_fallback_rejects_non_atomic_commit() {
    let (filesystem, calls, payload, _) = atomic_route_filesystem(AchievedAtomicity::NonAtomic, true, true);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::file().with_atomicity(AtomicityRequirement::Required),
        )
        .expect_err("non-atomic commit must violate Required atomicity");
    assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(ATOMIC_PAYLOAD.len() as u64, failure.partial_stats().bytes);
    assert!(failure.recovery().is_some());
    assert_eq!(ATOMIC_PAYLOAD, payload.lock().expect("payload lock").as_slice());
    assert_eq!(
        1,
        calls
            .lock()
            .expect("calls lock")
            .iter()
            .filter(|call| **call == "commit")
            .count()
    );
}

#[test]
fn test_sync_atomic_skip_has_no_stream_fallback() {
    let (filesystem, calls, _, _) = atomic_route_filesystem(AchievedAtomicity::Atomic, true, true);
    let options = CopyOptions::file()
        .with_conflict(CopyConflictPolicy::Skip)
        .with_atomicity(AtomicityRequirement::Required);
    let assessment = filesystem
        .assess_copy(&path("/source"), &path("/target"), &options)
        .expect("native route can be assessed");
    assert_eq!(qubit_fs::copy::CopyExecutionRoute::ProviderOnly, assessment.route());
    assert_eq!(Some(FallbackRejection::AtomicSkip), assessment.fallback_rejection());
    let failure = filesystem
        .copy(&path("/source"), &path("/target"), options)
        .expect_err("declined AtomicSkip has no fallback");
    assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
    assert_eq!(vec!["try_copy"], *calls.lock().expect("calls lock"));
}

#[test]
fn test_sync_required_atomic_copy_needs_copy_and_writer_capabilities() {
    let (filesystem, calls, _, _) = atomic_route_filesystem(AchievedAtomicity::Atomic, false, true);
    let error = filesystem
        .assess_copy(
            &path("/source"),
            &path("/target"),
            &CopyOptions::file().with_atomicity(AtomicityRequirement::Required),
        )
        .expect_err("AtomicReplace alone cannot satisfy atomic copy preflight");
    assert_eq!(FsErrorKind::RequirementNotMet, error.kind());
    assert!(calls.lock().expect("calls lock").is_empty());

    let (filesystem, calls, _, _) = atomic_route_filesystem(AchievedAtomicity::Atomic, true, false);
    let failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::file().with_atomicity(AtomicityRequirement::Required),
        )
        .expect_err("stream writer must satisfy AtomicReplace");
    assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
    assert!(!calls.lock().expect("calls lock").contains(&"open_writer"));
}

#[cfg(feature = "async")]
struct AsyncAtomicRouteProbe {
    calls: Arc<Mutex<Vec<&'static str>>>,
    payload: Arc<Mutex<Vec<u8>>>,
    writer_options: Arc<Mutex<Vec<WriteOptions>>>,
    commit_atomicity: AchievedAtomicity,
    with_copy_atomic_capability: bool,
    with_writer_atomic_capability: bool,
}

#[cfg(feature = "async")]
type AsyncAtomicRouteHandles = (
    AsyncFileSystem,
    Arc<Mutex<Vec<&'static str>>>,
    Arc<Mutex<Vec<u8>>>,
    Arc<Mutex<Vec<WriteOptions>>>,
);

#[cfg(feature = "async")]
impl AsyncAtomicRouteProbe {
    fn properties(&self) -> ProviderProperties {
        let mut capabilities = FileSystemCapabilities::new()
            .with_guaranteed(FileSystemCapability::Read)
            .with_guaranteed(FileSystemCapability::Write)
            .with_guaranteed(FileSystemCapability::Copy);
        if self.with_copy_atomic_capability {
            capabilities = capabilities.with_guaranteed(FileSystemCapability::AtomicFileCopy);
        }
        if self.with_writer_atomic_capability {
            capabilities = capabilities.with_guaranteed(FileSystemCapability::AtomicReplace);
        }
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("async-atomic-copy-route-test").expect("test id"),
                "async-atomic-copy-route-test",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::OpenReader)
                .with(ProviderOperation::OpenWriter)
                .with(ProviderOperation::TryCopy),
            capabilities,
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("valid async atomic-copy provider properties")
    }
}

#[cfg(feature = "async")]
impl AsyncFileSystemSpi for AsyncAtomicRouteProbe {
    fn properties(&self) -> ProviderProperties {
        AsyncAtomicRouteProbe::properties(self)
    }

    fn stat<'a>(&'a self, request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        self.calls.lock().expect("calls lock").push("stat");
        let path = request.path().clone();
        Box::pin(async move {
            Ok(StatResponse::new(
                path,
                FileMetadata::new(FileKind::File).with_len(Some(ATOMIC_PAYLOAD.len() as u64)),
            ))
        })
    }

    fn open_reader<'a>(&'a self, request: OpenReaderRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
        self.calls.lock().expect("calls lock").push("open_reader");
        let info = OpenedFileInfo::new(
            FileSystemId::new("async-atomic-copy-route-test").expect("test id"),
            request.path().clone(),
        )
        .with_metadata(FileMetadata::new(FileKind::File).with_len(Some(ATOMIC_PAYLOAD.len() as u64)));
        Box::pin(async move {
            Ok(OpenedAsyncReader::new(
                info,
                Box::new(AsyncAtomicReader { position: 0 }),
            ))
        })
    }

    fn open_writer<'a>(&'a self, request: OpenWriterRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
        self.calls.lock().expect("calls lock").push("open_writer");
        self.writer_options
            .lock()
            .expect("writer options lock")
            .push(request.options().options().clone());
        let info = OpenedFileInfo::new(
            FileSystemId::new("async-atomic-copy-route-test").expect("test id"),
            request.path().clone(),
        )
        .with_metadata(FileMetadata::new(FileKind::File));
        let writer = AsyncAtomicRouteWriter {
            calls: Arc::clone(&self.calls),
            payload: Arc::clone(&self.payload),
            commit_atomicity: self.commit_atomicity,
        };
        Box::pin(async move { Ok(OpenedAsyncWriter::new(info, Box::new(writer))) })
    }

    fn try_copy<'a>(&'a self, _: CopyRequest<'a>) -> SpiFuture<'a, Result<CopyAttempt, SpiCopyFailure>> {
        self.calls.lock().expect("calls lock").push("try_copy");
        Box::pin(async { Ok(CopyAttempt::Declined(CopyDeclineReason::NotApplicable)) })
    }
}

#[cfg(feature = "async")]
struct AsyncAtomicReader {
    position: usize,
}

#[cfg(feature = "async")]
impl AsyncInput for AsyncAtomicReader {
    type Item = u8;

    unsafe fn poll_read_unchecked(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        output: &mut [u8],
        index: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        let this = self.get_mut();
        let remaining = &ATOMIC_PAYLOAD[this.position..];
        let amount = remaining.len().min(count);
        output[index..index + amount].copy_from_slice(&remaining[..amount]);
        this.position += amount;
        Poll::Ready(Ok(amount))
    }
}

#[cfg(feature = "async")]
struct AsyncAtomicRouteWriter {
    calls: Arc<Mutex<Vec<&'static str>>>,
    payload: Arc<Mutex<Vec<u8>>>,
    commit_atomicity: AchievedAtomicity,
}

#[cfg(feature = "async")]
impl AsyncOutput for AsyncAtomicRouteWriter {
    type Item = u8;

    unsafe fn poll_write_unchecked(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        input: &[u8],
        index: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        let this = self.get_mut();
        this.calls.lock().expect("calls lock").push("write");
        this.payload
            .lock()
            .expect("payload lock")
            .extend_from_slice(&input[index..index + count]);
        Poll::Ready(Ok(count))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<IoResult<()>> {
        self.get_mut().calls.lock().expect("calls lock").push("flush");
        Poll::Ready(Ok(()))
    }
}

#[cfg(feature = "async")]
impl AsyncFileWriteSession for AsyncAtomicRouteWriter {
    fn commit_async<'a>(
        self: Pin<&'a mut Self>,
    ) -> qubit_fs::spi::SpiFuture<'a, Result<WriteOutcome, qubit_fs::write::WriteFailure>> {
        let this = self.get_mut();
        this.calls.lock().expect("calls lock").push("commit");
        let atomicity = this.commit_atomicity;
        Box::pin(async move { Ok(WriteOutcome::new(atomicity, PublicationMethod::StreamCopy)) })
    }

    fn abort_async<'a>(self: Pin<&'a mut Self>) -> qubit_fs::spi::SpiFuture<'a, FsResult<WriteAbortOutcome>> {
        self.get_mut().calls.lock().expect("calls lock").push("abort");
        Box::pin(async { Ok(WriteAbortOutcome::NotPublished) })
    }
}

#[cfg(feature = "async")]
fn async_atomic_route_filesystem(
    commit_atomicity: AchievedAtomicity,
    with_copy_atomic_capability: bool,
    with_writer_atomic_capability: bool,
) -> AsyncAtomicRouteHandles {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let payload = Arc::new(Mutex::new(Vec::new()));
    let writer_options = Arc::new(Mutex::new(Vec::new()));
    let filesystem = AsyncFileSystem::from_spi(AsyncAtomicRouteProbe {
        calls: Arc::clone(&calls),
        payload: Arc::clone(&payload),
        writer_options: Arc::clone(&writer_options),
        commit_atomicity,
        with_copy_atomic_capability,
        with_writer_atomic_capability,
    })
    .expect("valid async atomic-copy facade");
    (filesystem, calls, payload, writer_options)
}

#[cfg(feature = "async")]
fn ready<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let waker = std::task::Waker::noop();
    let mut context = Context::from_waker(waker);
    for _ in 0..10_000 {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
    panic!("ready test future did not complete");
}

#[cfg(feature = "async")]
#[test]
fn test_async_atomic_required_copy_can_use_stream_fallback() {
    let (filesystem, calls, payload, writer_options) =
        async_atomic_route_filesystem(AchievedAtomicity::Atomic, true, true);
    let options = CopyOptions::file().with_atomicity(AtomicityRequirement::Required);
    let assessment = filesystem
        .assess_copy(&path("/source"), &path("/target"), &options)
        .expect("atomic file copy should pass preflight");
    assert_eq!(
        qubit_fs::copy::CopyExecutionRoute::ProviderThenStream,
        assessment.route()
    );
    assert_eq!(None, assessment.fallback_rejection());
    let mut operation = filesystem
        .begin_copy(path("/source"), path("/target"), options)
        .expect("copy operation should be constructed");
    let outcome = ready(operation.execute()).expect("atomic streamed fallback should succeed");
    assert_eq!(CopyMethod::Streamed, outcome.method());
    assert_eq!(AchievedAtomicity::Atomic, outcome.atomicity());
    assert!(outcome.used_fallback());
    assert_eq!(1, outcome.stats().files);
    assert_eq!(ATOMIC_PAYLOAD.len() as u64, outcome.stats().bytes);
    assert_eq!(ATOMIC_PAYLOAD, payload.lock().expect("payload lock").as_slice());
    let writer_options = writer_options.lock().expect("writer options lock");
    assert_eq!(1, writer_options.len());
    assert_eq!(WriteDisposition::CreateNew, writer_options[0].disposition());
    assert_eq!(AtomicityRequirement::Required, writer_options[0].atomicity());
    assert_eq!(
        1,
        calls
            .lock()
            .expect("calls lock")
            .iter()
            .filter(|call| **call == "try_copy")
            .count()
    );
    assert_eq!(
        1,
        calls
            .lock()
            .expect("calls lock")
            .iter()
            .filter(|call| **call == "commit")
            .count()
    );
}

#[cfg(feature = "async")]
#[test]
fn test_async_atomic_required_fallback_rejects_non_atomic_commit() {
    let (filesystem, calls, payload, _) = async_atomic_route_filesystem(AchievedAtomicity::NonAtomic, true, true);
    let mut operation = filesystem
        .begin_copy(
            path("/source"),
            path("/target"),
            CopyOptions::file().with_atomicity(AtomicityRequirement::Required),
        )
        .expect("copy operation should be constructed");
    let failure = ready(operation.execute()).expect_err("non-atomic commit must violate Required atomicity");
    assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(ATOMIC_PAYLOAD.len() as u64, failure.partial_stats().bytes);
    assert!(operation.has_recovery());
    assert_eq!(ATOMIC_PAYLOAD, payload.lock().expect("payload lock").as_slice());
    assert_eq!(
        1,
        calls
            .lock()
            .expect("calls lock")
            .iter()
            .filter(|call| **call == "commit")
            .count()
    );
}

#[cfg(feature = "async")]
#[test]
fn test_async_atomic_skip_has_no_stream_fallback() {
    let (filesystem, calls, _, _) = async_atomic_route_filesystem(AchievedAtomicity::Atomic, true, true);
    let options = CopyOptions::file()
        .with_conflict(CopyConflictPolicy::Skip)
        .with_atomicity(AtomicityRequirement::Required);
    let assessment = filesystem
        .assess_copy(&path("/source"), &path("/target"), &options)
        .expect("native route can be assessed");
    assert_eq!(qubit_fs::copy::CopyExecutionRoute::ProviderOnly, assessment.route());
    assert_eq!(Some(FallbackRejection::AtomicSkip), assessment.fallback_rejection());
    let mut operation = filesystem
        .begin_copy(path("/source"), path("/target"), options)
        .expect("copy operation should be constructed");
    let failure = ready(operation.execute()).expect_err("declined AtomicSkip has no fallback");
    assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
    assert_eq!(vec!["try_copy"], *calls.lock().expect("calls lock"));
}

#[cfg(feature = "async")]
#[test]
fn test_async_required_atomic_copy_needs_copy_and_writer_capabilities() {
    let (filesystem, calls, _, _) = async_atomic_route_filesystem(AchievedAtomicity::Atomic, false, true);
    let error = filesystem
        .assess_copy(
            &path("/source"),
            &path("/target"),
            &CopyOptions::file().with_atomicity(AtomicityRequirement::Required),
        )
        .expect_err("AtomicReplace alone cannot satisfy atomic copy preflight");
    assert_eq!(FsErrorKind::RequirementNotMet, error.kind());
    assert!(calls.lock().expect("calls lock").is_empty());

    let (filesystem, calls, _, _) = async_atomic_route_filesystem(AchievedAtomicity::Atomic, true, false);
    let mut operation = filesystem
        .begin_copy(
            path("/source"),
            path("/target"),
            CopyOptions::file().with_atomicity(AtomicityRequirement::Required),
        )
        .expect("copy operation construction is provider I/O free");
    let failure = ready(operation.execute()).expect_err("stream writer must satisfy AtomicReplace");
    assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
    assert!(!calls.lock().expect("calls lock").contains(&"open_writer"));
}
