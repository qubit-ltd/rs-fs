// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public copy facade checks for provider-reported statistics.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

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
use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::AchievedAtomicity;
use qubit_fs::metadata::AtomicityRequirement;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::spi::CopyAttempt;
use qubit_fs::spi::CopyRequest;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

#[derive(Clone)]
struct CopyStatsSpi {
    stats: CopyStats,
    atomicity: AchievedAtomicity,
    calls: Arc<AtomicUsize>,
}

impl CopyStatsSpi {
    fn new(stats: CopyStats, atomicity: AchievedAtomicity) -> Self {
        Self {
            stats,
            atomicity,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn properties() -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("copy-stats-test").expect("test id should be valid"),
                "copy-stats-test",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::TryCopy),
            FileSystemCapabilities::new()
                .with_guaranteed(FileSystemCapability::Copy)
                .with_guaranteed(FileSystemCapability::AtomicFileCopy),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("test properties should be valid")
    }
}

impl FileSystemSpi for CopyStatsSpi {
    fn properties(&self) -> ProviderProperties {
        Self::properties()
    }

    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            qubit_fs::error::FsOperation::Stat,
            "copy test provider does not implement stat",
        )
        .with_path(request.path().clone()))
    }

    fn try_copy(&self, _: CopyRequest<'_>) -> Result<CopyAttempt, qubit_fs::spi::SpiCopyFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(CopyAttempt::Completed(CopyOutcome::new(
            self.stats,
            CopyMethod::Native,
            self.atomicity,
        )))
    }
}

fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}

#[test]
fn test_auto_and_tree_reject_excess_overwritten_statistics() {
    for options in [CopyOptions::default(), CopyOptions::tree()] {
        let options = options.with_conflict(CopyConflictPolicy::Overwrite);
        let spi = CopyStatsSpi::new(
            CopyStats {
                files: 1,
                overwritten: 2,
                ..CopyStats::default()
            },
            AchievedAtomicity::Atomic,
        );
        let calls = Arc::clone(&spi.calls);
        let filesystem = FileSystem::from_spi(spi).expect("facade should construct");
        let failure = filesystem
            .copy(&path("/source"), &path("/target"), options)
            .expect_err("excess overwritten statistics must violate the contract");

        assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
        assert_eq!(CopyFailureState::Indeterminate, failure.state());
        assert_eq!(
            CopyStats {
                files: 1,
                overwritten: 2,
                ..CopyStats::default()
            },
            *failure.partial_stats()
        );
        assert_eq!(1, calls.load(Ordering::SeqCst), "provider must be called once");
    }
}

#[test]
fn test_statistics_shape_legal_and_boundary_comparisons() {
    for options in [CopyOptions::default(), CopyOptions::tree()] {
        let result = CopyStatsSpi::new(
            CopyStats {
                files: 1,
                overwritten: 1,
                ..CopyStats::default()
            },
            AchievedAtomicity::Atomic,
        )
        .into_filesystem()
        .copy(
            &path("/source"),
            &path("/target"),
            options.clone().with_conflict(CopyConflictPolicy::Overwrite),
        );
        let outcome = result.expect("overwritten count equal to completed entries is valid");
        assert_eq!(1, outcome.stats().files);
        assert_eq!(1, outcome.stats().overwritten);

        assert!(
            CopyStatsSpi::new(CopyStats::default(), AchievedAtomicity::Atomic)
                .into_filesystem()
                .copy(
                    &path("/source"),
                    &path("/target"),
                    options.with_conflict(CopyConflictPolicy::Overwrite),
                )
                .is_ok()
        );
    }

    assert!(
        CopyStatsSpi::new(
            CopyStats {
                skipped: 1,
                ..CopyStats::default()
            },
            AchievedAtomicity::Atomic,
        )
        .into_filesystem()
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::file().with_conflict(CopyConflictPolicy::Skip),
        )
        .is_ok()
    );

    let overflow = CopyStatsSpi::new(
        CopyStats {
            files: u64::MAX,
            objects: 1,
            ..CopyStats::default()
        },
        AchievedAtomicity::Atomic,
    )
    .into_filesystem()
    .copy(&path("/source"), &path("/target"), CopyOptions::default())
    .expect_err("overflowing entry counters must be rejected");
    assert_eq!(FsErrorKind::ProviderContractViolation, overflow.error().kind());
    assert_eq!(CopyFailureState::Indeterminate, overflow.state());

    let downgraded = CopyStatsSpi::new(
        CopyStats {
            files: 1,
            overwritten: 1,
            ..CopyStats::default()
        },
        AchievedAtomicity::NonAtomic,
    )
    .into_filesystem()
    .copy(
        &path("/source"),
        &path("/target"),
        CopyOptions::default()
            .with_conflict(CopyConflictPolicy::Overwrite)
            .with_atomicity(AtomicityRequirement::Required),
    )
    .expect_err("required atomicity downgrade must be rejected");
    assert_eq!(FsErrorKind::ProviderContractViolation, downgraded.error().kind());
    assert_eq!(CopyFailureState::Published, downgraded.state());
}

impl CopyStatsSpi {
    fn into_filesystem(self) -> FileSystem {
        FileSystem::from_spi(self).expect("facade should construct")
    }
}

#[cfg(feature = "async")]
mod asynchronous {
    use std::future::Future;
    use std::task::Context;
    use std::task::Poll;
    use std::task::Waker;

    use qubit_fs::AsyncFileSystem;
    use qubit_fs::copy::AsyncCopyOperationState;
    use qubit_fs::spi::AsyncFileSystemSpi;
    use qubit_fs::spi::CopyRequest;
    use qubit_fs::spi::SpiFuture;

    use super::AchievedAtomicity;
    use super::Arc;
    use super::CopyAttempt;
    use super::CopyConflictPolicy;
    use super::CopyFailureState;
    use super::CopyMethod;
    use super::CopyOptions;
    use super::CopyOutcome;
    use super::CopyStats;
    use super::CopyStatsSpi;
    use super::FsError;
    use super::FsErrorKind;
    use super::FsResult;
    use super::Ordering;
    use super::ProviderProperties;
    use super::StatRequest;
    use super::StatResponse;
    use super::path;

    impl AsyncFileSystemSpi for CopyStatsSpi {
        fn properties(&self) -> ProviderProperties {
            CopyStatsSpi::properties()
        }

        fn stat<'a>(&'a self, request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
            Box::pin(async move {
                Err(FsError::new(
                    FsErrorKind::UnsupportedOperation,
                    qubit_fs::error::FsOperation::Stat,
                    "copy test provider does not implement stat",
                )
                .with_path(request.path().clone()))
            })
        }

        fn try_copy<'a>(
            &'a self,
            _: CopyRequest<'a>,
        ) -> SpiFuture<'a, Result<CopyAttempt, qubit_fs::spi::SpiCopyFailure>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                Ok(CopyAttempt::Completed(CopyOutcome::new(
                    self.stats,
                    CopyMethod::Native,
                    self.atomicity,
                )))
            })
        }
    }

    fn ready<F: Future>(future: F) -> F::Output {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut future = Box::pin(future);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test copy future should complete immediately"),
        }
    }

    fn filesystem(spi: CopyStatsSpi) -> AsyncFileSystem {
        AsyncFileSystem::from_spi(spi).expect("async facade should construct")
    }

    #[test]
    fn test_async_auto_and_tree_reject_excess_overwritten_statistics() {
        for options in [CopyOptions::default(), CopyOptions::tree()] {
            let options = options.with_conflict(CopyConflictPolicy::Overwrite);
            let spi = CopyStatsSpi::new(
                CopyStats {
                    files: 1,
                    overwritten: 2,
                    ..CopyStats::default()
                },
                AchievedAtomicity::Atomic,
            );
            let calls = Arc::clone(&spi.calls);
            let filesystem = filesystem(spi);
            let mut operation = filesystem
                .begin_copy(path("/source"), path("/target"), options)
                .expect("copy preflight should succeed");
            let failure =
                ready(operation.execute()).expect_err("excess overwritten statistics must violate the contract");

            assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
            assert_eq!(CopyFailureState::Indeterminate, failure.state());
            assert_eq!(
                CopyStats {
                    files: 1,
                    overwritten: 2,
                    ..CopyStats::default()
                },
                *failure.partial_stats()
            );
            assert_eq!(1, calls.load(Ordering::SeqCst), "provider must be called once");
            assert_eq!(
                AsyncCopyOperationState::Failed(CopyFailureState::Indeterminate),
                operation.state()
            );
        }
    }

    #[test]
    fn test_async_valid_overwrite_count_is_accepted() {
        for options in [CopyOptions::default(), CopyOptions::tree()] {
            let filesystem = filesystem(CopyStatsSpi::new(
                CopyStats {
                    files: 1,
                    overwritten: 1,
                    ..CopyStats::default()
                },
                AchievedAtomicity::Atomic,
            ));
            let mut operation = filesystem
                .begin_copy(
                    path("/source"),
                    path("/target"),
                    options.with_conflict(CopyConflictPolicy::Overwrite),
                )
                .expect("copy preflight should succeed");
            let result = ready(operation.execute());
            assert!(result.is_ok());
        }
    }
}
