// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Recoverable asynchronous temporary-resource creation contracts.

#![cfg(feature = "async")]

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

use qubit_fs::AsyncFileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
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
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::AsyncTempCreateFailure;
use qubit_fs::spi::AsyncTempResourceSpi;
use qubit_fs::spi::CreateTempDirectoryRequest;
use qubit_fs::spi::CreateTempFileRequest;
use qubit_fs::spi::OpenedAsyncTempDirectory;
use qubit_fs::spi::OpenedAsyncTempFile;
use qubit_fs::spi::PersistRequest;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::SpiPersistFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_fs::temp::PersistOutcome;
use qubit_fs::temp::TempOptions;

#[derive(Clone, Copy)]
enum CreateResponse {
    RecoverableFailure,
    FailureWithoutSession,
    InvalidIdentity,
    Pending,
}

struct Provider {
    response: CreateResponse,
    cleanup_calls: Arc<AtomicUsize>,
    pending_drops: Arc<AtomicUsize>,
}

struct Session {
    cleanup_calls: Arc<AtomicUsize>,
}

impl AsyncTempResourceSpi for Session {
    fn cleanup<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>> {
        let calls = Arc::clone(&self.cleanup_calls);
        Box::pin(async move {
            if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(FsError::new(
                    FsErrorKind::Io,
                    FsOperation::CleanupTemp,
                    "injected cleanup failure",
                ))
            } else {
                Ok(())
            }
        })
    }

    fn keep<'a>(
        self: Pin<&'a mut Self>,
    ) -> SpiFuture<'a, Result<PersistOutcome, SpiPersistFailure>> {
        Box::pin(async { unreachable!("rejected session cannot be kept") })
    }

    fn persist<'a>(
        self: Pin<&'a mut Self>,
        _: PersistRequest<'a>,
    ) -> SpiFuture<'a, Result<PersistOutcome, SpiPersistFailure>> {
        Box::pin(async { unreachable!("rejected session cannot be persisted") })
    }
}

struct DropSignal(Arc<AtomicUsize>);

impl Future for DropSignal {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

impl Drop for DropSignal {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl Provider {
    fn properties() -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("async-create-recovery").expect("provider id"),
                "async-create-recovery",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::CreateTempFile)
                .with(ProviderOperation::CreateTempDirectory),
            FileSystemCapabilities::new()
                .with_guaranteed(FileSystemCapability::TempFile)
                .with_guaranteed(FileSystemCapability::TempDirectory),
            FileSystemLimits::unknown(),
            PathConstraints::either(),
            SymlinkPolicy::Reject,
        )
        .expect("provider properties")
    }

    fn session(&self) -> Box<dyn AsyncTempResourceSpi> {
        Box::new(Session {
            cleanup_calls: Arc::clone(&self.cleanup_calls),
        })
    }

    fn info(&self, kind: FileKind) -> OpenedFileInfo {
        let id = match self.response {
            CreateResponse::InvalidIdentity => FileSystemId::new("other-provider").expect("id"),
            _ => FileSystemId::new("async-create-recovery").expect("id"),
        };
        OpenedFileInfo::new(id, Path::parse("/tmp/resource").expect("path"))
            .with_metadata(FileMetadata::new(kind))
    }

    fn failure() -> FsError {
        FsError::new(
            FsErrorKind::Io,
            FsOperation::CreateTemp,
            "injected create failure",
        )
    }

    fn open_file<'a>(
        &'a self,
        _: CreateTempFileRequest,
    ) -> SpiFuture<'a, Result<OpenedAsyncTempFile, AsyncTempCreateFailure>> {
        match self.response {
            CreateResponse::RecoverableFailure => Box::pin(async move {
                Err(AsyncTempCreateFailure::new(
                    Self::failure(),
                    Some(self.session()),
                ))
            }),
            CreateResponse::FailureWithoutSession => {
                Box::pin(async { Err(AsyncTempCreateFailure::new(Self::failure(), None)) })
            }
            CreateResponse::InvalidIdentity => Box::pin(async move {
                Ok(OpenedAsyncTempFile::new(
                    self.info(FileKind::File),
                    self.session(),
                ))
            }),
            CreateResponse::Pending => {
                let drops = Arc::clone(&self.pending_drops);
                Box::pin(async move {
                    DropSignal(drops).await;
                    unreachable!("pending create must not produce an outcome")
                })
            }
        }
    }

    fn open_directory<'a>(
        &'a self,
        _: CreateTempDirectoryRequest,
    ) -> SpiFuture<'a, Result<OpenedAsyncTempDirectory, AsyncTempCreateFailure>> {
        match self.response {
            CreateResponse::RecoverableFailure => Box::pin(async move {
                Err(AsyncTempCreateFailure::new(
                    Self::failure(),
                    Some(self.session()),
                ))
            }),
            CreateResponse::FailureWithoutSession => {
                Box::pin(async { Err(AsyncTempCreateFailure::new(Self::failure(), None)) })
            }
            CreateResponse::InvalidIdentity => Box::pin(async move {
                Ok(OpenedAsyncTempDirectory::new(
                    self.info(FileKind::Directory),
                    self.session(),
                ))
            }),
            CreateResponse::Pending => {
                let drops = Arc::clone(&self.pending_drops);
                Box::pin(async move {
                    DropSignal(drops).await;
                    unreachable!("pending create must not produce an outcome")
                })
            }
        }
    }
}

impl AsyncFileSystemSpi for Provider {
    fn properties(&self) -> ProviderProperties {
        Self::properties()
    }

    fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async { unreachable!("stat is outside this contract") })
    }

    fn create_temp_file<'a>(
        &'a self,
        _request: CreateTempFileRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempFile>> {
        Box::pin(async move { Err(Self::failure()) })
    }

    fn create_temp_directory<'a>(
        &'a self,
        _request: CreateTempDirectoryRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempDirectory>> {
        Box::pin(async move { Err(Self::failure()) })
    }

    fn create_temp_file_recoverable<'a>(
        &'a self,
        request: CreateTempFileRequest,
    ) -> SpiFuture<'a, Result<OpenedAsyncTempFile, AsyncTempCreateFailure>> {
        self.open_file(request)
    }

    fn create_temp_directory_recoverable<'a>(
        &'a self,
        request: CreateTempDirectoryRequest,
    ) -> SpiFuture<'a, Result<OpenedAsyncTempDirectory, AsyncTempCreateFailure>> {
        self.open_directory(request)
    }
}

fn provider(response: CreateResponse) -> (Provider, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let cleanup_calls = Arc::new(AtomicUsize::new(0));
    let pending_drops = Arc::new(AtomicUsize::new(0));
    (
        Provider {
            response,
            cleanup_calls: Arc::clone(&cleanup_calls),
            pending_drops: Arc::clone(&pending_drops),
        },
        cleanup_calls,
        pending_drops,
    )
}

fn ready<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("future unexpectedly remained pending"),
    }
}

fn requested_options() -> TempOptions {
    TempOptions::default().with_parent(Some(Path::parse("/requested").expect("path")))
}

#[test]
fn test_async_provider_open_failure_retains_file_and_directory_recovery() {
    for directory in [false, true] {
        let (provider, cleanup_calls, _) = provider(CreateResponse::RecoverableFailure);
        let filesystem = AsyncFileSystem::from_spi(provider).expect("filesystem");
        let mut failure = if directory {
            ready(filesystem.create_temp_directory(requested_options()))
                .err()
                .expect("directory create fails")
        } else {
            ready(filesystem.create_temp_file(requested_options()))
                .err()
                .expect("file create fails")
        };

        assert_eq!(OpenFailureStage::ProviderOpen, failure.stage());
        assert_eq!(FsErrorKind::Io, failure.error().kind());
        assert_eq!(Some("async-create-recovery"), failure.error().provider());
        assert_eq!(
            Some(&Path::parse("/requested").expect("path")),
            failure.error().path()
        );
        let mut recovery = failure
            .take_recovery()
            .expect("provider returned cleanup session");
        assert_eq!(RecoveryCleanupState::Pending, recovery.cleanup_state());
        assert_eq!(
            FsErrorKind::Io,
            ready(recovery.cleanup_async())
                .expect_err("first cleanup fails")
                .kind()
        );
        assert_eq!(
            RecoveryCleanupState::Indeterminate,
            recovery.cleanup_state()
        );
        ready(recovery.cleanup_async()).expect("cleanup can be retried");
        assert_eq!(RecoveryCleanupState::Completed, recovery.cleanup_state());
        assert_eq!(2, cleanup_calls.load(Ordering::SeqCst));
    }
}

#[test]
fn test_async_provider_open_failure_without_session_has_no_recovery() {
    for directory in [false, true] {
        let (provider, _, _) = provider(CreateResponse::FailureWithoutSession);
        let filesystem = AsyncFileSystem::from_spi(provider).expect("filesystem");
        let failure = if directory {
            ready(filesystem.create_temp_directory(TempOptions::default()))
                .err()
                .expect("directory create fails")
        } else {
            ready(filesystem.create_temp_file(TempOptions::default()))
                .err()
                .expect("file create fails")
        };
        assert_eq!(OpenFailureStage::ProviderOpen, failure.stage());
        assert!(failure.recovery().is_none());
    }
}

#[test]
fn test_async_provider_open_validation_still_retains_cleanup_session() {
    for directory in [false, true] {
        let (provider, cleanup_calls, _) = provider(CreateResponse::InvalidIdentity);
        let filesystem = AsyncFileSystem::from_spi(provider).expect("filesystem");
        let mut failure = if directory {
            ready(filesystem.create_temp_directory(TempOptions::default()))
                .err()
                .expect("invalid directory identity is rejected")
        } else {
            ready(filesystem.create_temp_file(TempOptions::default()))
                .err()
                .expect("invalid file identity is rejected")
        };
        assert_eq!(OpenFailureStage::OutcomeValidation, failure.stage());
        assert_eq!(
            FsErrorKind::ProviderContractViolation,
            failure.error().kind()
        );
        let recovery = failure
            .recovery_mut()
            .expect("validation retains the opened session");
        assert_eq!(
            FsErrorKind::Io,
            ready(recovery.cleanup_async())
                .expect_err("validation session retains retryable cleanup")
                .kind()
        );
        ready(recovery.cleanup_async()).expect("validation recovery remains cleanable");
        assert_eq!(2, cleanup_calls.load(Ordering::SeqCst));
    }
}

#[test]
fn test_cancelling_async_temp_open_does_not_start_cleanup() {
    let (provider, cleanup_calls, pending_drops) = provider(CreateResponse::Pending);
    let filesystem = AsyncFileSystem::from_spi(provider).expect("filesystem");
    let mut context = Context::from_waker(Waker::noop());
    let mut opening = Box::pin(filesystem.create_temp_file(TempOptions::default()));
    assert!(opening.as_mut().poll(&mut context).is_pending());
    drop(opening);

    assert_eq!(1, pending_drops.load(Ordering::SeqCst));
    assert_eq!(0, cleanup_calls.load(Ordering::SeqCst));
}
