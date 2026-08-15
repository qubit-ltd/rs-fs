// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::directory::ListOptions;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::SymlinkPolicy;

#[test]
fn test_list_options_full_configuration_is_usable() {
    let options = ListOptions::default()
        .with_recursive(true)
        .with_symlink_policy(SymlinkPolicy::FollowWithinFileSystem)
        .with_include_metadata(true)
        .with_page_size(Some(10))
        .with_prefix(Some("a".to_owned()));

    assert!(options.recursive());
    assert_eq!(
        Some(SymlinkPolicy::FollowWithinFileSystem),
        options.symlink_policy_override(),
    );
    assert!(!SymlinkPolicy::Reject.follows());
    assert!(SymlinkPolicy::FollowWithinFileSystem.follows());
    assert!(options.include_metadata());
    assert_eq!(Some(10), options.page_size());
    assert_eq!(Some("a"), options.prefix());
}

/// Rejects invalid provider-facing pagination and prefix values before they
/// can be dispatched through an SPI request.
#[test]
fn test_list_options_reject_invalid_page_size_and_noncanonical_prefix() {
    for options in [
        ListOptions::default().with_page_size(Some(0)),
        ListOptions::default().with_prefix(Some("../escape".to_owned())),
        ListOptions::default().with_prefix(Some("nested/../entry".to_owned())),
    ] {
        let error = options
            .validate()
            .expect_err("invalid list option must be rejected");
        assert_eq!(FsErrorKind::InvalidOptions, error.kind());
        assert_eq!(FsOperation::List, error.operation());
    }
}
