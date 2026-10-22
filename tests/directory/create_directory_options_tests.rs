// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::directory::CreateDirectoryOptions;
use qubit_fs::metadata::UserMetadata;

#[test]
fn test_option_accessors_are_executed_at_runtime() {
    let options = CreateDirectoryOptions::default()
        .with_recursive(true)
        .with_exists_ok(true)
        .with_user_metadata(UserMetadata::new());

    assert!(options.recursive());
    assert!(options.exists_ok());
    assert!(options.user_metadata().is_empty());
}
