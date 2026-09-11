// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Coverage for directory-creation option defaults and metadata replacement.

use qubit_fs::directory::CreateDirectoryOptions;
use qubit_fs::metadata::UserMetadata;

/// Verifies default creation options reject existing directories without
/// creating parents or retaining metadata.
#[test]
fn test_create_directory_options_default_is_non_recursive_and_strict() {
    let options = CreateDirectoryOptions::default();

    assert!(!options.recursive());
    assert!(!options.exists_ok());
    assert!(options.user_metadata().is_empty());

    let configured = options.clone().with_recursive(true).with_exists_ok(true);
    assert!(configured.recursive());
    assert!(configured.exists_ok());
}

/// Verifies validated user metadata replaces the default empty metadata.
#[test]
fn test_create_directory_options_with_user_metadata_replaces_metadata() {
    let metadata = UserMetadata::new()
        .with("owner", "storage")
        .expect("safe metadata key must be accepted");

    let options = CreateDirectoryOptions::default().with_user_metadata(metadata);

    assert_eq!(options.user_metadata().get("owner"), Some("storage"));
}

#[test]
fn test_create_directory_options_accessors_are_callable_directly() {
    let options = CreateDirectoryOptions::default();
    let recursive: fn(&CreateDirectoryOptions) -> bool = CreateDirectoryOptions::recursive;
    let exists_ok: fn(&CreateDirectoryOptions) -> bool = CreateDirectoryOptions::exists_ok;
    let user_metadata: fn(&CreateDirectoryOptions) -> &_ = CreateDirectoryOptions::user_metadata;

    assert!(!recursive(&options));
    assert!(!exists_ok(&options));
    assert!(user_metadata(&options).is_empty());
}
