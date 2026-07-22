// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::future::Future;
use std::task::{
    Context,
    Poll,
    Waker,
};

use qubit_fs::{
    AsyncDirectoryStream,
    AsyncDirectoryStreamExt,
    AsyncDirectoryStreamSession,
    DirEntry,
    FileKind,
    FsError,
    FsErrorKind,
    FsFuture,
    FsOperation,
    FsPath,
};

#[derive(Debug)]
struct ReadyDirectorySession {
    entry: Option<DirEntry>,
}

#[derive(Debug)]
struct FailingDirectorySession;

#[derive(Debug)]
struct PartiallyFailingDirectorySession {
    entry: Option<DirEntry>,
}

#[test]
fn async_directory_stream_collects_remaining_entries() {
    let stream = AsyncDirectoryStream::new(ReadyDirectorySession {
        entry: Some(DirEntry::new(
            FsPath::parse_normalized("/collected.txt")
                .expect("path should parse"),
            FileKind::File,
        )),
    });

    let entries = ready(stream.collect_entries_async(1))
        .expect("collection should succeed");
    assert_eq!(1, entries.len());
    assert_eq!("/collected.txt", entries[0].path.as_str());
}

impl AsyncDirectoryStreamSession for ReadyDirectorySession {
    fn next_entry_async(&mut self) -> FsFuture<'_, Option<DirEntry>> {
        let entry = self.entry.take();
        Box::pin(async move { Ok(entry) })
    }
}

impl AsyncDirectoryStreamSession for FailingDirectorySession {
    fn next_entry_async(&mut self) -> FsFuture<'_, Option<DirEntry>> {
        Box::pin(async {
            Err(FsError::new(
                FsErrorKind::Io,
                FsOperation::List,
                "directory enumeration failed",
            ))
        })
    }
}

impl AsyncDirectoryStreamSession for PartiallyFailingDirectorySession {
    fn next_entry_async(&mut self) -> FsFuture<'_, Option<DirEntry>> {
        if let Some(entry) = self.entry.take() {
            Box::pin(async move { Ok(Some(entry)) })
        } else {
            Box::pin(async {
                Err(FsError::new(
                    FsErrorKind::Io,
                    FsOperation::List,
                    "directory enumeration failed",
                ))
            })
        }
    }
}

fn ready<F>(future: F) -> F::Output
where
    F: Future,
{
    let mut context = Context::from_waker(Waker::noop());
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("test future should be immediately ready"),
    }
}

#[test]
fn async_directory_stream_is_a_concrete_type_erased_handle() {
    let mut stream = AsyncDirectoryStream::new(ReadyDirectorySession {
        entry: Some(DirEntry::new(
            FsPath::parse_normalized("/a.txt").expect("path should parse"),
            FileKind::File,
        )),
    });

    assert!(format!("{stream:?}").contains("AsyncDirectoryStream"));

    assert!(
        ready(stream.next_entry_async())
            .expect("listing should succeed")
            .is_some()
    );
    assert!(
        ready(stream.next_entry_async())
            .expect("listing should finish")
            .is_none()
    );
}

#[test]
fn async_directory_stream_collection_propagates_enumeration_errors() {
    let error = ready(
        AsyncDirectoryStream::new(FailingDirectorySession)
            .collect_entries_async(1),
    )
    .expect_err("collection should preserve the enumeration failure");

    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(FsOperation::List, error.operation());
}

#[test]
fn async_directory_collection_enforces_an_inclusive_budget() {
    let stream = AsyncDirectoryStream::new(ReadyDirectorySession {
        entry: Some(DirEntry::new(
            FsPath::parse_normalized("/extra.txt").unwrap(),
            FileKind::File,
        )),
    });

    let error = ready(stream.collect_entries_async(0)).unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(FsOperation::List, error.operation());
}

#[test]
fn async_directory_collection_preserves_probe_errors() {
    let stream = AsyncDirectoryStream::new(PartiallyFailingDirectorySession {
        entry: Some(DirEntry::new(
            FsPath::parse_normalized("/entry.txt").unwrap(),
            FileKind::File,
        )),
    });

    let error = ready(stream.collect_entries_async(1)).unwrap_err();
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(FsOperation::List, error.operation());
}
