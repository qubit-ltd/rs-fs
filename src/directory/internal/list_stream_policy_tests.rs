// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deterministic tests of the crate-private directory lifecycle contract.

use std::error::Error;
use std::time::Duration;
use std::time::Instant;

use super::ListStreamPolicy;
use crate::directory::DirectoryStreamState;
use crate::directory::ListOptions;
use crate::directory::ListScope;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::metadata::DirEntry;
use crate::metadata::FileKind;
use crate::metadata::FileSystemLimits;
use crate::path::Path;
use crate::path::PathSemantics;

/// Creates a flat policy with caller-supplied time and options.
fn policy(now: Instant, options: ListOptions) -> ListStreamPolicy {
    ListStreamPolicy::new(
        ListScope::Namespace,
        options,
        "clock-test",
        PathSemantics::ObjectKey,
        FileSystemLimits::unknown(),
        now,
    )
    .unwrap()
}

/// Produces a valid entry so only lifecycle policy controls the result.
fn entry() -> DirEntry {
    DirEntry::new(Path::parse_literal("a").unwrap(), FileKind::File)
}

/// A deadline is inclusive and permanently fails the stream at the boundary.
#[test]
fn deadline_boundary_is_inclusive() {
    let now = Instant::now();
    let mut stream = policy(
        now,
        ListOptions::object_keys().with_deadline(Some(Duration::from_millis(10))),
    );
    stream.before_next(now + Duration::from_millis(9)).unwrap();
    assert_eq!(
        stream.before_next(now + Duration::from_millis(10)).unwrap_err().kind(),
        FsErrorKind::ResourceLimitExceeded
    );
    assert_eq!(stream.state(), DirectoryStreamState::Failed);
    assert_eq!(stream.before_next(now).unwrap_err().kind(), FsErrorKind::InvalidState);
}

/// Neither an entry nor EOF can conceal time spent in a provider call.
#[test]
fn successful_late_results_are_rejected() {
    let now = Instant::now();
    for value in [Some(entry()), None] {
        let mut stream = policy(
            now,
            ListOptions::object_keys().with_deadline(Some(Duration::from_millis(10))),
        );
        stream.before_next(now).unwrap();
        assert_eq!(
            stream
                .finish_next(Ok(value), now + Duration::from_millis(10))
                .unwrap_err()
                .kind(),
            FsErrorKind::ResourceLimitExceeded
        );
        assert_eq!(stream.state(), DirectoryStreamState::Failed);
    }
}

/// A real provider failure takes precedence over a simultaneously expired
/// budget.
#[test]
fn late_provider_error_preserves_kind_path_and_source() {
    let now = Instant::now();
    let mut stream = policy(
        now,
        ListOptions::object_keys().with_deadline(Some(Duration::from_millis(10))),
    );
    stream.before_next(now).unwrap();
    let path = Path::parse_literal("actual-key").unwrap();
    let error = FsError::with_source(
        FsErrorKind::PermissionDenied,
        FsOperation::List,
        "denied",
        std::io::Error::other("provider source"),
    )
    .with_path(path.clone());
    let error = stream
        .finish_next(Err(error), now + Duration::from_secs(1))
        .unwrap_err();
    assert_eq!(error.kind(), FsErrorKind::PermissionDenied);
    assert_eq!(error.path(), Some(&path));
    assert_eq!(error.source().unwrap().to_string(), "provider source");
    assert_eq!(stream.state(), DirectoryStreamState::Failed);
}

/// A zero entry allowance still permits the provider to establish an empty
/// list.
#[test]
fn empty_list_and_entry_overrun_are_distinct() {
    let now = Instant::now();
    let options = ListOptions::object_keys().with_max_entries(Some(0));
    let mut empty = policy(now, options.clone());
    empty.before_next(now).unwrap();
    assert!(empty.finish_next(Ok(None), now).unwrap().is_none());
    assert_eq!(empty.state(), DirectoryStreamState::Exhausted);
    assert_eq!(empty.before_next(now).unwrap_err().kind(), FsErrorKind::InvalidState);
    let mut nonempty = policy(now, options);
    nonempty.before_next(now).unwrap();
    assert_eq!(
        nonempty.finish_next(Ok(Some(entry())), now).unwrap_err().kind(),
        FsErrorKind::ResourceLimitExceeded
    );
}

/// Reaching the allowance does not fabricate EOF before probing the next entry.
#[test]
fn entry_limit_requires_an_explicit_overrun_probe() {
    let now = Instant::now();
    let mut stream = policy(now, ListOptions::object_keys().with_max_entries(Some(1)));
    stream.before_next(now).unwrap();
    assert!(stream.finish_next(Ok(Some(entry())), now).unwrap().is_some());
    stream.before_next(now).unwrap();
    assert_eq!(
        stream.finish_next(Ok(Some(entry())), now).unwrap_err().kind(),
        FsErrorKind::ResourceLimitExceeded
    );
}

/// Unrepresentable deadlines never silently turn into unlimited streams.
#[test]
fn overflowing_deadline_is_rejected_at_construction() {
    let result = ListStreamPolicy::new(
        ListScope::Namespace,
        ListOptions::object_keys().with_deadline(Some(Duration::MAX)),
        "clock-test",
        PathSemantics::ObjectKey,
        FileSystemLimits::unknown(),
        Instant::now(),
    );
    assert_eq!(result.err().unwrap().kind(), FsErrorKind::InvalidOptions);
}
