// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public temporary-resource parent checks.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
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
use qubit_fs::spi::CreateTempDirectoryRequest;
use qubit_fs::spi::CreateTempFileRequest;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::OpenedTempDirectory;
use qubit_fs::spi::OpenedTempFile;
use qubit_fs::spi::PersistRequest;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::SpiPersistFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_fs::spi::TempResourceSpi;
use qubit_fs::temp::PersistOutcome;
use qubit_fs::temp::TempOptions;

#[derive(Clone)]
struct TempParentSpi {
    semantics: PathSemantics,
    returned_path: String,
    cleanup_calls: Arc<AtomicUsize>,
    cleanup_behavior: CleanupBehavior,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum CleanupBehavior {
    #[default]
    Succeed,
    FailOnce,
    #[cfg(feature = "async")]
    PendingOnce,
}

impl TempParentSpi {
    fn properties(semantics: PathSemantics) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("temp-parent-test").expect("test id should be valid"),
                "temp-parent-test",
                semantics,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::CreateTempFile)
                .with(ProviderOperation::CreateTempDirectory),
            FileSystemCapabilities::new()
                .with_guaranteed(FileSystemCapability::TempFile)
                .with_guaranteed(FileSystemCapability::TempDirectory),
            FileSystemLimits::unknown(),
            if semantics == PathSemantics::Hierarchical {
                PathConstraints::absolute()
            } else {
                PathConstraints::either()
            },
            SymlinkPolicy::Reject,
        )
        .expect("test properties should be valid")
    }

    fn info(&self, kind: FileKind) -> FsResult<OpenedFileInfo> {
        Ok(OpenedFileInfo::new(
            FileSystemId::new("temp-parent-test")?,
            Path::parse_with_semantics(&self.returned_path, self.semantics)?,
        )
        .with_metadata(FileMetadata::new(kind)))
    }
}

struct SyncTempSession {
    cleanup_calls: Arc<AtomicUsize>,
    cleanup_behavior: CleanupBehavior,
}

impl TempResourceSpi for SyncTempSession {
    fn persist(&mut self, _: PersistRequest<'_>) -> Result<PersistOutcome, SpiPersistFailure> {
        unreachable!("lifecycle methods are outside the opening contract")
    }

    fn keep(&mut self) -> Result<PersistOutcome, SpiPersistFailure> {
        unreachable!("lifecycle methods are outside the opening contract")
    }

    fn cleanup(&mut self) -> FsResult<()> {
        let attempt = self.cleanup_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.cleanup_behavior == CleanupBehavior::FailOnce && attempt == 0 {
            Err(FsError::new(
                FsErrorKind::Io,
                FsOperation::CleanupTemp,
                "injected cleanup failure",
            ))
        } else {
            Ok(())
        }
    }
}

impl FileSystemSpi for TempParentSpi {
    fn properties(&self) -> ProviderProperties {
        Self::properties(self.semantics)
    }

    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Err(
            FsError::new(FsErrorKind::UnsupportedOperation, FsOperation::Stat, "stat is unused")
                .with_path(request.path().clone()),
        )
    }

    fn create_temp_file(&self, _: CreateTempFileRequest) -> FsResult<OpenedTempFile> {
        Ok(OpenedTempFile::new(
            self.info(FileKind::File)?,
            Box::new(SyncTempSession {
                cleanup_calls: Arc::clone(&self.cleanup_calls),
                cleanup_behavior: self.cleanup_behavior,
            }),
        ))
    }

    fn create_temp_directory(&self, _: CreateTempDirectoryRequest) -> FsResult<OpenedTempDirectory> {
        Ok(OpenedTempDirectory::new(
            self.info(FileKind::Directory)?,
            Box::new(SyncTempSession {
                cleanup_calls: Arc::clone(&self.cleanup_calls),
                cleanup_behavior: self.cleanup_behavior,
            }),
        ))
    }
}

fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}

fn path_with_semantics(value: &str, semantics: PathSemantics) -> Path {
    Path::parse_with_semantics(value, semantics).expect("test path should parse")
}

fn assert_sync_rejected(kind: FileKind) {
    let filesystem = FileSystem::from_spi(TempParentSpi {
        semantics: PathSemantics::Hierarchical,
        returned_path: "/other/temp".to_owned(),
        cleanup_calls: Arc::new(AtomicUsize::new(0)),
        cleanup_behavior: CleanupBehavior::Succeed,
    })
    .expect("facade should construct");
    let options = TempOptions::new().with_parent(Some(path("/reports")));
    let failure = match kind {
        FileKind::File => filesystem
            .create_temp_file(options)
            .expect_err("outside parent must be rejected"),
        FileKind::Directory => filesystem
            .create_temp_directory(options)
            .expect_err("outside parent must be rejected"),
        _ => unreachable!("test requests only file and directory"),
    };
    assert_eq!(qubit_fs::error::OpenFailureStage::OutcomeValidation, failure.stage());
    assert_eq!(
        qubit_fs::error::FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert!(failure.recovery().is_some(), "the provider session remains recoverable");
    assert_eq!(Some(&path("/reports")), failure.error().path());
}

#[test]
fn test_sync_temp_file_and_directory_reject_provider_location_outside_requested_parent() {
    assert_sync_rejected(FileKind::File);
    assert_sync_rejected(FileKind::Directory);
}

#[test]
fn test_sync_rejected_temp_cleanup_failure_retains_session_and_rejects_repeat_after_success() {
    let cleanup_calls = Arc::new(AtomicUsize::new(0));
    let filesystem = FileSystem::from_spi(TempParentSpi {
        semantics: PathSemantics::Hierarchical,
        returned_path: "/other/temp".to_owned(),
        cleanup_calls: Arc::clone(&cleanup_calls),
        cleanup_behavior: CleanupBehavior::FailOnce,
    })
    .expect("facade should construct");
    let mut failure = filesystem
        .create_temp_file(TempOptions::new().with_parent(Some(path("/reports"))))
        .expect_err("outside parent must be rejected");
    let recovery = failure.recovery_mut().expect("recovery session must be retained");
    assert_eq!(RecoveryCleanupState::Pending, recovery.cleanup_state());
    assert_eq!(
        FsErrorKind::Io,
        recovery.cleanup().expect_err("first cleanup attempt fails").kind()
    );
    assert_eq!(RecoveryCleanupState::Indeterminate, recovery.cleanup_state());
    assert_eq!(1, cleanup_calls.load(std::sync::atomic::Ordering::SeqCst));

    recovery.cleanup().expect("retained session can be cleaned on retry");
    assert_eq!(RecoveryCleanupState::Completed, recovery.cleanup_state());
    assert_eq!(
        FsErrorKind::InvalidState,
        recovery
            .cleanup()
            .expect_err("confirmed cleanup cannot be repeated")
            .kind()
    );
    assert_eq!(2, cleanup_calls.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn test_sync_temp_parent_location_semantics_and_recovery() {
    for (semantics, parent, returned_path, should_succeed) in [
        (
            PathSemantics::Hierarchical,
            Some("/reports"),
            "/reports/nested/temp",
            true,
        ),
        (PathSemantics::Hierarchical, Some("/reports"), "/reports2/temp", false),
        (PathSemantics::Hierarchical, Some("/reports"), "/reports", false),
        (PathSemantics::Hierarchical, Some("/"), "/tmp/temp", true),
        (PathSemantics::Hierarchical, Some("/"), "/", false),
        (PathSemantics::Hierarchical, None, "/unconstrained/temp", true),
        (PathSemantics::ObjectKey, Some("/reports"), "/reports2/key", true),
        (PathSemantics::ObjectKey, Some("/reports"), "/reports", true),
        (PathSemantics::ObjectKey, Some("/reports/"), "/reports/%2F//key", true),
        (PathSemantics::ObjectKey, Some("/reports"), "/other/key", false),
        (PathSemantics::ProviderSpecific, Some("/reports"), "/other/key", true),
    ] {
        let cleanup_calls = Arc::new(AtomicUsize::new(0));
        let filesystem = FileSystem::from_spi(TempParentSpi {
            semantics,
            returned_path: returned_path.to_owned(),
            cleanup_calls: Arc::clone(&cleanup_calls),
            cleanup_behavior: CleanupBehavior::Succeed,
        })
        .expect("facade should construct");
        let parent = parent.map(|value| path_with_semantics(value, semantics));
        let options = TempOptions::new().with_parent(parent.clone());
        let result = filesystem.create_temp_file(options);
        assert_eq!(
            should_succeed,
            result.is_ok(),
            "semantics={semantics:?}, parent={parent:?}, returned={returned_path}"
        );
        match result {
            Ok(mut file) => file.cleanup().expect("valid temporary resource should clean up"),
            Err(mut failure) => {
                assert_eq!(qubit_fs::error::OpenFailureStage::OutcomeValidation, failure.stage());
                assert_eq!(
                    Some(&parent.expect("rejected cases have an explicit parent")),
                    failure.error().path()
                );
                failure
                    .recovery_mut()
                    .expect("rejected resource must retain cleanup authority")
                    .cleanup()
                    .expect("rejected session cleanup should remain available");
            }
        }
        assert_eq!(1, cleanup_calls.load(std::sync::atomic::Ordering::SeqCst));
    }
}

#[cfg(feature = "async")]
mod asynchronous {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::Context;
    use std::task::Poll;
    use std::task::Waker;

    use qubit_fs::AsyncFileSystem;
    use qubit_fs::error::FsResult;
    use qubit_fs::spi::AsyncFileSystemSpi;
    use qubit_fs::spi::AsyncTempResourceSpi;
    use qubit_fs::spi::OpenedAsyncTempDirectory;
    use qubit_fs::spi::OpenedAsyncTempFile;
    use qubit_fs::spi::PersistRequest;
    use qubit_fs::spi::SpiFuture;
    use qubit_fs::spi::SpiPersistFailure;
    use qubit_fs::temp::PersistOutcome;

    use super::Arc;
    use super::CreateTempDirectoryRequest;
    use super::CreateTempFileRequest;
    use super::FileKind;
    use super::FsError;
    use super::FsErrorKind;
    use super::FsOperation;
    use super::Path;
    use super::PathSemantics;
    use super::ProviderProperties;
    use super::RecoveryCleanupState;
    use super::StatRequest;
    use super::StatResponse;
    use super::TempOptions;
    use super::TempParentSpi;
    use super::path;

    struct AsyncTempSession {
        cleanup_calls: Arc<std::sync::atomic::AtomicUsize>,
        cleanup_behavior: super::CleanupBehavior,
    }

    impl AsyncTempResourceSpi for AsyncTempSession {
        fn cleanup<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>> {
            let cleanup_calls = Arc::clone(&self.cleanup_calls);
            let cleanup_behavior = self.cleanup_behavior;
            Box::pin(async move {
                let attempt = cleanup_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if cleanup_behavior == super::CleanupBehavior::PendingOnce && attempt == 0 {
                    std::future::pending::<FsResult<()>>().await
                } else if cleanup_behavior == super::CleanupBehavior::FailOnce && attempt == 0 {
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

        fn keep<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, Result<PersistOutcome, SpiPersistFailure>> {
            Box::pin(async { unreachable!("lifecycle methods are outside the opening contract") })
        }

        fn persist<'a>(
            self: Pin<&'a mut Self>,
            _: PersistRequest<'a>,
        ) -> SpiFuture<'a, Result<PersistOutcome, SpiPersistFailure>> {
            Box::pin(async { unreachable!("lifecycle methods are outside the opening contract") })
        }
    }

    impl AsyncFileSystemSpi for TempParentSpi {
        fn properties(&self) -> ProviderProperties {
            Self::properties(self.semantics)
        }

        fn stat<'a>(&'a self, request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
            Box::pin(async move {
                Err(
                    FsError::new(FsErrorKind::UnsupportedOperation, FsOperation::Stat, "stat is unused")
                        .with_path(request.path().clone()),
                )
            })
        }

        fn create_temp_file<'a>(&'a self, _: CreateTempFileRequest) -> SpiFuture<'a, FsResult<OpenedAsyncTempFile>> {
            Box::pin(async move {
                Ok(OpenedAsyncTempFile::new(
                    self.info(FileKind::File)?,
                    Box::new(AsyncTempSession {
                        cleanup_calls: Arc::clone(&self.cleanup_calls),
                        cleanup_behavior: self.cleanup_behavior,
                    }),
                ))
            })
        }

        fn create_temp_directory<'a>(
            &'a self,
            _: CreateTempDirectoryRequest,
        ) -> SpiFuture<'a, FsResult<OpenedAsyncTempDirectory>> {
            Box::pin(async move {
                Ok(OpenedAsyncTempDirectory::new(
                    self.info(FileKind::Directory)?,
                    Box::new(AsyncTempSession {
                        cleanup_calls: Arc::clone(&self.cleanup_calls),
                        cleanup_behavior: self.cleanup_behavior,
                    }),
                ))
            })
        }
    }

    fn ready<F: Future>(future: F) -> F::Output {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut future = Box::pin(future);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("temporary open should complete immediately"),
        }
    }

    fn assert_async_rejected(kind: FileKind) {
        let filesystem = AsyncFileSystem::from_spi(TempParentSpi {
            semantics: PathSemantics::Hierarchical,
            returned_path: "/other/temp".to_owned(),
            cleanup_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            cleanup_behavior: super::CleanupBehavior::Succeed,
        })
        .expect("async facade should construct");
        let options = TempOptions::new().with_parent(Some(path("/reports")));
        let failure = match kind {
            FileKind::File => ready(filesystem.create_temp_file(options))
                .err()
                .expect("outside parent must be rejected"),
            FileKind::Directory => ready(filesystem.create_temp_directory(options))
                .err()
                .expect("outside parent must be rejected"),
            _ => unreachable!("test requests only file and directory"),
        };
        assert_eq!(qubit_fs::error::OpenFailureStage::OutcomeValidation, failure.stage());
        assert_eq!(
            qubit_fs::error::FsErrorKind::ProviderContractViolation,
            failure.error().kind()
        );
        assert!(failure.recovery().is_some(), "the provider session remains recoverable");
        assert_eq!(Some(&path("/reports")), failure.error().path());
    }

    #[test]
    fn test_async_temp_file_and_directory_reject_provider_location_outside_requested_parent() {
        assert_async_rejected(FileKind::File);
        assert_async_rejected(FileKind::Directory);
    }

    #[test]
    fn test_async_rejected_temp_cleanup_failure_retains_session_and_rejects_repeat_after_success() {
        let cleanup_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let filesystem = AsyncFileSystem::from_spi(TempParentSpi {
            semantics: PathSemantics::Hierarchical,
            returned_path: "/other/temp".to_owned(),
            cleanup_calls: Arc::clone(&cleanup_calls),
            cleanup_behavior: super::CleanupBehavior::FailOnce,
        })
        .expect("async facade should construct");
        let mut failure = ready(filesystem.create_temp_file(TempOptions::new().with_parent(Some(path("/reports")))))
            .err()
            .expect("outside parent must be rejected");
        let recovery = failure.recovery_mut().expect("recovery session must be retained");
        assert_eq!(RecoveryCleanupState::Pending, recovery.cleanup_state());
        assert_eq!(
            FsErrorKind::Io,
            ready(recovery.cleanup_async())
                .expect_err("first cleanup attempt fails")
                .kind()
        );
        assert_eq!(RecoveryCleanupState::Indeterminate, recovery.cleanup_state());
        assert_eq!(1, cleanup_calls.load(std::sync::atomic::Ordering::SeqCst));

        ready(recovery.cleanup_async()).expect("retained session can be cleaned on retry");
        assert_eq!(RecoveryCleanupState::Completed, recovery.cleanup_state());
        assert_eq!(
            FsErrorKind::InvalidState,
            ready(recovery.cleanup_async())
                .expect_err("confirmed cleanup cannot be repeated")
                .kind()
        );
        assert_eq!(2, cleanup_calls.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_async_unpolled_rejected_cleanup_future_leaves_session_pending() {
        let cleanup_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let filesystem = AsyncFileSystem::from_spi(TempParentSpi {
            semantics: PathSemantics::Hierarchical,
            returned_path: "/other/temp".to_owned(),
            cleanup_calls: Arc::clone(&cleanup_calls),
            cleanup_behavior: super::CleanupBehavior::Succeed,
        })
        .expect("async facade should construct");
        let mut failure = ready(filesystem.create_temp_file(TempOptions::new().with_parent(Some(path("/reports")))))
            .err()
            .expect("outside parent must be rejected");
        let recovery = failure.recovery_mut().expect("recovery session must be retained");
        assert_eq!(RecoveryCleanupState::Pending, recovery.cleanup_state());
        let cleanup = recovery.cleanup_async();
        drop(cleanup);
        assert_eq!(RecoveryCleanupState::Pending, recovery.cleanup_state());
        assert_eq!(0, cleanup_calls.load(std::sync::atomic::Ordering::SeqCst));
        ready(recovery.cleanup_async()).expect("unpolled cleanup can be retried");
        assert_eq!(RecoveryCleanupState::Completed, recovery.cleanup_state());
        assert_eq!(1, cleanup_calls.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_async_polled_pending_rejected_cleanup_cancellation_retains_indeterminate_session() {
        let cleanup_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let filesystem = AsyncFileSystem::from_spi(TempParentSpi {
            semantics: PathSemantics::Hierarchical,
            returned_path: "/other/temp".to_owned(),
            cleanup_calls: Arc::clone(&cleanup_calls),
            cleanup_behavior: super::CleanupBehavior::PendingOnce,
        })
        .expect("async facade should construct");
        let mut failure = ready(filesystem.create_temp_file(TempOptions::new().with_parent(Some(path("/reports")))))
            .err()
            .expect("outside parent must be rejected");
        let recovery = failure.recovery_mut().expect("recovery session must be retained");
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut cleanup = recovery.cleanup_async();
        assert!(cleanup.as_mut().poll(&mut context).is_pending());
        assert_eq!(1, cleanup_calls.load(std::sync::atomic::Ordering::SeqCst));
        drop(cleanup);

        assert_eq!(RecoveryCleanupState::Indeterminate, recovery.cleanup_state());
        ready(recovery.cleanup_async()).expect("cancelled session can be cleaned on retry");
        assert_eq!(RecoveryCleanupState::Completed, recovery.cleanup_state());
        assert_eq!(2, cleanup_calls.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(
            FsErrorKind::InvalidState,
            ready(recovery.cleanup_async())
                .expect_err("confirmed cleanup cannot be repeated")
                .kind()
        );
        assert_eq!(2, cleanup_calls.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_async_temp_parent_location_semantics_and_recovery() {
        for (semantics, parent, returned_path, should_succeed) in [
            (
                PathSemantics::Hierarchical,
                Some("/reports"),
                "/reports/nested/temp",
                true,
            ),
            (PathSemantics::Hierarchical, Some("/reports"), "/reports2/temp", false),
            (PathSemantics::Hierarchical, Some("/reports"), "/reports", false),
            (PathSemantics::Hierarchical, Some("/"), "/tmp/temp", true),
            (PathSemantics::Hierarchical, Some("/"), "/", false),
            (PathSemantics::Hierarchical, None, "/unconstrained/temp", true),
            (PathSemantics::ObjectKey, Some("/reports"), "/reports2/key", true),
            (PathSemantics::ObjectKey, Some("/reports"), "/reports", true),
            (PathSemantics::ObjectKey, Some("/reports/"), "/reports/%2F//key", true),
            (PathSemantics::ObjectKey, Some("/reports"), "/other/key", false),
            (PathSemantics::ProviderSpecific, Some("/reports"), "/other/key", true),
        ] {
            let cleanup_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let filesystem = AsyncFileSystem::from_spi(TempParentSpi {
                semantics,
                returned_path: returned_path.to_owned(),
                cleanup_calls: Arc::clone(&cleanup_calls),
                cleanup_behavior: super::CleanupBehavior::Succeed,
            })
            .expect("async facade should construct");
            let parent = parent.map(|value| Path::parse_with_semantics(value, semantics).expect("parent"));
            let options = TempOptions::new().with_parent(parent.clone());
            let result = ready(filesystem.create_temp_file(options));
            assert_eq!(
                should_succeed,
                result.is_ok(),
                "semantics={semantics:?}, parent={parent:?}, returned={returned_path}"
            );
            match result {
                Ok(mut file) => ready(file.cleanup()).expect("valid resource cleanup should succeed"),
                Err(mut failure) => {
                    assert_eq!(qubit_fs::error::OpenFailureStage::OutcomeValidation, failure.stage());
                    assert_eq!(
                        Some(&parent.expect("rejected cases have an explicit parent")),
                        failure.error().path()
                    );
                    ready(
                        failure
                            .recovery_mut()
                            .expect("rejected resource must retain cleanup authority")
                            .cleanup_async(),
                    )
                    .expect("rejected session cleanup should remain available");
                }
            }
            assert_eq!(1, cleanup_calls.load(std::sync::atomic::Ordering::SeqCst));
        }
    }
}
