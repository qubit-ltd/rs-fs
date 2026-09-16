// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Listing scopes keep namespace queries distinct from resource paths.

use std::sync::Arc;
use std::sync::atomic::Ordering;

#[cfg(feature = "async")]
use qubit_fs::AsyncFileSystem;
use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::Path;
use qubit_fs::directory::ListFilter;
use qubit_fs::directory::ListOptions;
use qubit_fs::directory::ListScope;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::DirEntry;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileSystemLimit;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::path::PathSemantics;

#[path = "support/listing.rs"]
mod list_scope_support;
use list_scope_support::ListingProvider;
use list_scope_support::Observations;
#[cfg(feature = "async")]
#[path = "common/poll_support.rs"]
mod poll_support;

/// An entire namespace is a query scope rather than an empty resource path.
#[test]
fn namespace_is_not_an_empty_resource_path() {
    assert!(Path::parse_literal("").is_err());
    assert!(ListScope::Namespace.path().is_none());
    let path = Path::parse_literal("folder/").unwrap();
    let scope = ListScope::Path(path.clone());
    assert_eq!(scope.path(), Some(&path));
}

/// Exercises each scope through both real facade implementations.
fn check_listing(
    scope: ListScope,
    options: ListOptions,
    semantics: PathSemantics,
    supported: bool,
    limits: FileSystemLimits,
    entries: Vec<DirEntry>,
    expected: Result<(), (FsErrorKind, bool)>,
) {
    let modes = if cfg!(feature = "async") {
        &[false, true][..]
    } else {
        &[false][..]
    };
    for &asynchronous in modes {
        let observations = Arc::new(Observations::default());
        let provider = ListingProvider {
            semantics,
            supported,
            limits,
            entries: entries.clone(),
            observations: Arc::clone(&observations),
        };
        let mut actual = Vec::new();
        let result = if asynchronous {
            #[cfg(feature = "async")]
            {
                poll_support::ready(async {
                    let filesystem = AsyncFileSystem::from_spi(provider).unwrap();
                    let mut stream = filesystem.list(&scope, options.clone()).await?;
                    while let Some(entry) = stream.next_entry_async().await? {
                        actual.push(entry.path);
                    }
                    Ok::<_, FsError>(())
                })
            }
            #[cfg(not(feature = "async"))]
            {
                unreachable!("async feature required")
            }
        } else {
            (|| {
                let filesystem = FileSystem::from_spi(provider).unwrap();
                let mut stream = filesystem.list(&scope, options.clone())?;
                while let Some(entry) = stream.next_entry()? {
                    actual.push(entry.path);
                }
                Ok::<_, FsError>(())
            })()
        };
        match expected {
            Ok(()) => {
                result.unwrap();
                assert_eq!(
                    actual,
                    entries.iter().map(|entry| entry.path.clone()).collect::<Vec<_>>()
                );
                assert_eq!(observations.list_calls.load(Ordering::SeqCst), 1);
                assert_eq!(observations.requests.lock().unwrap()[0].0, scope);
            }
            Err((kind, opened)) => {
                let error = result.unwrap_err();
                assert_eq!(error.kind(), kind);
                assert_eq!(error.operation(), FsOperation::List);
                if !opened {
                    assert_eq!(error.path(), scope.path());
                }
                assert_eq!(observations.list_calls.load(Ordering::SeqCst), usize::from(opened));
            }
        }
    }
}

/// Builds exact object-key entries without path normalization.
fn keys(names: &[&str]) -> Vec<DirEntry> {
    names
        .iter()
        .map(|name| DirEntry::new(Path::parse_literal(name).unwrap(), FileKind::File))
        .collect()
}

/// Namespace listings accept both top-level and nested logical keys.
#[test]
fn namespace_accepts_entire_flat_keyspace() {
    check_listing(
        ListScope::Namespace,
        ListOptions::object_keys(),
        PathSemantics::ObjectKey,
        true,
        FileSystemLimits::unknown(),
        keys(&["a", "b/c"]),
        Ok(()),
    );
}

/// A raw path prefix has no implicit separator and its filter is relative.
#[test]
fn raw_prefix_and_relative_filter_preserve_exact_text() {
    check_listing(
        ListScope::Path(Path::parse_literal("folder").unwrap()),
        ListOptions::object_keys(),
        PathSemantics::ObjectKey,
        true,
        FileSystemLimits::unknown(),
        keys(&["folder/a", "folderish"]),
        Ok(()),
    );
    let options = ListOptions::object_keys().with_filter(Some(ListFilter::LiteralPrefix("a".into())));
    check_listing(
        ListScope::Path(Path::parse_literal("folder/").unwrap()),
        options.clone(),
        PathSemantics::ObjectKey,
        true,
        FileSystemLimits::unknown(),
        keys(&["folder/a", "folder/ab"]),
        Ok(()),
    );
    check_listing(
        ListScope::Namespace,
        options.clone(),
        PathSemantics::ObjectKey,
        true,
        FileSystemLimits::unknown(),
        keys(&["a", "ab"]),
        Ok(()),
    );
    check_listing(
        ListScope::Namespace,
        options,
        PathSemantics::ObjectKey,
        true,
        FileSystemLimits::unknown(),
        keys(&["ba"]),
        Err((FsErrorKind::ProviderContractViolation, true)),
    );
}

/// Invalid scopes and unsupported capabilities are rejected before I/O.
#[test]
fn namespace_preflight_rejects_hierarchical_and_unsupported_listing() {
    check_listing(
        ListScope::Namespace,
        ListOptions::default(),
        PathSemantics::Hierarchical,
        true,
        FileSystemLimits::unknown(),
        vec![],
        Err((FsErrorKind::InvalidOptions, false)),
    );
    check_listing(
        ListScope::Namespace,
        ListOptions::object_keys(),
        PathSemantics::ObjectKey,
        false,
        FileSystemLimits::unknown(),
        vec![],
        Err((FsErrorKind::UnsupportedCapability, false)),
    );
    check_listing(
        ListScope::Path(Path::root()),
        ListOptions::default(),
        PathSemantics::Hierarchical,
        true,
        FileSystemLimits::unknown(),
        vec![DirEntry::new(Path::parse("/a").unwrap(), FileKind::File)],
        Ok(()),
    );
}

/// Combined query text is budgeted before opening, while entries are checked
/// later.
#[test]
fn query_and_entry_limits_are_distinct() {
    let limits = FileSystemLimits::unknown().with_max_path_text_bytes(FileSystemLimit::Maximum(4));
    let options = ListOptions::object_keys().with_filter(Some(ListFilter::LiteralPrefix("abcde".into())));
    check_listing(
        ListScope::Namespace,
        options,
        PathSemantics::ObjectKey,
        true,
        limits,
        vec![],
        Err((FsErrorKind::ResourceLimitExceeded, false)),
    );
    let options = ListOptions::object_keys().with_filter(Some(ListFilter::LiteralPrefix("cde".into())));
    check_listing(
        ListScope::Path(Path::parse_literal("ab").unwrap()),
        options,
        PathSemantics::ObjectKey,
        true,
        limits,
        vec![],
        Err((FsErrorKind::ResourceLimitExceeded, false)),
    );
    check_listing(
        ListScope::Namespace,
        ListOptions::object_keys(),
        PathSemantics::ObjectKey,
        true,
        limits,
        keys(&["abcde"]),
        Err((FsErrorKind::ProviderContractViolation, true)),
    );
}

/// Namespace scope never relaxes provider entry path semantics.
#[test]
fn namespace_rejects_foreign_entry_semantics() {
    check_listing(
        ListScope::Namespace,
        ListOptions::object_keys(),
        PathSemantics::ObjectKey,
        true,
        FileSystemLimits::unknown(),
        vec![DirEntry::new(Path::parse("/a").unwrap(), FileKind::File)],
        Err((FsErrorKind::ProviderContractViolation, true)),
    );
}
