// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

use qubit_fs::AsyncFileSystem;
use qubit_fs::FileSystem;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

/// Provider fixture that records whether preflight reached the SPI.
#[derive(Clone)]
struct RecordingSpi {
    calls: Arc<AtomicUsize>,
}

impl RecordingSpi {
    /// Creates a fixture with no recorded SPI calls.
    fn new() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns the number of metadata requests received by the SPI.
    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl FileSystemSpi for RecordingSpi {
    /// Returns the valid provider snapshot used by the facade test.
    fn properties(&self) -> ProviderProperties {
        valid_provider_properties()
    }

    /// Records a metadata call; the test expects preflight to prevent it.
    fn stat(&self, _: StatRequest<'_>) -> FsResult<StatResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        unreachable!("invalid paths must be rejected before synchronous SPI")
    }
}

impl AsyncFileSystemSpi for RecordingSpi {
    /// Returns the valid provider snapshot used by the facade test.
    fn properties(&self) -> ProviderProperties {
        valid_provider_properties()
    }

    /// Records a metadata call; the test expects preflight to prevent it.
    fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        unreachable!("invalid paths must be rejected before asynchronous SPI")
    }
}

/// Builds a valid hierarchical provider snapshot with metadata support.
fn valid_provider_properties() -> ProviderProperties {
    ProviderProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("facade-core-test").expect("test provider id should be valid"),
            "facade-core-test",
            PathSemantics::Hierarchical,
        ),
        ProviderOperations::new().with(ProviderOperation::Stat),
        FileSystemCapabilities::new(),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("test provider properties should be valid")
}

/// Resolves the immediately-ready future returned by local preflight.
fn ready<F>(future: F) -> F::Output
where
    F: Future,
{
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = Box::pin(future);
    match Pin::as_mut(&mut future).poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("preflight future should complete immediately"),
    }
}

/// Verifies synchronous and asynchronous facades share local path preflight.
#[test]
fn test_sync_and_async_facades_reject_path_before_spi() {
    let synchronous = RecordingSpi::new();
    let filesystem = FileSystem::from_spi(synchronous.clone()).expect("synchronous facade should construct");
    let wrong = Path::parse_literal("object-key").expect("test literal path should parse");
    let error = filesystem
        .stat(&wrong)
        .expect_err("mismatched path semantics should be rejected");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(0, synchronous.call_count());

    let asynchronous = RecordingSpi::new();
    let filesystem = AsyncFileSystem::from_spi(asynchronous.clone()).expect("asynchronous facade should construct");
    let error = ready(filesystem.stat(&wrong)).expect_err("mismatched path semantics should be rejected");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(0, asynchronous.call_count());
}
