// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Runtime-neutral asynchronous directory deadline lifecycle regressions.

#![cfg(feature = "async")]

#[path = "support/listing.rs"]
mod list_scope_support;
#[path = "common/poll_support.rs"]
mod poll_support;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use list_scope_support::ListingProvider;
use list_scope_support::Observations;
#[cfg(feature = "async")]
use qubit_fs::AsyncFileSystem;
use qubit_fs::Path;
use qubit_fs::directory::DirectoryStreamState;
use qubit_fs::directory::ListOptions;
use qubit_fs::directory::ListScope;
use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::DirEntry;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::path::PathSemantics;

/// Merely constructing an expired future must not change stream state.
#[test]
fn dropping_unpolled_next_has_no_side_effects() {
    let observations = Arc::new(Observations::default());
    let provider = ListingProvider {
        semantics: PathSemantics::ObjectKey,
        supported: true,
        limits: FileSystemLimits::unknown(),
        entries: vec![],
        observations: Arc::clone(&observations),
    };
    let filesystem = AsyncFileSystem::from_spi(provider).unwrap();
    let mut stream = poll_support::ready(filesystem.list(
        &ListScope::Namespace,
        ListOptions::object_keys().with_deadline(Some(Duration::ZERO)),
    ))
    .unwrap();
    drop(stream.next_entry_async());
    assert_eq!(stream.state(), DirectoryStreamState::Open);
    assert_eq!(observations.next_calls.load(Ordering::SeqCst), 0);
    let error = poll_support::ready(stream.next_entry_async()).unwrap_err();
    assert_eq!(error.kind(), FsErrorKind::ResourceLimitExceeded);
    assert_eq!(error.path(), None);
    assert_eq!(stream.state(), DirectoryStreamState::Failed);
    assert_eq!(observations.next_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        poll_support::ready(stream.next_entry_async()).unwrap_err().kind(),
        FsErrorKind::InvalidState
    );
}

/// Each completed future consumes one entry and leaves subsequent entries
/// available.
#[test]
fn ready_next_consumes_exactly_one_entry() {
    let observations = Arc::new(Observations::default());
    let provider = ListingProvider {
        semantics: PathSemantics::ObjectKey,
        supported: true,
        limits: FileSystemLimits::unknown(),
        entries: ["a", "b"]
            .into_iter()
            .map(|key| DirEntry::new(Path::parse_literal(key).unwrap(), FileKind::File))
            .collect(),
        observations: Arc::clone(&observations),
    };
    let filesystem = AsyncFileSystem::from_spi(provider).unwrap();
    let mut stream = poll_support::ready(filesystem.list(&ListScope::Namespace, ListOptions::object_keys())).unwrap();
    for (index, expected) in ["a", "b"].into_iter().enumerate() {
        drop(stream.next_entry_async());
        assert_eq!(observations.next_calls.load(Ordering::SeqCst), index);
        let entry = poll_support::ready(stream.next_entry_async()).unwrap().unwrap();
        assert_eq!(entry.path.as_str(), expected);
        assert_eq!(observations.next_calls.load(Ordering::SeqCst), index + 1);
        assert_eq!(stream.state(), DirectoryStreamState::Open);
    }
    assert!(poll_support::ready(stream.next_entry_async()).unwrap().is_none());
    assert_eq!(observations.next_calls.load(Ordering::SeqCst), 3);
    assert_eq!(stream.state(), DirectoryStreamState::Exhausted);
}
