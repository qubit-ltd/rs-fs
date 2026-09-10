// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Invalid opened identities retain isolated cleanup ownership.

use std::error::Error as _;
use std::ptr::eq;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use qubit_fs::FileSystem;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::error::OpenFailure;
use qubit_fs::error::OpenFailureStage;
use qubit_fs::error::RecoveryCleanupState;
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
use qubit_fs::spi::CreateTempDirectoryRequest;
use qubit_fs::spi::CreateTempFileRequest;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::FileWriterSpi;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::OpenedTempDirectory;
use qubit_fs::spi::OpenedTempFile;
use qubit_fs::spi::OpenedWriter;
use qubit_fs::spi::PersistRequest;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::SpiPersistFailure;
use qubit_fs::spi::SpiWriteFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_fs::spi::TempResourceSpi;
use qubit_fs::temp::PersistFailureState;
use qubit_fs::temp::PersistOutcome;
use qubit_fs::temp::TempOptions;
use qubit_fs::write::WriteAbortOutcome;
use qubit_fs::write::WriteOptions;
use qubit_io::Output;

#[derive(Default)]
struct Calls {
    opens: AtomicUsize,
    cleanups: AtomicUsize,
    drops: AtomicUsize,
    fail: AtomicBool,
    pending: AtomicBool,
}
struct Provider(Arc<Calls>);
struct Session(Arc<Calls>);

/// Supplies deliberately untrusted diagnostics from the rejected provider.
fn failure() -> FsError {
    FsError::with_source(
        FsErrorKind::Io,
        FsOperation::Other,
        "injected failure",
        std::io::Error::other("native cleanup failure"),
    )
    .with_provider("untrusted-provider")
    .with_path(Path::parse("/untrusted").expect("path"))
    .with_target(Path::parse("/untrusted-target").expect("path"))
}
/// Returns the stable configured provider identity and primitive set.
fn properties() -> ProviderProperties {
    ProviderProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("correct").expect("id"),
            "test",
            PathSemantics::Hierarchical,
        ),
        ProviderOperations::new()
            .with(ProviderOperation::Stat)
            .with(ProviderOperation::OpenWriter)
            .with(ProviderOperation::CreateTempFile)
            .with(ProviderOperation::CreateTempDirectory),
        FileSystemCapabilities::new()
            .with_guaranteed(FileSystemCapability::Write)
            .with_guaranteed(FileSystemCapability::TempFile)
            .with_guaranteed(FileSystemCapability::TempDirectory),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("properties")
}
/// The provider incorrectly attaches another filesystem's identity.
fn wrong_info(path: &Path, kind: FileKind) -> OpenedFileInfo {
    OpenedFileInfo::new(FileSystemId::new("untrusted-identity").expect("id"), path.clone())
        .with_metadata(FileMetadata::new(kind))
}
impl Session {
    /// Explicit cleanup records attempts and allows a controlled failure.
    fn cleanup_owned(&mut self) -> FsResult<()> {
        self.0.cleanups.fetch_add(1, Ordering::SeqCst);
        if self.0.fail.load(Ordering::SeqCst) {
            Err(failure())
        } else {
            Ok(())
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.0.drops.fetch_add(1, Ordering::SeqCst);
    }
}
impl Output for Session {
    type Item = u8;
    unsafe fn write_unchecked(&mut self, _: &[u8], _: usize, _: usize) -> std::io::Result<usize> {
        panic!("isolated session must not write")
    }
    fn flush(&mut self) -> std::io::Result<()> {
        panic!("isolated session must not flush")
    }
}
impl FileWriterSpi for Session {
    fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure> {
        panic!("isolated session must not commit")
    }
    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        self.cleanup_owned().map(|()| WriteAbortOutcome::NotPublished)
    }
}
impl TempResourceSpi for Session {
    fn cleanup(&mut self) -> FsResult<()> {
        self.cleanup_owned()
    }
    fn persist(&mut self, _: PersistRequest<'_>) -> Result<PersistOutcome, SpiPersistFailure> {
        Err(SpiPersistFailure::new(failure(), PersistFailureState::NotPublished))
    }
    fn keep(&mut self) -> Result<PersistOutcome, SpiPersistFailure> {
        panic!("isolated session must not publish")
    }
}
impl FileSystemSpi for Provider {
    fn properties(&self) -> ProviderProperties {
        properties()
    }
    fn stat(&self, _: StatRequest<'_>) -> FsResult<StatResponse> {
        Err(failure())
    }
    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        self.0.opens.fetch_add(1, Ordering::SeqCst);
        Ok(OpenedWriter::new(
            wrong_info(request.path(), FileKind::File),
            Box::new(Session(Arc::clone(&self.0))),
        ))
    }
    fn create_temp_file(&self, _: CreateTempFileRequest) -> FsResult<OpenedTempFile> {
        Ok(OpenedTempFile::new(
            wrong_info(&Path::parse("/tmp/file").expect("path"), FileKind::File),
            Box::new(Session(Arc::clone(&self.0))),
        ))
    }
    fn create_temp_directory(&self, _: CreateTempDirectoryRequest) -> FsResult<OpenedTempDirectory> {
        Ok(OpenedTempDirectory::new(
            wrong_info(&Path::parse("/tmp/dir").expect("path"), FileKind::Directory),
            Box::new(Session(Arc::clone(&self.0))),
        ))
    }
}

/// The error owns the session, and cleanup errors do not destroy it.
#[test]
fn test_rejected_writer_can_retry_cleanup_without_publishing() {
    let calls = Arc::new(Calls::default());
    let fs = FileSystem::from_spi(Provider(Arc::clone(&calls))).expect("filesystem");
    let mut failed = fs
        .open_writer(&Path::parse("/target").expect("path"), WriteOptions::default())
        .expect_err("identity mismatch");
    assert_eq!(failed.stage(), OpenFailureStage::OutcomeValidation);
    assert_eq!(failed.error().kind(), FsErrorKind::ProviderContractViolation);
    assert!(!format!("{failed:?}").contains("untrusted-identity"));
    assert_eq!(calls.drops.load(Ordering::SeqCst), 0);
    assert_eq!(calls.cleanups.load(Ordering::SeqCst), 0);
    let recovery = failed.recovery_mut().expect("recovery");
    assert_eq!(recovery.cleanup_state(), RecoveryCleanupState::Pending);
    calls.fail.store(true, Ordering::SeqCst);
    let error = recovery.abort().expect_err("injected cleanup error");
    assert_cleanup_context(&error, Some("/target"));
    assert_eq!(recovery.cleanup_state(), RecoveryCleanupState::Indeterminate);
    calls.fail.store(false, Ordering::SeqCst);
    assert_eq!(recovery.abort().expect("cleanup"), WriteAbortOutcome::NotPublished);
    assert_eq!(recovery.cleanup_state(), RecoveryCleanupState::Completed);
    assert_eq!(
        recovery.abort().expect_err("repeated cleanup").kind(),
        FsErrorKind::InvalidState
    );
    assert_eq!(calls.cleanups.load(Ordering::SeqCst), 2);
    drop(failed);
    assert_eq!(calls.drops.load(Ordering::SeqCst), 1);
}

/// Temporary identity rejection never starts an implicit cleanup attempt.
#[test]
fn test_rejected_temp_resources_keep_explicit_cleanup_ownership() {
    for directory in [false, true] {
        let calls = Arc::new(Calls::default());
        let fs = FileSystem::from_spi(Provider(Arc::clone(&calls))).expect("filesystem");
        let mut failed = if directory {
            fs.create_temp_directory(TempOptions::default()).expect_err("identity")
        } else {
            fs.create_temp_file(TempOptions::default()).expect_err("identity")
        };
        assert_eq!(failed.stage(), OpenFailureStage::OutcomeValidation);
        assert_eq!(calls.cleanups.load(Ordering::SeqCst), 0);
        assert_temp_open_failure_reporting(&failed);
        let mut recovery = failed.take_recovery().expect("owned recovery");
        drop(failed);
        let diagnostic = format!("{recovery:?}");
        assert!(diagnostic.contains("Pending"));
        assert!(!diagnostic.contains("untrusted"));
        assert_eq!(calls.drops.load(Ordering::SeqCst), 0);
        calls.fail.store(true, Ordering::SeqCst);
        assert_cleanup_context(&recovery.cleanup().expect_err("cleanup error"), None);
        calls.fail.store(false, Ordering::SeqCst);
        recovery.cleanup().expect("cleanup");
        assert_eq!(recovery.cleanup_state(), RecoveryCleanupState::Completed);
        let diagnostic = format!("{recovery:?}");
        assert!(diagnostic.contains("Completed"));
        assert!(!diagnostic.contains("untrusted"));
        let repeated = recovery.cleanup().expect_err("completed recovery cannot clean again");
        assert_eq!(FsErrorKind::InvalidState, repeated.kind());
        assert_eq!(FsOperation::CleanupTemp, repeated.operation());
        assert_eq!(Some("test"), repeated.provider());
        assert_eq!(None, repeated.path());
        assert_eq!(None, repeated.target());
        drop(recovery);
        assert_eq!(calls.cleanups.load(Ordering::SeqCst), 2);
        assert_eq!(calls.drops.load(Ordering::SeqCst), 1);
    }
}

/// Abandonment is observable only as Drop, never as confirmed cleanup.
#[test]
fn test_rejected_drop_does_not_call_cleanup() {
    let calls = Arc::new(Calls::default());
    let fs = FileSystem::from_spi(Provider(Arc::clone(&calls))).expect("filesystem");
    drop(fs.create_temp_file(TempOptions::default()).expect_err("identity"));
    assert_eq!(calls.cleanups.load(Ordering::SeqCst), 0);
    assert_eq!(calls.drops.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "async")]
#[path = "common/poll_support.rs"]
mod poll_support;

#[cfg(feature = "async")]
mod asynchronous {
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use std::task::Context;
    use std::task::Poll;

    use qubit_fs::AsyncFileSystem;
    use qubit_fs::FsResult;
    use qubit_fs::Path;
    use qubit_fs::error::FsErrorKind;
    use qubit_fs::error::FsOperation;
    use qubit_fs::error::OpenFailureStage;
    use qubit_fs::error::RecoveryCleanupState;
    use qubit_fs::metadata::FileKind;
    use qubit_fs::metadata::WriteOutcome;
    use qubit_fs::spi::AsyncFileSystemSpi;
    use qubit_fs::spi::AsyncFileWriteSession;
    use qubit_fs::spi::AsyncTempResourceSpi;
    use qubit_fs::spi::CreateTempDirectoryRequest;
    use qubit_fs::spi::CreateTempFileRequest;
    use qubit_fs::spi::OpenWriterRequest;
    use qubit_fs::spi::OpenedAsyncTempDirectory;
    use qubit_fs::spi::OpenedAsyncTempFile;
    use qubit_fs::spi::OpenedAsyncWriter;
    use qubit_fs::spi::PersistRequest;
    use qubit_fs::spi::ProviderProperties;
    use qubit_fs::spi::SpiFuture;
    use qubit_fs::spi::SpiPersistFailure;
    use qubit_fs::spi::StatRequest;
    use qubit_fs::spi::StatResponse;
    use qubit_fs::temp::PersistOutcome;
    use qubit_fs::temp::TempOptions;
    use qubit_fs::write::AsyncWriterRecovery;
    use qubit_fs::write::WriteAbortOutcome;
    use qubit_fs::write::WriteFailure;
    use qubit_fs::write::WriteFailureState;
    use qubit_fs::write::WriteOptions;
    use qubit_io::AsyncOutput;

    use super::Calls;
    use super::Provider;
    use super::Session;
    use super::assert_cleanup_context;
    use super::assert_temp_open_failure_reporting;
    use super::failure;
    use super::poll_support::assert_pending;
    use super::poll_support::ready;
    use super::properties;
    use super::wrong_info;

    impl AsyncOutput for Session {
        type Item = u8;
        unsafe fn poll_write_unchecked(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &[u8],
            _: usize,
            _: usize,
        ) -> Poll<std::io::Result<usize>> {
            panic!("rejected session cannot write")
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            panic!("rejected session cannot flush")
        }
    }
    impl Session {
        /// A deterministic cleanup gate; cancellation keeps ownership in
        /// Session.
        async fn cleanup_gated(&mut self) -> FsResult<()> {
            self.0.cleanups.fetch_add(1, Ordering::SeqCst);
            std::future::poll_fn(|_| {
                if self.0.pending.load(Ordering::SeqCst) {
                    Poll::Pending
                } else {
                    Poll::Ready(())
                }
            })
            .await;
            if self.0.fail.load(Ordering::SeqCst) {
                Err(failure())
            } else {
                Ok(())
            }
        }
    }
    impl AsyncFileWriteSession for Session {
        fn commit_async<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, Result<WriteOutcome, WriteFailure>> {
            panic!("rejected session cannot commit")
        }
        fn abort_async<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<WriteAbortOutcome>> {
            Box::pin(async move {
                self.get_mut()
                    .cleanup_gated()
                    .await
                    .map(|()| WriteAbortOutcome::NotPublished)
            })
        }
        fn cancel_on_drop(self: Pin<&mut Self>) {
            panic!("rejected session must not implicitly cancel")
        }
    }
    impl AsyncTempResourceSpi for Session {
        fn cleanup<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>> {
            Box::pin(async move { self.get_mut().cleanup_gated().await })
        }
        fn persist<'a>(
            self: Pin<&'a mut Self>,
            _: PersistRequest<'a>,
        ) -> SpiFuture<'a, Result<PersistOutcome, SpiPersistFailure>> {
            panic!("rejected session cannot persist")
        }
        fn keep<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, Result<PersistOutcome, SpiPersistFailure>> {
            panic!("rejected session cannot keep")
        }
        fn cancel_on_drop(self: Pin<&mut Self>) {
            panic!("rejected session must not implicitly cancel")
        }
    }
    impl AsyncFileSystemSpi for Provider {
        fn properties(&self) -> ProviderProperties {
            properties()
        }
        fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
            Box::pin(async { Err(failure()) })
        }
        fn open_writer<'a>(&'a self, request: OpenWriterRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
            Box::pin(async move {
                self.0.opens.fetch_add(1, Ordering::SeqCst);
                Ok(OpenedAsyncWriter::new(
                    wrong_info(request.path(), FileKind::File),
                    Box::new(Session(Arc::clone(&self.0))),
                ))
            })
        }
        fn create_temp_file<'a>(&'a self, _: CreateTempFileRequest) -> SpiFuture<'a, FsResult<OpenedAsyncTempFile>> {
            Box::pin(async move {
                Ok(OpenedAsyncTempFile::new(
                    wrong_info(&Path::parse("/tmp/file").expect("path"), FileKind::File),
                    Box::new(Session(Arc::clone(&self.0))),
                ))
            })
        }
        fn create_temp_directory<'a>(
            &'a self,
            _: CreateTempDirectoryRequest,
        ) -> SpiFuture<'a, FsResult<OpenedAsyncTempDirectory>> {
            Box::pin(async move {
                Ok(OpenedAsyncTempDirectory::new(
                    wrong_info(&Path::parse("/tmp/dir").expect("path"), FileKind::Directory),
                    Box::new(Session(Arc::clone(&self.0))),
                ))
            })
        }
    }

    /// Cancelling a polled cleanup future preserves the original session.
    #[test]
    fn test_async_rejected_writer_cleanup_cancellation_and_retry() {
        let calls = Arc::new(Calls::default());
        let fs = AsyncFileSystem::from_spi(Provider(Arc::clone(&calls))).expect("filesystem");
        let path = Path::parse("/target").expect("path");
        let mut failed = ready(fs.open_writer(&path, WriteOptions::default())).expect_err("identity");
        assert_eq!(failed.stage(), OpenFailureStage::OutcomeValidation);
        let recovery = failed.recovery_mut().expect("recovery");
        drop(recovery.abort_async());
        assert_eq!(recovery.cleanup_state(), RecoveryCleanupState::Pending);
        assert_eq!(calls.cleanups.load(Ordering::SeqCst), 0);
        calls.pending.store(true, Ordering::SeqCst);
        let mut future = recovery.abort_async();
        assert_pending(future.as_mut());
        drop(future);
        assert_eq!(recovery.cleanup_state(), RecoveryCleanupState::Indeterminate);
        assert_eq!(calls.drops.load(Ordering::SeqCst), 0);
        calls.pending.store(false, Ordering::SeqCst);
        calls.fail.store(true, Ordering::SeqCst);
        let error = ready(recovery.abort_async()).expect_err("injected cleanup error");
        assert_cleanup_context(&error, Some("/target"));
        calls.fail.store(false, Ordering::SeqCst);
        assert_eq!(
            ready(recovery.abort_async()).expect("cleanup"),
            WriteAbortOutcome::NotPublished
        );
        assert_eq!(recovery.cleanup_state(), RecoveryCleanupState::Completed);
        assert!(ready(recovery.abort_async()).is_err());
        assert_eq!(calls.cleanups.load(Ordering::SeqCst), 3);
        drop(failed);
        assert_eq!(calls.drops.load(Ordering::SeqCst), 1);
    }

    /// Both temporary envelope types use the same cancellation-safe ownership.
    #[test]
    fn test_async_rejected_temp_cleanup_cancellation_and_retry() {
        for directory in [false, true] {
            let calls = Arc::new(Calls::default());
            let fs = AsyncFileSystem::from_spi(Provider(Arc::clone(&calls))).expect("filesystem");
            let mut failed = if directory {
                ready(fs.create_temp_directory(TempOptions::default()))
                    .err()
                    .expect("identity")
            } else {
                ready(fs.create_temp_file(TempOptions::default()))
                    .err()
                    .expect("identity")
            };
            assert_temp_open_failure_reporting(&failed);
            let mut recovery = failed.take_recovery().expect("recovery");
            drop(failed);
            let diagnostic = format!("{recovery:?}");
            assert!(diagnostic.contains("Pending"));
            assert!(!diagnostic.contains("untrusted"));
            drop(recovery.cleanup_async());
            assert_eq!(recovery.cleanup_state(), RecoveryCleanupState::Pending);
            assert_eq!(calls.cleanups.load(Ordering::SeqCst), 0);
            calls.pending.store(true, Ordering::SeqCst);
            let mut future = recovery.cleanup_async();
            assert_pending(future.as_mut());
            drop(future);
            assert_eq!(recovery.cleanup_state(), RecoveryCleanupState::Indeterminate);
            assert_eq!(calls.drops.load(Ordering::SeqCst), 0);
            calls.pending.store(false, Ordering::SeqCst);
            calls.fail.store(true, Ordering::SeqCst);
            assert_cleanup_context(&ready(recovery.cleanup_async()).expect_err("cleanup error"), None);
            calls.fail.store(false, Ordering::SeqCst);
            ready(recovery.cleanup_async()).expect("cleanup");
            assert_eq!(recovery.cleanup_state(), RecoveryCleanupState::Completed);
            let diagnostic = format!("{recovery:?}");
            assert!(diagnostic.contains("Completed"));
            assert!(!diagnostic.contains("untrusted"));
            let repeated = ready(recovery.cleanup_async()).expect_err("completed recovery cannot clean again");
            assert_eq!(FsErrorKind::InvalidState, repeated.kind());
            assert_eq!(FsOperation::CleanupTemp, repeated.operation());
            assert_eq!(Some("test"), repeated.provider());
            assert_eq!(None, repeated.path());
            assert_eq!(None, repeated.target());
            drop(recovery);
            assert_eq!(calls.cleanups.load(Ordering::SeqCst), 3);
            assert_eq!(calls.drops.load(Ordering::SeqCst), 1);
        }
    }

    /// Aggregate execution transfers rejected ownership into the outer
    /// operation.
    #[test]
    fn test_async_write_all_preserves_rejected_session() {
        let calls = Arc::new(Calls::default());
        let fs = AsyncFileSystem::from_spi(Provider(Arc::clone(&calls))).expect("filesystem");
        let mut operation = fs
            .begin_write_all(
                Path::parse("/target").expect("path"),
                b"data".to_vec(),
                WriteOptions::default(),
            )
            .expect("operation");
        let failed = ready(operation.execute()).expect_err("identity");
        assert_eq!(failed.state(), WriteFailureState::Indeterminate);
        assert_eq!(failed.written_bytes(), 0);
        assert!(operation.has_recovery());
        let Some(AsyncWriterRecovery::Rejected(mut recovery)) = operation.take_recovery() else {
            panic!("rejected ownership")
        };
        assert_eq!(
            ready(recovery.abort_async()).expect("cleanup"),
            WriteAbortOutcome::NotPublished
        );
        assert_eq!(failed.state(), WriteFailureState::Indeterminate);
        assert_eq!(calls.cleanups.load(Ordering::SeqCst), 1);
        drop(recovery);
        drop(operation);
        assert_eq!(calls.drops.load(Ordering::SeqCst), 1);
    }
}

/// Public cleanup diagnostics carry only the trusted request location.
fn assert_cleanup_context(error: &FsError, expected: Option<&str>) {
    assert_eq!(error.provider(), Some("test"));
    assert_eq!(error.path().map(Path::as_str), expected);
    assert!(error.target().is_none());
    assert!(std::error::Error::source(error).is_some());
}

/// Reporting a rejected temporary opening preserves the causal error and owned
/// recovery.
fn assert_temp_open_failure_reporting<R: 'static>(failure: &OpenFailure<R>) {
    assert_eq!(failure.error().to_string(), failure.to_string());
    let cause = failure
        .source()
        .expect("the original filesystem error remains in the chain");
    let original = cause
        .downcast_ref::<FsError>()
        .expect("filesystem cause type is preserved");
    assert!(eq(failure.error(), original));
    assert_eq!(FsErrorKind::ProviderContractViolation, original.kind());
    assert!(
        failure.recovery().is_some(),
        "formatting and error inspection must retain cleanup ownership"
    );
}
