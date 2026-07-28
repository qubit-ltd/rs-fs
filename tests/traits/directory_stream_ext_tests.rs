// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::{
    DirEntry,
    DirectoryStream,
    DirectoryStreamExt,
    FileKind,
    FsErrorKind,
    FsOperation,
    FsPath,
};

use crate::common::{
    FailingDirectoryStream,
    MockDirectoryStream,
    PartiallyFailingDirectoryStream,
};

#[test]
fn test_collect_entries_collects_all_entries() {
    let stream = DirectoryStream::new(MockDirectoryStream {
        entries: vec![DirEntry::new(
            FsPath::parse("/a.txt").expect("path should parse"),
            FileKind::File,
        )],
    });
    assert!(format!("{stream:?}").contains("DirectoryStream"));
    let entries = stream.collect_entries(1).expect("stream should collect");

    assert_eq!(1, entries.len());
}

#[test]
fn test_collect_entries_returns_empty_vec_for_empty_stream() {
    let entries = DirectoryStream::new(MockDirectoryStream {
        entries: Vec::new(),
    })
    .collect_entries(1)
    .expect("empty stream should collect");

    assert!(entries.is_empty());
}

#[test]
fn test_collect_entries_returns_errors_from_stream() {
    assert!(
        DirectoryStream::new(FailingDirectoryStream)
            .collect_entries(1)
            .is_err(),
    );
    assert!(
        DirectoryStream::new(PartiallyFailingDirectoryStream {
            entry: Some(DirEntry::new(
                FsPath::parse("/partial.txt").expect("path should parse"),
                FileKind::File,
            )),
        })
        .collect_entries(1)
        .is_err(),
    );
}

#[test]
fn collect_entries_enforces_an_inclusive_caller_budget() {
    let stream = DirectoryStream::new(MockDirectoryStream {
        entries: vec![DirEntry::new(
            FsPath::parse("/a.txt").unwrap(),
            FileKind::File,
        )],
    });

    let error = stream.collect_entries(0).unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(FsOperation::List, error.operation());
}

#[test]
fn collect_entries_preserves_an_error_raised_by_the_limit_probe() {
    let stream = DirectoryStream::new(PartiallyFailingDirectoryStream {
        entry: Some(DirEntry::new(
            FsPath::parse("/a.txt").unwrap(),
            FileKind::File,
        )),
    });

    let error = stream.collect_entries(1).unwrap_err();
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(FsOperation::List, error.operation());
}
