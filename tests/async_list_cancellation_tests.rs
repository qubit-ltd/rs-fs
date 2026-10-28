// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Cancellation behavior of the public asynchronous directory stream facade.

#![cfg(feature = "async")]

mod common;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use std::time::Duration;

use qubit_fs::AsyncFileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::directory::DirectoryStreamState;
use qubit_fs::directory::ListOptions;
use qubit_fs::directory::ListScope;
use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::DirEntry;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::spi::AsyncDirectoryStreamSession;
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::ListRequest;
use qubit_fs::spi::OpenedAsyncDirectoryStream;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

struct PendingAfterConsumeProvider {
    next_calls: Arc<AtomicUsize>,
    pending_polls: usize,
    provider_error: bool,
}

impl AsyncFileSystemSpi for PendingAfterConsumeProvider {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("cancel-test").unwrap(),
                "cancel-test",
                PathSemantics::ObjectKey,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::List),
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::List),
            FileSystemLimits::unknown(),
            PathConstraints::either(),
            SymlinkPolicy::Reject,
        )
        .unwrap()
    }

    fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async {
            Err(FsError::new(
                FsErrorKind::UnsupportedOperation,
                qubit_fs::error::FsOperation::Stat,
                "unexpected stat",
            ))
        })
    }

    fn list<'a>(&'a self, _: ListRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncDirectoryStream>> {
        let next_calls = Arc::clone(&self.next_calls);
        let pending_polls = self.pending_polls;
        let provider_error = self.provider_error;
        Box::pin(async move {
            Ok(OpenedAsyncDirectoryStream::new(Box::new(PendingAfterConsumeSession {
                entries: ["a", "b"]
                    .map(|key| DirEntry::new(Path::parse_literal(key).unwrap(), FileKind::File))
                    .into_iter(),
                pending_polls_remaining: pending_polls,
                consumed_first: false,
                provider_error,
                next_calls,
            })))
        })
    }
}

struct PendingAfterConsumeSession {
    entries: std::array::IntoIter<DirEntry, 2>,
    pending_polls_remaining: usize,
    consumed_first: bool,
    provider_error: bool,
    next_calls: Arc<AtomicUsize>,
}

impl AsyncDirectoryStreamSession for PendingAfterConsumeSession {
    fn next_entry_async(&mut self) -> SpiFuture<'_, FsResult<Option<DirEntry>>> {
        self.next_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::poll_fn(move |_| {
            if self.pending_polls_remaining > 0 {
                self.pending_polls_remaining -= 1;
                if !self.consumed_first {
                    self.consumed_first = true;
                    let _consumed_but_unreturned = self.entries.next();
                }
                Poll::Pending
            } else {
                if self.provider_error {
                    self.provider_error = false;
                    Poll::Ready(Err(FsError::new(
                        FsErrorKind::UnsupportedOperation,
                        qubit_fs::error::FsOperation::List,
                        "scripted provider failure",
                    )))
                } else {
                    Poll::Ready(Ok(self.entries.next()))
                }
            }
        }))
    }
}

/// Cancelling after provider progress fails the stream without another call.
#[test]
fn test_pending_cancellation_does_not_silently_continue() {
    let next_calls = Arc::new(AtomicUsize::new(0));
    let filesystem = AsyncFileSystem::from_spi(PendingAfterConsumeProvider {
        next_calls: Arc::clone(&next_calls),
        pending_polls: 1,
        provider_error: false,
    })
    .unwrap();
    let mut stream =
        common::poll_support::ready(filesystem.list(&ListScope::Namespace, ListOptions::object_keys())).unwrap();

    drop(stream.next_entry_async());
    assert_eq!(DirectoryStreamState::Open, stream.state());
    assert_eq!(0, next_calls.load(Ordering::SeqCst));

    let mut pending = Box::pin(stream.next_entry_async());
    let mut context = Context::from_waker(Waker::noop());
    assert!(Pin::as_mut(&mut pending).poll(&mut context).is_pending());
    drop(pending);

    assert_eq!(DirectoryStreamState::Failed, stream.state());
    assert_eq!(1, next_calls.load(Ordering::SeqCst));
    let error = common::poll_support::ready(stream.next_entry_async()).unwrap_err();
    assert_eq!(FsErrorKind::InvalidState, error.kind());
    assert_eq!(DirectoryStreamState::Failed, stream.state());
    assert_eq!(1, next_calls.load(Ordering::SeqCst));
}

/// A completed next call can remain pending repeatedly, then yield an entry
/// and report EOF without making the stream terminal early.
#[test]
fn test_repeated_pending_then_entry_and_eof() {
    let next_calls = Arc::new(AtomicUsize::new(0));
    let filesystem = AsyncFileSystem::from_spi(PendingAfterConsumeProvider {
        next_calls: Arc::clone(&next_calls),
        pending_polls: 2,
        provider_error: false,
    })
    .unwrap();
    let mut stream =
        common::poll_support::ready(filesystem.list(&ListScope::Namespace, ListOptions::object_keys())).unwrap();
    let mut future = Box::pin(stream.next_entry_async());
    let mut context = Context::from_waker(Waker::noop());
    assert!(Pin::as_mut(&mut future).poll(&mut context).is_pending());
    assert!(Pin::as_mut(&mut future).poll(&mut context).is_pending());
    let entry = match Pin::as_mut(&mut future).poll(&mut context) {
        Poll::Ready(Ok(Some(entry))) => entry,
        result => panic!("expected entry after pending polls, got {result:?}"),
    };
    assert_eq!("b", entry.path.as_str());
    drop(future);
    assert_eq!(DirectoryStreamState::Open, stream.state());
    assert!(
        common::poll_support::ready(stream.next_entry_async())
            .unwrap()
            .is_none()
    );
    assert_eq!(DirectoryStreamState::Exhausted, stream.state());
    assert_eq!(2, next_calls.load(Ordering::SeqCst));
}

/// Provider errors retain their category and make the stream terminal.
#[test]
fn test_provider_error_remains_terminal() {
    let next_calls = Arc::new(AtomicUsize::new(0));
    let filesystem = AsyncFileSystem::from_spi(PendingAfterConsumeProvider {
        next_calls: Arc::clone(&next_calls),
        pending_polls: 0,
        provider_error: true,
    })
    .unwrap();
    let mut stream =
        common::poll_support::ready(filesystem.list(&ListScope::Namespace, ListOptions::object_keys())).unwrap();
    let error = common::poll_support::ready(stream.next_entry_async()).unwrap_err();
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    assert_eq!(DirectoryStreamState::Failed, stream.state());
    assert_eq!(1, next_calls.load(Ordering::SeqCst));
    assert_eq!(
        FsErrorKind::InvalidState,
        common::poll_support::ready(stream.next_entry_async())
            .unwrap_err()
            .kind()
    );
    assert_eq!(1, next_calls.load(Ordering::SeqCst));
}

/// An out-of-scope provider entry keeps its contract-violation category.
#[test]
fn test_out_of_scope_entry_remains_a_contract_violation() {
    let next_calls = Arc::new(AtomicUsize::new(0));
    let filesystem = AsyncFileSystem::from_spi(PendingAfterConsumeProvider {
        next_calls: Arc::clone(&next_calls),
        pending_polls: 1,
        provider_error: false,
    })
    .unwrap();
    let scope = ListScope::Path(Path::parse_literal("a").unwrap());
    let mut stream = common::poll_support::ready(filesystem.list(&scope, ListOptions::object_keys())).unwrap();
    let mut future = Box::pin(stream.next_entry_async());
    let mut context = Context::from_waker(Waker::noop());
    assert!(Pin::as_mut(&mut future).poll(&mut context).is_pending());
    let error = match Pin::as_mut(&mut future).poll(&mut context) {
        Poll::Ready(Err(error)) => error,
        result => panic!("expected provider contract violation, got {result:?}"),
    };
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    drop(future);
    assert_eq!(DirectoryStreamState::Failed, stream.state());
    assert_eq!(1, next_calls.load(Ordering::SeqCst));
}

/// A deadline rejection keeps its resource-limit category without dispatch.
#[test]
fn test_deadline_rejection_does_not_call_provider() {
    let next_calls = Arc::new(AtomicUsize::new(0));
    let filesystem = AsyncFileSystem::from_spi(PendingAfterConsumeProvider {
        next_calls: Arc::clone(&next_calls),
        pending_polls: 1,
        provider_error: false,
    })
    .unwrap();
    let mut stream = common::poll_support::ready(filesystem.list(
        &ListScope::Namespace,
        ListOptions::object_keys().with_deadline(Some(Duration::ZERO)),
    ))
    .unwrap();
    let error = common::poll_support::ready(stream.next_entry_async()).unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(DirectoryStreamState::Failed, stream.state());
    assert_eq!(0, next_calls.load(Ordering::SeqCst));
}
