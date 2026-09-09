// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fixed provider outcomes for scope and directory lifecycle tests.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::directory::ListOptions;
use qubit_fs::directory::ListScope;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::DirEntry;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
#[cfg(feature = "async")]
use qubit_fs::spi::AsyncDirectoryStreamSession;
#[cfg(feature = "async")]
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::DirectoryStreamSpi;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::ListRequest;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenedAsyncDirectoryStream;
use qubit_fs::spi::OpenedDirectoryStream;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
#[cfg(feature = "async")]
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

/// Records provider interaction independently of facade filtering.
#[derive(Default)]
pub struct Observations {
    pub list_calls: AtomicUsize,
    pub next_calls: AtomicUsize,
    pub requests: Mutex<Vec<(ListScope, ListOptions)>>,
}

/// A provider returning exactly the supplied entries, including invalid ones.
pub struct ListingProvider {
    pub semantics: PathSemantics,
    pub supported: bool,
    pub limits: FileSystemLimits,
    pub entries: Vec<DirEntry>,
    pub observations: Arc<Observations>,
}

impl ListingProvider {
    /// Builds a snapshot with optional List support and configured limits.
    fn snapshot(&self) -> ProviderProperties {
        let capabilities = if self.supported {
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::List)
        } else {
            FileSystemCapabilities::new()
        };
        ProviderProperties::new(
            FileSystemInfo::new(FileSystemId::new("scope-test").unwrap(), "scope-test", self.semantics),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::List),
            capabilities,
            self.limits,
            PathConstraints::either(),
            SymlinkPolicy::Reject,
        )
        .unwrap()
    }

    /// Captures the dispatched request and creates an independent iterator.
    fn open(&self, request: ListRequest<'_>) -> Entries {
        self.observations.list_calls.fetch_add(1, Ordering::SeqCst);
        self.observations
            .requests
            .lock()
            .unwrap()
            .push((request.scope().clone(), request.options().options().clone()));
        Entries {
            entries: self.entries.clone().into_iter(),
            observations: Arc::clone(&self.observations),
        }
    }
}

impl FileSystemSpi for ListingProvider {
    /// Returns the configured provider properties.
    fn properties(&self) -> ProviderProperties {
        self.snapshot()
    }

    /// Rejects stat because listing must not need a resource probe.
    fn stat(&self, _: StatRequest<'_>) -> FsResult<StatResponse> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::Stat,
            "unexpected stat",
        ))
    }

    /// Exposes raw provider results without reproducing facade validation.
    fn list(&self, request: ListRequest<'_>) -> FsResult<OpenedDirectoryStream> {
        Ok(OpenedDirectoryStream::new(Box::new(self.open(request))))
    }
}

#[cfg(feature = "async")]
impl AsyncFileSystemSpi for ListingProvider {
    /// Uses identical properties in asynchronous mode.
    fn properties(&self) -> ProviderProperties {
        self.snapshot()
    }

    /// Rejects unexpected metadata probing.
    fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async {
            Err(FsError::new(
                FsErrorKind::UnsupportedOperation,
                FsOperation::Stat,
                "unexpected stat",
            ))
        })
    }

    /// Records opening only when the provider future executes.
    fn list<'a>(&'a self, request: ListRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncDirectoryStream>> {
        Box::pin(async move { Ok(OpenedAsyncDirectoryStream::new(Box::new(self.open(request)))) })
    }
}

/// Iterator that makes every provider advancement observable.
struct Entries {
    entries: std::vec::IntoIter<DirEntry>,
    observations: Arc<Observations>,
}

impl DirectoryStreamSpi for Entries {
    /// Returns the next configured entry exactly once.
    fn next_entry(&mut self) -> FsResult<Option<DirEntry>> {
        self.observations.next_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.entries.next())
    }
}

#[cfg(feature = "async")]
impl AsyncDirectoryStreamSession for Entries {
    /// Advances only when polled, without requiring a runtime.
    fn next_entry_async(&mut self) -> SpiFuture<'_, FsResult<Option<DirEntry>>> {
        Box::pin(async move { self.next_entry() })
    }
}
